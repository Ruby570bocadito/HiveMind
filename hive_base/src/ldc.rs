use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Worker,
    Weaver,
    Drone,
    Honeybee,
    Queen,
    Swarm,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Worker => write!(f, "worker"),
            Role::Weaver => write!(f, "weaver"),
            Role::Drone => write!(f, "drone"),
            Role::Honeybee => write!(f, "honeybee"),
            Role::Queen => write!(f, "queen"),
            Role::Swarm => write!(f, "swarm"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Bool(bool),
    String(String),
    Int(i64),
    Float(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    Support,
    Reject,
    Abstain,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Belief {
        asset: String,
        value: Value,
        confidence: f32,
    },
    Desire {
        action: String,
        priority: f32,
    },
    Proposal {
        action: String,
        argument: String,
        proposal_id: Uuid,
    },
    Vote {
        proposal_id: Uuid,
        decision: Decision,
        weight: f32,
    },
    Request {
        service: String,
        payload: Vec<u8>,
    },
    Query {
        dilemma: String,
        context: String,
        query_id: Uuid,
    },
    Response {
        query_id: Uuid,
        answer: String,
        confidence: f32,
    },
    Heartbeat,
    // Swarm status messages (for dead agent detection, etc.)
    StatusEvent {
        event_type: String,
        subject_id: Uuid,
        subject_role: Role,
        detail: String,
    },
}

/// Signed message for wire transport.
/// The `signature` covers `payload_bytes` (rmp-serialized Message).
#[derive(Debug, Clone)]
pub struct SignedMessage {
    pub agent_id: Uuid,
    pub verifying_key: [u8; 32],
    pub signature: [u8; 64],
    pub payload_bytes: Vec<u8>,
}

/// Unsigned message that gets serialized for signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub agent_id: Uuid,
    pub agent_role: Role,
    pub timestamp: u64,
    pub payload: Payload,
}

impl Message {
    pub fn to_signed_bytes(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }

    pub fn from_signed_message(signed: &SignedMessage) -> Option<Self> {
        rmp_serde::from_slice(&signed.payload_bytes).ok()
    }

    pub fn heartbeat(agent_id: Uuid, agent_role: Role) -> Self {
        Self {
            agent_id,
            agent_role,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: Payload::Heartbeat,
        }
    }

    pub fn belief(agent_id: Uuid, agent_role: Role, asset: String, value: Value, confidence: f32) -> Self {
        Self {
            agent_id,
            agent_role,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: Payload::Belief { asset, value, confidence },
        }
    }

    pub fn proposal(agent_id: Uuid, agent_role: Role, action: String, argument: String) -> (Self, Uuid) {
        let proposal_id = Uuid::new_v4();
        let msg = Self {
            agent_id,
            agent_role,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: Payload::Proposal {
                action,
                argument,
                proposal_id,
            },
        };
        (msg, proposal_id)
    }

    pub fn vote(agent_id: Uuid, agent_role: Role, proposal_id: Uuid, decision: Decision, weight: f32) -> Self {
        Self {
            agent_id,
            agent_role,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: Payload::Vote {
                proposal_id,
                decision,
                weight,
            },
        }
    }

    pub fn status_event(agent_id: Uuid, agent_role: Role, event_type: &str, subject_id: Uuid, subject_role: Role, detail: &str) -> Self {
        Self {
            agent_id,
            agent_role,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: Payload::StatusEvent {
                event_type: event_type.to_string(),
                subject_id,
                subject_role,
                detail: detail.to_string(),
            },
        }
    }
}
