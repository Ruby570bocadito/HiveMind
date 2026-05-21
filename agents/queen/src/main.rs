use hive_base::{AgentIdentity, ConsensusEngine, HiveChamber, Message, Payload, Role};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};

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
}

impl OvermindAgent {
    async fn new() -> Self {
        let identity = AgentIdentity::new();
        let comms = HiveChamber::connect(&identity, Role::Queen)
            .await
            .expect("Failed to connect to colmena arena");

        info!("Overmind connected to shared-memory arena");

        Self {
            comms, identity,
            consensus: ConsensusEngine::new(0.66),
            ollama_url: "http://localhost:11434".to_string(),
            model: "tinyllama".to_string(),
            heartbeat_interval: Duration::from_secs(10),
        }
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

        match client.post(format!("{}/api/generate", self.ollama_url)).json(&request).send().await {
            Ok(resp) => match resp.json::<OllamaResponse>().await {
                Ok(r) => {
                    info!("LLM: {}", r.response);
                    serde_json::from_str(&r.response).ok()
                }
                Err(e) => { warn!("Parse error: {}", e); None }
            },
            Err(e) => { warn!("Ollama unavailable at {}: {}", self.ollama_url, e); None }
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
            if let Payload::Query { dilemma, context, query_id } = &msg.payload {
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
        }
    }

    async fn run(&mut self) {
        info!("Swarm-Overmind starting | ID: {}", self.identity.id());
        info!("Ollama: {} | Model: {}", self.ollama_url, self.model);
        self.send_heartbeat().await;

        let mut heartbeat_timer = time::interval(self.heartbeat_interval);
        loop {
            tokio::select! {
                _ = heartbeat_timer.tick() => { self.send_heartbeat().await; }
                _ = time::sleep(Duration::from_millis(200)) => { self.process_incoming().await; }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    hive_base::utils::init_logging("queen");
    info!("Initializing Swarm-Overmind...");
    let mut overmind = OvermindAgent::new().await;
    overmind.run().await;
}
