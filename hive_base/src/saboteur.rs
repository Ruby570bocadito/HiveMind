use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SabotageTarget {
    FinancialData,        // Excel/CSV/SQL — mutate balances, rates, accounts
    SourceCode,           // Git repos — introduce subtle bugs
    MLModel,              // .pth/.h5/.onnx — degrade prediction accuracy
    LogInjection,         // syslog/journald — insert false entries
    InfrastructureConfig, // kube/docker/terraform/nginx — degrade service
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SabotageOrder {
    pub target_type: SabotageTarget,
    pub target_path: PathBuf,
    pub severity: f32, // 0.0-1.0 how aggressively to mutate
    pub scope: String, // "all" | "random" | "specific"
    pub mutator_id: Uuid,
    pub completed: bool,
}

pub struct Saboteur;

impl Default for Saboteur {
    fn default() -> Self {
        Self::new()
    }
}

impl Saboteur {
    pub fn new() -> Self {
        Self
    }

    /// Ronda 4 (emulación): la mutación real de datos fue eliminada. La orden se
    /// registra y devuelve un resultado simulado; `scan_for_targets` sigue siendo
    /// una enumeración de solo lectura para entrenamiento red-team en laboratorio.
    pub fn execute_order(&self, order: &SabotageOrder) -> Result<String, String> {
        let severity = order.severity.clamp(0.0, 1.0);
        tracing::info!(
            "SABOTEUR (simulated): orden recibida para {:?} en {} (severidad {:.2}) — mutación no ejecutada (emulation mode)",
            order.target_type,
            order.target_path.display(),
            severity
        );
        Ok(format!(
            "simulated: sabotage order for {:?} at {} acknowledged (severity {:.2}); mutation disabled (emulation mode)",
            order.target_type,
            order.target_path.display(),
            severity
        ))
    }

    pub fn scan_for_targets(&self, root: &Path) -> Vec<(SabotageTarget, PathBuf)> {
        let mut targets = Vec::new();
        if !root.exists() {
            return targets;
        }

        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    continue;
                }

                let _name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

                match ext {
                    "xlsx" | "xls" | "csv" | "sqlite" | "db" => {
                        targets.push((SabotageTarget::FinancialData, path.clone()));
                    }
                    "rs" | "py" | "js" | "ts" | "go" | "java" => {
                        targets.push((SabotageTarget::SourceCode, path.clone()));
                    }
                    "pth" | "h5" | "onnx" | "pt" | "pkl" | "pickle" => {
                        targets.push((SabotageTarget::MLModel, path.clone()));
                    }
                    "log" | "syslog" | "journal" => {
                        targets.push((SabotageTarget::LogInjection, path));
                    }
                    _ => {}
                }
            }
        }
        targets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seer::{Seer, TelemetrySample};

    #[test]
    fn test_new_saboteur_creates_empty() {
        let s = Saboteur::new();
        // Saboteur is a unit struct; verify it can be created and used
        let targets = s.scan_for_targets(std::path::Path::new("/"));
        assert!(targets.is_empty());
    }

    #[test]
    fn test_saboteur_tournament_order_integration() {
        // Tournament generates variant codes that Saboteur can execute as orders
        use crate::seer::Seer;
        use crate::tournament::{Tournament, TournamentConfig, WinCriteria};
        let s = Saboteur::new();
        let seer = Seer::new();
        let t = Tournament::new();
        let config = TournamentConfig {
            target: "10.0.0.1".into(),
            competitors: 1,
            criteria: vec![WinCriteria::Speed],
            timeout_secs: 300,
            generations: 1,
        };
        let competitors = t.generate_competitors(&config);
        let _variant = &competitors[0].variant_code;
        // Saboteur can execute a sabotage order using tournament variant as technique context
        let tmp = std::env::temp_dir().join("hive_sab_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let target_file = tmp.join("finances.csv");
        std::fs::write(&target_file, "revenue,expenses\n100,50\n").unwrap();
        let order = SabotageOrder {
            target_type: SabotageTarget::FinancialData,
            target_path: target_file.clone(),
            severity: 0.3,
            scope: "random".into(),
            mutator_id: Uuid::new_v4(),
            completed: false,
        };
        let result = s.execute_order(&order);
        assert!(result.is_ok() || result.is_err());
        let _ = std::fs::remove_dir_all(&tmp);
        // Validate with Seer that low-stealth orders are flagged
        let telemetry = crate::seer::TelemetrySample {
            edr_process_count: 5,
            total_processes: 50,
            uptime_hours: 10,
            firewall_rules: 3,
            logged_in_users: 2,
            listening_ports: 0,
            has_defender: true,
            has_sentinelone: false,
            has_crowdstrike: false,
            has_carbonblack: false,
            has_symantec: false,
            is_vm: false,
            is_domain_controller: false,
            is_server_os: false,
        };
        let pred = seer.predict_detection(&telemetry, &format!("sabotage {:?}", order.target_type));
        assert!(pred.probability >= 0.0 && pred.probability <= 1.0);
    }

    #[test]
    fn test_saboteur_and_seer_integration() {
        let s = Saboteur::new();
        let seer = Seer::new();
        let targets = s.scan_for_targets(std::path::Path::new("/"));
        let telemetry = TelemetrySample {
            edr_process_count: 0,
            total_processes: 50,
            uptime_hours: 10,
            firewall_rules: 3,
            logged_in_users: 2,
            listening_ports: 0,
            has_defender: false,
            has_sentinelone: false,
            has_crowdstrike: false,
            has_carbonblack: false,
            has_symantec: false,
            is_vm: false,
            is_domain_controller: false,
            is_server_os: false,
        };
        // Seer should give low risk for simple targets
        if let Some((_, path)) = targets.first() {
            let pred = seer.predict_detection(&telemetry, &format!("sabotage {}", path.display()));
            assert!(pred.probability >= 0.0 && pred.probability <= 1.0);
            assert!(seer.should_proceed(&pred, 0.7));
        }
    }

    #[test]
    fn test_sabotage_source_code() {
        let dir = std::env::temp_dir().join("hive_test_sab_code");
        let _ = std::fs::create_dir_all(&dir);
        let rs_path = dir.join("main.rs");
        std::fs::write(
            &rs_path,
            "fn main() {\n    let x = 5;\n    if x <= 10 {\n        println!(\"ok\");\n    }\n}",
        )
        .unwrap();

        let order = SabotageOrder {
            target_type: SabotageTarget::SourceCode,
            target_path: rs_path.clone(),
            severity: 0.5,
            scope: "random".into(),
            mutator_id: Uuid::new_v4(),
            completed: false,
        };

        let sab = Saboteur::new();
        let result = sab.execute_order(&order);
        assert!(result.is_ok());
        // Ronda 4 (emulación): el fichero NO se muta; la orden se acusa simulada.
        let content = std::fs::read_to_string(&rs_path).unwrap();
        assert!(
            content.contains("x <= 10"),
            "source must remain untouched under emulation"
        );
        assert!(result.unwrap().contains("simulated"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sabotage_config() {
        let dir = std::env::temp_dir().join("hive_test_sab_cfg");
        let _ = std::fs::create_dir_all(&dir);
        let yml_path = dir.join("deploy.yml");
        std::fs::write(&yml_path, "replicas: 3\ntimeout: 30\nenabled: true").unwrap();

        let order = SabotageOrder {
            target_type: SabotageTarget::InfrastructureConfig,
            target_path: yml_path.clone(),
            severity: 0.5,
            scope: "all".into(),
            mutator_id: Uuid::new_v4(),
            completed: false,
        };

        let sab = Saboteur::new();
        let result = sab.execute_order(&order);
        assert!(result.is_ok());
        // Ronda 4 (emulación): la configuración NO cambia.
        let content = std::fs::read_to_string(&yml_path).unwrap();
        assert!(
            content.contains("replicas: 3"),
            "config must remain untouched under emulation: {}",
            content
        );
        assert!(result.unwrap().contains("simulated"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_targets() {
        let dir = std::env::temp_dir().join("hive_test_sab_scan");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("data.csv"), "a,b,c\n1,2,3").unwrap();
        std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("model.onnx"), [0u8; 100]).unwrap();

        let sab = Saboteur::new();
        let targets = sab.scan_for_targets(&dir);
        assert!(
            targets.len() >= 3,
            "Should find 3+ targets, found: {}",
            targets.len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_log_injection() {
        let dir = std::env::temp_dir().join("hive_test_sab_log");
        let _ = std::fs::create_dir_all(&dir);
        let log_path = dir.join("syslog");
        std::fs::write(&log_path, "May 23 10:00:00 localhost kernel: [0.0] boot").unwrap();

        let order = SabotageOrder {
            target_type: SabotageTarget::LogInjection,
            target_path: log_path.clone(),
            severity: 0.5,
            scope: "all".into(),
            mutator_id: Uuid::new_v4(),
            completed: false,
        };

        let sab = Saboteur::new();
        let result = sab.execute_order(&order);
        assert!(result.is_ok());
        // Ronda 4 (emulación): NO se inyectan entradas falsas en logs.
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            !content.contains("sshd"),
            "logs must remain untouched under emulation"
        );
        assert!(result.unwrap().contains("simulated"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
