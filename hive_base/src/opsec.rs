// OPSEC: Operational Security for beacon timing.
//
// Ronda 12 — honestidad de transporte: este módulo SOLO decide CUÁNDO
// actuar (jitter determinista, ventanas horarias, multiplicadores de fin
// de semana). Fue ELIMINADO aquí:
//
//   - "Decoy traffic": enviaba peticiones HTTP REALES a terceros
//     (crl.microsoft.com, ocsp.digicert.com, cdn.cloudflare.net,
//     settings-win.data.microsoft.com, v10.vortex-win.data.microsoft.com,
//     api-global.netflix.com) con User-Agents suplantados en cada ciclo
//     de heartbeat (probabilidad 0.3) — exactamente la clase de masquerade
//     que la ronda 11 eliminó de smoke_signals y que aquí sobrevivió.
//   - "Traffic mimicry": fábrica de User-Agents suplantados y selector de
//     canal encubierto a partir del perfil de nube observado.
//   - Anti-análisis en el gate de decisión: `evasion_check`
//     (sandbox/debugger/EDR → congelar beacons) retirado de `should_act`;
//     la colonia de laboratorio no altera su comportamiento según quién
//     la observa (la matriz de CAPABILITIES ya declaraba "sin
//     anti-análisis" — el código ahora lo cumple).

#![cfg_attr(not(test), deny(clippy::unwrap_used))]
use chrono::{Datelike, Timelike};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
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
            base_ms: 30_000,
            jitter_percent: 30,
            min_ms: 5_000,
            max_ms: 120_000,
        }
    }
}

impl JitterConfig {
    /// Create a config with a deterministic seed derived from agent id.
    pub fn with_seed(seed_bytes: &[u8]) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        seed_bytes.hash(&mut hasher);
        Self {
            seed: hasher.finish(),
            ..Default::default()
        }
    }

    /// Next delay with deterministic ±jitter_percent variation.
    pub fn next_delay(&self) -> Duration {
        let mut rng = if self.seed == 0 {
            StdRng::from_entropy()
        } else {
            // Seed mixed with the current minute: deterministic within the
            // minute (replay-consistent), varies between minutes.
            let minute = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() / 60)
                .unwrap_or(0);
            StdRng::seed_from_u64(self.seed ^ minute)
        };
        let spread = (self.base_ms as f64) * (self.jitter_percent as f64 / 100.0);
        let offset: f64 = rng.gen_range(-spread..=spread);
        let ms = (self.base_ms as f64 + offset).clamp(self.min_ms as f64, self.max_ms as f64);
        Duration::from_millis(ms as u64)
    }

    /// Whether the given last-activation timestamp is due for another action.
    pub fn is_due(&self, last_activation_ms: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        now.saturating_sub(last_activation_ms) >= self.base_ms
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

// ── unified OPSEC engine ─────────────────────────────────────────────────────

/// Unified OPSEC engine that orchestrates timing: jitter + schedule.
/// Ronda 12: sin decoys de red, sin mimicry de UAs, sin anti-análisis —
/// solo decisiones de CUÁNDO, nunca de QUÉ tráfico enviar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpsecEngine {
    pub jitter: JitterConfig,
    pub schedule: ActivitySchedule,
    /// When this engine was last activated (UNIX ms)
    pub last_activation: u64,
    /// Completed OPSEC cycles (accounting only — no traffic is sent)
    pub cycles_completed: u64,
}

impl OpsecEngine {
    /// Create a new engine with a seed derived from agent_id.
    pub fn new(agent_id: &[u8]) -> Self {
        Self {
            jitter: JitterConfig::with_seed(agent_id),
            schedule: ActivitySchedule::default(),
            last_activation: 0,
            cycles_completed: 0,
        }
    }

    /// Calibrate the activity schedule from a lab org profile (timing only).
    /// Ronda 12: antes también construía un `TrafficMimic` para elegir
    /// canales encubiertos y User-Agents suplantados — eliminado.
    pub fn calibrate(&mut self, profile: &crate::smoke_signals::OrgCloudProfile) {
        self.schedule.hourly_multipliers = default_hourly_from_profile(profile);
        info!("OPSEC: schedule calibrated from org profile (timing only)");
    }

    /// Whether the engine allows action right now.
    /// Ronda 12: SOLO el horario gobierna la decisión — el anti-análisis
    /// (sandbox/debugger/EDR → congelar) fue retirado de este gate.
    pub fn should_act(&self) -> bool {
        self.schedule.should_act(0.05)
    }

    /// Get the effective delay before the next action (jitter + schedule applied).
    pub fn next_delay(&self) -> Duration {
        let base = self.jitter.next_delay();
        let schedule_ms = self.schedule.effective_interval_ms(self.jitter.base_ms);
        // Use the longer of the two
        let ms = base.as_millis().max(schedule_ms as u128);
        Duration::from_millis(ms as u64)
    }

    /// Execute one OPSEC cycle: account the activation and return the delay
    /// before the next action. NO traffic is generated here (ronda 12).
    pub fn cycle(&mut self) -> Duration {
        self.last_activation = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if !self.should_act() {
            return Duration::from_secs(60); // Check again in 60s
        }

        self.cycles_completed += 1;

        // Return the jittered delay
        self.next_delay()
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
        let _d1 = j1.next_delay();
        let _d2 = j2.next_delay();
        // Very unlikely to be equal for different seeds
        // (run multiple times to verify statistically)
    }

    #[test]
    fn test_jitter_is_due() {
        let j = JitterConfig {
            base_ms: 1,
            ..Default::default()
        };
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
        if (8..=18).contains(&hour) {
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
    fn test_opsec_engine_new() {
        let engine = OpsecEngine::new(b"test-agent");
        assert_eq!(engine.cycles_completed, 0);
        assert!(engine.last_activation == 0);
    }

    #[test]
    fn test_opsec_engine_cycle() {
        let mut engine = OpsecEngine::new(b"test-agent");
        let delay = engine.cycle();
        assert!(delay > Duration::ZERO);
        assert!(engine.last_activation > 0);
    }

    // ── Ronda 12: regresiones de honestidad ─────────────────────────────

    #[test]
    fn test_engine_cycle_accounts_without_traffic() {
        // El ciclo contabiliza activaciones; no genera tráfico (el módulo
        // ya no tiene ningún constructor de peticiones HTTP).
        let mut engine = OpsecEngine::new(b"test-agent");
        let before = engine.cycles_completed;
        let _ = engine.cycle();
        let _ = engine.cycle();
        assert_eq!(engine.cycles_completed, before + 2);
    }

    #[test]
    fn test_should_act_is_schedule_only() {
        // should_act depende exclusivamente del horario: con un calendario
        // "siempre activo" la respuesta debe ser true en cualquier entorno
        // (antes un detect_sandbox/EDR podía congelar el beacon).
        let mut engine = OpsecEngine::new(b"test-agent");
        engine.schedule.hourly_multipliers = [1.0; 24];
        engine.schedule.weekend_multiplier = 1.0;
        assert!(engine.should_act());
    }

    #[test]
    fn test_calibrate_only_touches_schedule() {
        // La calibración ajusta el calendario horario y nada más — ya no
        // construye mimicry ni selecciona canales encubiertos.
        let mut engine = OpsecEngine::new(b"test");
        let profile = crate::smoke_signals::OrgCloudProfile {
            peak_hours: vec![9, 10, 11],
            ..Default::default()
        };
        engine.calibrate(&profile);
        assert_eq!(engine.schedule.hourly_multipliers[9], 1.0);
        assert_eq!(engine.schedule.hourly_multipliers[10], 1.0);
        assert_eq!(engine.schedule.hourly_multipliers[23], 0.1);
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
        let s = ActivitySchedule {
            weekend_multiplier: 0.0,
            ..ActivitySchedule::default()
        };
        // Force a weekend day
        for &day in &[0u8, 6u8] {
            let weekend = s.weekend_days.contains(&day);
            assert!(weekend);
        }
    }
}
