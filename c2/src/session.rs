use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct ShellSession {
    pub session_id: String,
    pub agent_id: String,
    pub created_at: i64,
    pub last_activity: i64,
}

impl ShellSession {
    pub fn new(session_id: &str, agent_id: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Self {
            session_id: session_id.to_string(),
            agent_id: agent_id.to_string(),
            created_at: now,
            last_activity: now,
        }
    }

    #[allow(dead_code)]
    pub fn touch(&mut self) {
        self.last_activity = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
    }
}

#[derive(Default)]
pub struct SessionManager {
    sessions: HashMap<String, ShellSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn register(&mut self, session_id: &str, agent_id: &str) {
        self.sessions.insert(
            session_id.to_string(),
            ShellSession::new(session_id, agent_id),
        );
    }

    pub fn unregister(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    #[allow(dead_code)]
    pub fn touch(&mut self, session_id: &str) {
        if let Some(s) = self.sessions.get_mut(session_id) {
            s.touch();
        }
    }

    #[allow(dead_code)]
    pub fn get(&self, session_id: &str) -> Option<&ShellSession> {
        self.sessions.get(session_id)
    }

    pub fn list(&self) -> Vec<super::shell::SessionInfo> {
        self.sessions
            .values()
            .map(|s| super::shell::SessionInfo {
                session_id: s.session_id.clone(),
                agent_id: s.agent_id.clone(),
                created_at: s.created_at,
                last_activity: s.last_activity,
                alive: true,
            })
            .collect()
    }
}
