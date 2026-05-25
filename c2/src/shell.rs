use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use crate::AppState;

#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub agent_id: String,
    pub created_at: i64,
    pub last_activity: i64,
    pub alive: bool,
}

type RelayMap = Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>;

pub async fn handle_shell(ws: WebSocket, state: AppState, session_id: String) {
    let (ws_sender, mut ws_receiver) = ws.split();
    let relay_map: RelayMap = Arc::new(Mutex::new(HashMap::new()));

    let agent_id = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            let ident: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            let agent = ident
                .get("agent_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            info!(agent = %agent, session = %session_id, "Operator connected");
            agent.to_string()
        }
        _ => {
            warn!(session = %session_id, "Session closed without identification");
            return;
        }
    };

    {
        let mut sessions = state.sessions.lock().await;
        sessions.register(&session_id, &agent_id);
    }

    let (output_tx, mut output_rx) = mpsc::channel::<String>(256);

    {
        let mut map = relay_map.lock().await;
        map.insert(session_id.clone(), output_tx);
    }

    let relay_map_clone = relay_map.clone();
    let session_id_clone = session_id.clone();

    let ws_send_task = tokio::spawn(async move {
        let mut ws_sender: SplitSink<WebSocket, Message> = ws_sender;
        loop {
            tokio::select! {
                Some(msg) = output_rx.recv() => {
                    if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    if ws_sender.send(Message::Ping(vec![])).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let ws_recv_task = tokio::spawn(async move {
        loop {
            match ws_receiver.next().await {
                Some(Ok(Message::Text(text))) => {
                    // Forward to agent if there's a connection
                    let map = relay_map_clone.lock().await;
                    if let Some(tx) = map.get(&session_id_clone) {
                        let _ = tx.send(text).await;
                    }
                }
                Some(Ok(Message::Close(_))) | None => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = ws_send_task => {},
        _ = ws_recv_task => {},
    }

    {
        let mut map = relay_map.lock().await;
        map.remove(&session_id);
    }

    {
        let mut sessions = state.sessions.lock().await;
        sessions.unregister(&session_id);
    }

    info!(session = %session_id, "Shell session closed");
}
