// Swarming: decisiones internas de la colonia sobre división de carga.
//
// Ronda 6: la migración real entre hosts (SSH + harvesting de claves) fue
// ELIMINADA junto con los módulos `lateral` (exec_ssh) y `leech`. Lo que
// queda es la lógica de decisión/umbral y la señal interna `swarm_initiate`
// (mensaje LdC de la arena), sin ningún transporte de propagación.

use crate::ldc::{Message, Role};
use tracing::info;
use uuid::Uuid;

/// Swarm configuration.
pub struct SwarmConfig {
    pub max_agents_per_host: usize,
    pub migration_threshold: usize, // agents before considering split
    pub swarm_cooldown_secs: u64,   // wait between swarms
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_agents_per_host: 6,
            migration_threshold: 5,
            swarm_cooldown_secs: 300,
        }
    }
}

/// Result of a swarm decision.
pub struct SwarmResult {
    pub success: bool,
    pub new_host: String,
    pub migrated_agents: usize,
    pub reason: String,
}

/// Evalúa si la colonia alcanzaría el umbral de división en `target_host`.
///
/// Ronda 6: NO ejecuta ninguna migración (no hay transporte de propagación en
/// el build). Solo comprueba umbrales y el gate BRAIN, y devuelve la decisión
/// documental para que la telemetría/consenso la registren.
pub fn evaluate_swarm(
    current_agent_count: usize,
    target_host: &str,
    config: &SwarmConfig,
) -> Option<SwarmResult> {
    if current_agent_count < config.migration_threshold {
        return None;
    }

    // Check if target is safe
    let hive_cfg = crate::config::HiveConfig::load();
    if crate::panal::is_safe_target(target_host, &hive_cfg.brain) {
        return Some(SwarmResult {
            success: false,
            new_host: target_host.to_string(),
            migrated_agents: 0,
            reason: "Target is in safe_ips".into(),
        });
    }

    let half = current_agent_count / 2;

    info!(
        "SWARMING: umbral de división alcanzado — decisión documental: {} agentes migrarían a {} (sin transporte de propagación en el build)",
        half, target_host
    );

    Some(SwarmResult {
        success: false,
        new_host: target_host.to_string(),
        migrated_agents: half,
        reason: "decisional only: no propagation transport exists (ronda 6)".into(),
    })
}

/// Signal the colony to prepare for swarming.
pub fn signal_swarm(queen_id: Uuid, target: &str, reason: &str) -> Message {
    Message::status_event(
        queen_id,
        Role::Queen,
        "swarm_initiate",
        Uuid::new_v4(),
        Role::Swarm,
        &format!("Migrate to {}: {}", target, reason),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_config_default() {
        let cfg = SwarmConfig::default();
        assert_eq!(cfg.max_agents_per_host, 6);
        assert_eq!(cfg.migration_threshold, 5);
    }

    #[test]
    fn test_swarm_below_threshold() {
        let cfg = SwarmConfig::default();
        assert!(evaluate_swarm(3, "10.0.0.1", &cfg).is_none());
    }

    #[test]
    fn test_swarm_decisional_never_succeeds() {
        // Ronda 6: la decisión nunca implica migración real.
        let cfg = SwarmConfig::default();
        if let Some(r) = evaluate_swarm(10, "10.99.99.99", &cfg) {
            assert!(!r.success);
            assert!(r.reason.contains("no propagation transport"));
        }
    }

    #[test]
    fn test_signal_swarm_creates_status_event() {
        let queen = Uuid::new_v4();
        let msg = signal_swarm(queen, "10.0.0.2", "host full");
        assert_eq!(msg.agent_id, queen);
        assert_eq!(msg.agent_role, Role::Queen);
    }
}
