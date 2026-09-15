use rusqlite::{params, Connection, Result};
use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Db {
    conn: Connection,
}

/// Retención máxima de beacons (ronda 11): la tabla crecía sin límite en
/// labs de larga duración (un beacon cada pocos segundos por agente →
/// millones de filas y una BD de gigas). Al superar el tope se recorta a
/// los más recientes.
const MAX_BEACONS: i64 = 10_000;

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS beacons (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                agent_id TEXT NOT NULL,
                agent_role TEXT NOT NULL,
                hostname TEXT DEFAULT '',
                username TEXT DEFAULT '',
                os TEXT DEFAULT '',
                version TEXT DEFAULT '',
                extra TEXT DEFAULT '{}'
            );
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created INTEGER NOT NULL,
                agent_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                command TEXT NOT NULL,
                payload TEXT NOT NULL,
                claimed INTEGER DEFAULT 0,
                completed INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_agent ON tasks(agent_id, claimed);
            CREATE INDEX IF NOT EXISTS idx_beacons_ts ON beacons(timestamp DESC);
            ",
        )?;
        Ok(())
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    #[allow(clippy::too_many_arguments)] // mirrors the beacons table columns
    pub fn record_beacon(
        &self,
        agent_id: &str,
        agent_role: &str,
        hostname: &str,
        username: &str,
        os: &str,
        version: &str,
        extra: &Value,
    ) {
        let ts = Self::now();
        let extra_str = serde_json::to_string(extra).unwrap_or_default();
        if let Err(e) = self.conn.execute(
            "INSERT INTO beacons (timestamp, agent_id, agent_role, hostname, username, os, version, extra)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![ts, agent_id, agent_role, hostname, username, os, version, extra_str],
        ) {
            tracing::error!("record_beacon failed: {e}");
        }

        // Retención (ronda 11): corte por id — O(1) con el rowid, se puede
        // ejecutar en cada insert sin degradar el beaconing. (La variante
        // NOT IN con ORDER BY DESC era O(n²) por inserción: 10k beacons
        // tardaban >60 s solo en prunes.)
        if let Err(e) = self.conn.execute(
            "DELETE FROM beacons WHERE id <= (SELECT COALESCE(MAX(id), 0) FROM beacons) - ?1",
            params![MAX_BEACONS],
        ) {
            tracing::error!("beacon retention failed: {e}");
        }
    }

    pub fn push_task(&self, agent_id: &str, task_id: &str, command: &str, payload: &Value) {
        let ts = Self::now();
        let payload_str = serde_json::to_string(payload).unwrap_or_default();
        if let Err(e) = self.conn.execute(
            "INSERT INTO tasks (created, agent_id, task_id, command, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ts, agent_id, task_id, command, payload_str],
        ) {
            tracing::error!("push_task failed: {e}");
        }
    }

    pub fn pending_tasks(&self, agent_id: &str) -> Vec<super::Task> {
        let Ok(mut stmt) = self.conn.prepare(
            "SELECT task_id, command, payload FROM tasks
             WHERE agent_id = ?1 AND claimed = 0 ORDER BY created ASC LIMIT 10",
        ) else {
            tracing::error!("pending_tasks: prepare failed");
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![agent_id], |row| {
            let task_id: String = row.get(0)?;
            let command: String = row.get(1)?;
            let payload_str: String = row.get(2)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
            Ok(super::Task {
                id: task_id,
                command,
                payload,
            })
        }) else {
            tracing::error!("pending_tasks: query failed");
            return Vec::new();
        };
        let tasks: Vec<super::Task> = rows.filter_map(|r| r.ok()).collect();

        for task in &tasks {
            // Ronda 11: el UPDATE antes solo filtraba por task_id — un
            // task_id repetido entre agentes (el operador decide los ids)
            // marcaba como claimed una tarea de OTRO agente sin entregarla.
            let _ = self.conn.execute(
                "UPDATE tasks SET claimed = 1 WHERE task_id = ?1 AND agent_id = ?2",
                params![task.id, agent_id],
            );
        }

        tasks
    }

    pub fn beacon_count(&self) -> usize {
        let beacons: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM beacons", [], |r| r.get(0))
            .unwrap_or(0);
        beacons as usize
    }

    pub fn recent_activity(&self, limit: usize) -> Vec<super::LogEntry> {
        let mut entries = Vec::new();

        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT timestamp, agent_id, agent_role, hostname, username, os, version
                 FROM beacons ORDER BY timestamp DESC LIMIT ?1",
        ) {
            if let Ok(rows) = stmt.query_map(params![limit as i64], |row| {
                let ts: i64 = row.get(0)?;
                let agent_id: String = row.get(1)?;
                let agent_role: String = row.get(2)?;
                let hostname: String = row.get(3)?;
                let username: String = row.get(4)?;
                let os: String = row.get(5)?;
                let version: String = row.get(6)?;
                Ok(super::LogEntry {
                    timestamp: Self::format_ts(ts),
                    agent_id,
                    agent_role,
                    data: serde_json::json!({
                        "type": "beacon",
                        "hostname": hostname,
                        "username": username,
                        "os": os,
                        "version": version,
                    }),
                })
            }) {
                for row in rows.flatten() {
                    entries.push(row);
                }
            }
        }

        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        entries.truncate(limit);
        entries
    }

    pub fn agent_summary(&self) -> serde_json::Value {
        let mut agents = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT agent_id, agent_role, hostname, username, os, version, MAX(timestamp)
             FROM beacons GROUP BY agent_id ORDER BY MAX(timestamp) DESC",
        ) {
            if let Ok(rows) = stmt.query_map([], |row| {
                let agent_id: String = row.get(0)?;
                let agent_role: String = row.get(1)?;
                let hostname: String = row.get(2)?;
                let username: String = row.get(3)?;
                let os: String = row.get(4)?;
                let version: String = row.get(5)?;
                let last_seen: i64 = row.get(6)?;
                Ok(serde_json::json!({
                    "agent_id": agent_id,
                    "agent_role": agent_role,
                    "hostname": hostname,
                    "username": username,
                    "os": os,
                    "version": version,
                    "last_seen": Self::format_ts(last_seen),
                }))
            }) {
                for row in rows.flatten() {
                    agents.push(row);
                }
            }
        }
        serde_json::json!({ "agents": agents })
    }

    /// Métricas operativas REALES de la BD (ronda 12): contadores del
    /// estado del despliegue para operador/monitoring vía `GET
    /// /admin/metrics`. Nada simulado: todo sale de SQL sobre las tablas
    /// que alimentan los handlers existentes.
    pub fn metrics(&self) -> serde_json::Value {
        let count = |sql: &str| -> u64 {
            self.conn
                .query_row(sql, [], |r| r.get::<_, i64>(0))
                .unwrap_or(0) as u64
        };
        let agents_registered = count("SELECT COUNT(DISTINCT agent_id) FROM beacons");
        let beacons_total = count("SELECT COUNT(*) FROM beacons");
        let tasks_total = count("SELECT COUNT(*) FROM tasks");
        let tasks_pending = count("SELECT COUNT(*) FROM tasks WHERE claimed = 0");
        let tasks_claimed = count("SELECT COUNT(*) FROM tasks WHERE claimed = 1");
        let tasks_completed = count("SELECT COUNT(*) FROM tasks WHERE completed = 1");
        serde_json::json!({
            "agents_registered": agents_registered,
            "beacons_total": beacons_total,
            "tasks": {
                "total": tasks_total,
                "pending": tasks_pending,
                "claimed": tasks_claimed,
                "completed": tasks_completed,
            },
        })
    }

    fn format_ts(ts: i64) -> String {
        let dt: chrono::DateTime<chrono::Utc> =
            chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default();
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Db {
        fn with_conn(conn: Connection) -> Self {
            let db = Self { conn };
            db.migrate().expect("migrate");
            db
        }
    }

    #[test]
    fn beacon_retention_caps_table_size() {
        // Ronda 11: la tabla de beacons crecía sin límite. Con el tope a
        // MAX_BEACONS, insertar más de ese número conserva solo los más
        // recientes. Para no depender del valor absoluto, comprobamos la
        // invariante: tras N > tope inserciones, count == tope y quedan los
        // últimos (los antiguos desaparecen).
        let db = Db::with_conn(Connection::open_in_memory().unwrap());
        for i in 0..(MAX_BEACONS + 50) {
            db.record_beacon(
                &format!("agent-{i}"),
                "worker",
                "h",
                "u",
                "os",
                "v",
                &serde_json::json!({}),
            );
        }
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM beacons", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, MAX_BEACONS, "la retención recorta la tabla");
        // Insertamos MAX_BEACONS + 50 → se borran las 50 más antiguas: el
        // beacon más antiguo conservado es agent-50.
        let oldest: String = db
            .conn
            .query_row(
                "SELECT agent_id FROM beacons ORDER BY id ASC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(oldest, "agent-50");
    }

    #[test]
    fn pending_tasks_claim_is_scoped_to_agent() {
        // Ronda 11: dos agentes con el MISMO task_id — el claim de uno no
        // debe robar la tarea del otro.
        let db = Db::with_conn(Connection::open_in_memory().unwrap());
        db.push_task(
            "agent-a",
            "shared-task-id",
            "shell_exec",
            &serde_json::json!({"cmd": "echo a"}),
        );
        db.push_task(
            "agent-b",
            "shared-task-id",
            "shell_exec",
            &serde_json::json!({"cmd": "echo b"}),
        );

        let a_tasks = db.pending_tasks("agent-a");
        assert_eq!(a_tasks.len(), 1, "agent-a recibe su tarea");

        // agent-b NO está afectado por el claim de agent-a.
        let b_tasks = db.pending_tasks("agent-b");
        assert_eq!(
            b_tasks.len(),
            1,
            "la tarea de agent-b sigue pendiente (claim scoped por agent_id)"
        );
    }

    #[test]
    fn metrics_reflect_real_db_state() {
        // Ronda 12: /admin/metrics debe reflejar el estado REAL de la BD —
        // contadores coherentes con lo insertado, no valores fijos.
        let db = Db::with_conn(Connection::open_in_memory().unwrap());
        db.record_beacon(
            "agent-a",
            "worker",
            "h",
            "u",
            "os",
            "v",
            &serde_json::json!({}),
        );
        db.record_beacon(
            "agent-a",
            "worker",
            "h",
            "u",
            "os",
            "v",
            &serde_json::json!({}),
        );
        db.record_beacon(
            "agent-b",
            "drone",
            "h",
            "u",
            "os",
            "v",
            &serde_json::json!({}),
        );
        db.push_task("agent-a", "t1", "shell_exec", &serde_json::json!({}));
        db.push_task("agent-a", "t2", "shell_exec", &serde_json::json!({}));
        let _ = db.pending_tasks("agent-a"); // reclama t1 y t2
        db.push_task("agent-b", "t3", "shell_exec", &serde_json::json!({}));

        let m = db.metrics();
        assert_eq!(m["agents_registered"], 2);
        assert_eq!(m["beacons_total"], 3);
        assert_eq!(m["tasks"]["total"], 3);
        assert_eq!(m["tasks"]["claimed"], 2);
        assert_eq!(m["tasks"]["pending"], 1);
        assert_eq!(m["tasks"]["completed"], 0);
    }
}
