use hive_base::{AgentIdentity, ConsensusEngine, HiveChamber, Message, Payload, Role, Value};
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};

const SCOUT_MODEL_ENC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/scout_classifier.onnx.enc"));

fn load_scout_model() -> Vec<u8> {
    let seed = b"SWARM_SCOUT_ONNX_V1_X7k2Mp9Q_n3R4sT8v";
    hive_base::decrypt_model(SCOUT_MODEL_ENC, seed.as_slice())
        .expect("Failed to decrypt scout model")
}

struct ScoutAgent {
    comms: HiveChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    scan_interval: Duration,
    heartbeat_interval: Duration,
}

impl ScoutAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        let comms = HiveChamber::connect(&identity, Role::Worker)
            .await
            .expect("Failed to connect to colmena arena");

        info!("Scout connected to shared-memory arena");

        Self {
            comms,
            identity,
            consensus: ConsensusEngine::new(0.66),
            scan_interval: Duration::from_secs(15),
            heartbeat_interval: Duration::from_secs(10),
        }
    }

    async fn collect_system_profile(&self) -> Vec<(String, Value, f32)> {
        let mut beliefs = Vec::new();

        beliefs.push(("os_type".to_string(), Value::String(std::env::consts::OS.to_string()), 1.0));
        beliefs.push(("arch".to_string(), Value::String(std::env::consts::ARCH.to_string()), 1.0));

        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
        beliefs.push(("hostname".to_string(), Value::String(hostname), 1.0));

        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        beliefs.push(("user".to_string(), Value::String(user), 0.9));

        let edr_raw = check_edr_indicators_raw();
        let backup_raw = check_backup_indicators_raw();

        let process_count = get_process_count() as f32;
        let network_interfaces = get_interface_count() as f32;

        let mut is_edr = edr_raw;
        let mut is_backup = backup_raw;

        // ONNX model skipped (ort crate incompatibility with skl2onnx models)
        // Heuristic detection is used instead — works correctly

        beliefs.push(("edr_present".to_string(), Value::Bool(is_edr), 0.95));
        beliefs.push(("backup_present".to_string(), Value::Bool(is_backup), 0.90));
        beliefs.push(("network_interfaces".to_string(), Value::Int(get_interface_count() as i64), 0.95));
        beliefs.push(("process_count".to_string(), Value::Int(get_process_count() as i64), 0.9));

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
            self.comms.publish(msg).await;
        }
    }

    async fn send_heartbeat(&self) {
        self.comms.send_heartbeat().await;
    }

    async fn process_incoming(&mut self) {
        let messages = self.comms.read_new().await;
        for msg in messages {
            self.consensus.process_message(&msg);
            match &msg.payload {
                Payload::Request { service, payload: _ } if service == "scan" => {
                    info!("Received scan request");
                    let beliefs = self.collect_system_profile().await;
                    self.publish_beliefs(&beliefs).await;
                }
                Payload::Belief { asset, value, confidence } => {
                    info!("Belief from {}: {} = {:?} ({})", msg.agent_role, asset, value, confidence);
                }
                Payload::StatusEvent { event_type, subject_id, .. } if event_type == "agent_dead" => {
                    warn!("Agent {} reported DEAD", subject_id);
                }
                _ => {}
            }
        }
    }

    async fn check_dead_agents(&self) {
        let dead = self.comms.check_dead_agents(30).await;
        for agent_id in dead {
            let msg = Message::status_event(
                self.identity.id(),
                Role::Worker,
                "agent_dead",
                agent_id,
                Role::Worker,
                "no heartbeat detected",
            );
            self.comms.publish(msg).await;
        }
    }

    async fn run(&mut self) {
        info!("Swarm-Scout starting | ID: {}", self.identity.id());
        self.send_heartbeat().await;

        let mut heartbeat_timer = time::interval(self.heartbeat_interval);
        let mut scan_timer = time::interval(self.scan_interval);

        loop {
            tokio::select! {
                _ = heartbeat_timer.tick() => {
                    self.send_heartbeat().await;
                    self.check_dead_agents().await;
                }
                _ = scan_timer.tick() => {
                    let beliefs = self.collect_system_profile().await;
                    self.publish_beliefs(&beliefs).await;
                }
                _ = time::sleep(Duration::from_millis(200)) => {
                    self.process_incoming().await;
                }
            }
        }
    }
}

fn check_edr_indicators_raw() -> bool {
    let edr_processes = [
        "csfalcon", "csagent", "msmpeng", "sentinelone",
        "carbonblack", "cylancesvc", "symantec", "mcafee",
    ];
    let running = get_running_processes();
    edr_processes.iter().any(|&p| running.iter().any(|r| r.to_lowercase().contains(p)))
}

fn check_backup_indicators_raw() -> bool {
    let backup_processes = ["veeam", "backup_exec", "commvault", "netbackup", "backup_agent", "vss"];
    let running = get_running_processes();
    backup_processes.iter().any(|&p| running.iter().any(|r| r.to_lowercase().contains(p)))
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
    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        entries.count()
    } else { 0 }
}

fn get_process_count() -> usize {
    if let Ok(entries) = std::fs::read_dir("/proc") {
        entries.filter_map(|e| e.ok()).filter(|e| e.path().join("comm").exists()).count()
    } else { 0 }
}

use uuid::Uuid;

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("worker");
    info!("Initializing Swarm-Scout...");
    let mut scout = ScoutAgent::new().await;
    scout.run().await;
}
