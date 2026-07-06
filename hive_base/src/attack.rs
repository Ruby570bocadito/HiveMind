// MITRE ATT&CK Enterprise technique mapping for Swarm operations.
// Every action the swarm takes is tagged with its corresponding
// ATT&CK technique ID for professional Red Team reporting.
//
// Reference: MITRE ATT&CK v15 (Enterprise Matrix)
// https://attack.mitre.org/

use serde::{Deserialize, Serialize};

// ── Tactic enum ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tactic {
    Reconnaissance,       // TA0043
    ResourceDevelopment,  // TA0042
    InitialAccess,        // TA0001
    Execution,            // TA0002
    Persistence,          // TA0003
    PrivilegeEscalation,  // TA0004
    DefenseEvasion,       // TA0005
    CredentialAccess,     // TA0006
    Discovery,            // TA0007
    LateralMovement,      // TA0008
    Collection,           // TA0009
    CommandAndControl,    // TA0011
    Exfiltration,         // TA0010
    Impact,               // TA0040
}

impl Tactic {
    pub fn id(&self) -> &'static str {
        match self {
            Tactic::Reconnaissance => "TA0043",
            Tactic::ResourceDevelopment => "TA0042",
            Tactic::InitialAccess => "TA0001",
            Tactic::Execution => "TA0002",
            Tactic::Persistence => "TA0003",
            Tactic::PrivilegeEscalation => "TA0004",
            Tactic::DefenseEvasion => "TA0005",
            Tactic::CredentialAccess => "TA0006",
            Tactic::Discovery => "TA0007",
            Tactic::LateralMovement => "TA0008",
            Tactic::Collection => "TA0009",
            Tactic::CommandAndControl => "TA0011",
            Tactic::Exfiltration => "TA0010",
            Tactic::Impact => "TA0040",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Tactic::Reconnaissance => "Reconnaissance",
            Tactic::ResourceDevelopment => "Resource Development",
            Tactic::InitialAccess => "Initial Access",
            Tactic::Execution => "Execution",
            Tactic::Persistence => "Persistence",
            Tactic::PrivilegeEscalation => "Privilege Escalation",
            Tactic::DefenseEvasion => "Defense Evasion",
            Tactic::CredentialAccess => "Credential Access",
            Tactic::Discovery => "Discovery",
            Tactic::LateralMovement => "Lateral Movement",
            Tactic::Collection => "Collection",
            Tactic::CommandAndControl => "Command and Control",
            Tactic::Exfiltration => "Exfiltration",
            Tactic::Impact => "Impact",
        }
    }
}

// ── Technique catalog ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Technique {
    pub id: &'static str,
    pub name: &'static str,
    pub tactic: Tactic,
    pub description: &'static str,
    pub swarm_module: &'static str,
    pub detection_difficulty: &'static str,
}

/// Complete catalog of swarm-mapped ATT&CK techniques.
pub const TECHNIQUES: &[Technique] = &[
    // ── Defense Evasion ──────────────────────────────────────────────────
    Technique {
        id: "T1055.012", name: "Process Hollowing",
        tactic: Tactic::DefenseEvasion,
        description: "Dropper spawns agents via memfd/fileless injection",
        swarm_module: "fileless::MemfdBinary",
        detection_difficulty: "High",
    },
    Technique {
        id: "T1562.001", name: "Disable or Modify Tools",
        tactic: Tactic::DefenseEvasion,
        description: "Syscall hooks bypass via direct kernel invocation",
        swarm_module: "syscalls",
        detection_difficulty: "Very High",
    },
    Technique {
        id: "T1622", name: "Debugger Evasion",
        tactic: Tactic::DefenseEvasion,
        description: "Anti-debug checks via ptrace and /proc/self/status",
        swarm_module: "anti_analysis",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1497.001", name: "System Checks - Sandbox",
        tactic: Tactic::DefenseEvasion,
        description: "Sandbox detection via uptime, CPU, RAM, username checks",
        swarm_module: "anti_analysis",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1497.003", name: "Time Based Evasion",
        tactic: Tactic::DefenseEvasion,
        description: "Random delays at agent init (1-10s) to evade sandbox timers",
        swarm_module: "utils::safe_init",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1027.002", name: "Software Packing",
        tactic: Tactic::DefenseEvasion,
        description: "ONNX models XOR-encrypted at build time",
        swarm_module: "crypto::decrypt_model",
        detection_difficulty: "High",
    },
    Technique {
        id: "T1027.005", name: "Indicator Removal from Tools",
        tactic: Tactic::DefenseEvasion,
        description: "Shared memory IPC avoids TCP listening sockets",
        swarm_module: "shared_arena",
        detection_difficulty: "Very High",
    },
    Technique {
        id: "T1070.004", name: "File Deletion",
        tactic: Tactic::DefenseEvasion,
        description: "Dropper self-destructs after deployment",
        swarm_module: "dropper",
        detection_difficulty: "Low",
    },
    Technique {
        id: "T1564.004", name: "Hidden File System - Memory",
        tactic: Tactic::DefenseEvasion,
        description: "memfd_create stores agent binaries in RAM only",
        swarm_module: "fileless",
        detection_difficulty: "Very High",
    },
    Technique {
        id: "T1055", name: "Process Injection (Polymorphic)",
        tactic: Tactic::DefenseEvasion,
        description: "Weaver XOR-mutates agent binaries before regeneration",
        swarm_module: "weaver",
        detection_difficulty: "High",
    },

    // ── Discovery ─────────────────────────────────────────────────────────
    Technique {
        id: "T1082", name: "System Information Discovery",
        tactic: Tactic::Discovery,
        description: "Scout collects OS, hostname, user, process list",
        swarm_module: "scout::collect_system_profile",
        detection_difficulty: "Low",
    },
    Technique {
        id: "T1057", name: "Process Discovery",
        tactic: Tactic::Discovery,
        description: "Scout reads /proc to enumerate running processes",
        swarm_module: "scout::get_running_processes",
        detection_difficulty: "Low",
    },
    Technique {
        id: "T1046", name: "Network Service Discovery",
        tactic: Tactic::Discovery,
        description: "Discover live hosts on local subnet via nmap/ARP",
        swarm_module: "lateral::discover_hosts",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1518.001", name: "Security Software Discovery",
        tactic: Tactic::Discovery,
        description: "Scout detects EDR processes (CrowdStrike, Defender, etc.)",
        swarm_module: "scout::check_edr_indicators",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1614.001", name: "System Location Discovery",
        tactic: Tactic::Discovery,
        description: "Anti-analysis checks for VM (DMI, CPUID, kernel modules)",
        swarm_module: "anti_analysis::check_vm",
        detection_difficulty: "Medium",
    },

    // ── Credential Access ─────────────────────────────────────────────────
    Technique {
        id: "T1552.001", name: "Credentials from Files (SSH Keys)",
        tactic: Tactic::CredentialAccess,
        description: "Harvest SSH private keys from ~/.ssh",
        swarm_module: "lateral::harvest_credentials",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1552.004", name: "Credentials from Files (Cloud Keys)",
        tactic: Tactic::CredentialAccess,
        description: "Harvest AWS/Azure/GCP keys from env vars and config files",
        swarm_module: "lateral::harvest_credentials",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1552.002", name: "Credentials in Files (.bash_history)",
        tactic: Tactic::CredentialAccess,
        description: "Search command history for passwords and tokens",
        swarm_module: "lateral::harvest_credentials",
        detection_difficulty: "Low",
    },

    // ── Lateral Movement ──────────────────────────────────────────────────
    Technique {
        id: "T1021.004", name: "Remote Services: SSH",
        tactic: Tactic::LateralMovement,
        description: "Execute commands on remote hosts via SSH",
        swarm_module: "lateral::exec_ssh",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1570", name: "Lateral Tool Transfer (SCP)",
        tactic: Tactic::LateralMovement,
        description: "Deploy agent binary to remote host via SCP",
        swarm_module: "lateral::deploy_agent_scp",
        detection_difficulty: "High",
    },
    Technique {
        id: "T1021.006", name: "Remote Services: WinRM",
        tactic: Tactic::LateralMovement,
        description: "Remote execution via WinRM (placeholder)",
        swarm_module: "lateral::exec_winrm",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1047", name: "Remote Services: WMI",
        tactic: Tactic::LateralMovement,
        description: "Remote execution via WMI (placeholder)",
        swarm_module: "lateral::exec_wmi",
        detection_difficulty: "Medium",
    },

    // ── Collection ────────────────────────────────────────────────────────
    Technique {
        id: "T1005", name: "Data from Local System",
        tactic: Tactic::Collection,
        description: "Hoarder collects target data before exfiltration",
        swarm_module: "honeybee",
        detection_difficulty: "Medium",
    },

    // ── Command and Control ───────────────────────────────────────────────
    Technique {
        id: "T1573.002", name: "Encrypted Channel (ChaCha20)",
        tactic: Tactic::CommandAndControl,
        description: "Inter-agent messages encrypted with Ed25519 + ChaCha20",
        swarm_module: "crypto",
        detection_difficulty: "Very High",
    },
    Technique {
        id: "T1090.004", name: "Proxy: CDN Fronting",
        tactic: Tactic::CommandAndControl,
        description: "HTTP beacons disguised as Google/Fonts CDN traffic",
        swarm_module: "exfil::http_beacon",
        detection_difficulty: "Very High",
    },
    Technique {
        id: "T1572", name: "Protocol Tunneling (DNS)",
        tactic: Tactic::CommandAndControl,
        description: "Data exfiltrated via DNS query chains",
        swarm_module: "exfil::dns_exfiltrate",
        detection_difficulty: "High",
    },
    Technique {
        id: "T1571", name: "Non-Standard Port",
        tactic: Tactic::CommandAndControl,
        description: "No TCP ports used - all IPC via shared memory",
        swarm_module: "shared_arena",
        detection_difficulty: "Very High",
    },

    // ── Exfiltration ──────────────────────────────────────────────────────
    Technique {
        id: "T1048.003", name: "Exfiltration Over DNS",
        tactic: Tactic::Exfiltration,
        description: "Data exfiltrated via encoded DNS queries",
        swarm_module: "exfil::dns_exfiltrate",
        detection_difficulty: "High",
    },
    Technique {
        id: "T1048.002", name: "Exfiltration Over HTTP",
        tactic: Tactic::Exfiltration,
        description: "Data exfiltrated via CDN-camouflaged HTTP GET requests",
        swarm_module: "exfil::http_beacon",
        detection_difficulty: "Very High",
    },
    Technique {
        id: "T1029", name: "Scheduled Transfer",
        tactic: Tactic::Exfiltration,
        description: "Exfiltration scheduled during business hours only",
        swarm_module: "exfil::ExfilScheduler",
        detection_difficulty: "High",
    },

    // ── Execution ─────────────────────────────────────────────────────────
    Technique {
        id: "T1204.002", name: "User Execution: Malicious File (Dropper)",
        tactic: Tactic::Execution,
        description: "Dropper embeds and deploys all swarm agents",
        swarm_module: "dropper",
        detection_difficulty: "Low",
    },
    Technique {
        id: "T1106", name: "Native API (Syscalls)",
        tactic: Tactic::Execution,
        description: "Direct syscalls bypass libc for process operations",
        swarm_module: "syscalls",
        detection_difficulty: "Very High",
    },

    // ── Persistence ───────────────────────────────────────────────────────
    Technique {
        id: "T1543.002", name: "Create or Modify System Process (systemd)",
        tactic: Tactic::Persistence,
        description: "Shaper regenerates killed agents maintaining swarm presence",
        swarm_module: "shaper::regenerate_agent",
        detection_difficulty: "Medium",
    },
    Technique {
        id: "T1547.001", name: "Boot or Logon Autostart (reboot survival)",
        tactic: Tactic::Persistence,
        description: "Shaper can install boot persistence via cron/systemd",
        swarm_module: "drone",
        detection_difficulty: "Medium",
    },

    // ── Impact ────────────────────────────────────────────────────────────
    Technique {
        id: "T1485", name: "Data Destruction (Hoarder)",
        tactic: Tactic::Impact,
        description: "Hoarder can execute encrypt/destroy actions after consensus",
        swarm_module: "honeybee",
        detection_difficulty: "High",
    },
];

// ── Search and reporting utilities ───────────────────────────────────────────

/// Find all techniques matching a swarm module name.
pub fn techniques_by_module(module: &str) -> Vec<&'static Technique> {
    TECHNIQUES.iter()
        .filter(|t| t.swarm_module.to_lowercase().contains(&module.to_lowercase()))
        .collect()
}

/// Find all techniques for a given tactic.
pub fn techniques_by_tactic(tactic: Tactic) -> Vec<&'static Technique> {
    TECHNIQUES.iter()
        .filter(|t| t.tactic == tactic)
        .collect()
}

/// Generate an ATT&CK coverage report.
pub fn generate_coverage_report() -> String {
    let mut report = String::new();
    report.push_str("=== MITRE ATT&CK Coverage Report ===\n\n");

    let tactics = [
        Tactic::DefenseEvasion, Tactic::Discovery, Tactic::CredentialAccess,
        Tactic::LateralMovement, Tactic::CommandAndControl, Tactic::Exfiltration,
        Tactic::Execution, Tactic::Persistence, Tactic::Collection, Tactic::Impact,
    ];

    let mut total = 0;
    for tactic in &tactics {
        let techs = techniques_by_tactic(*tactic);
        if !techs.is_empty() {
            report.push_str(&format!("{} ({}):\n", tactic.name(), tactic.id()));
            for t in &techs {
                report.push_str(&format!("  {} - {} [{}.detection: {}]\n",
                    t.id, t.name, t.swarm_module, t.detection_difficulty));
            }
            report.push_str(&format!("  Total: {} techniques\n\n", techs.len()));
            total += techs.len();
        }
    }

    report.push_str(&format!("Overall: {} ATT&CK techniques covered\n", total));

    // Score: techniques covered out of ~200
    let coverage_pct = (total as f32 / 200.0 * 100.0).min(100.0);
    report.push_str(&format!("Coverage: {:.1}% of enterprise matrix\n", coverage_pct));

    report
}

/// Tag an action with its ATT&CK technique for reporting.
pub fn tag_action(module: &str, _action: &str) -> Option<&'static Technique> {
    TECHNIQUES.iter()
        .find(|t| t.swarm_module.to_lowercase().contains(&module.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_non_empty() {
        let report = generate_coverage_report();
        assert!(report.contains("ATT&CK"));
        assert!(report.len() > 500);
    }

    #[test]
    fn test_find_technique() {
        // First technique with "exfil::dns_exfiltrate" in module path is T1572 (DNS Tunneling)
        let t = tag_action("exfil::dns_exfiltrate", "send_dns");
        assert!(t.is_some(), "Should find DNS technique");
        assert_eq!(t.unwrap().id, "T1572");
    }

    #[test]
    fn test_tactics_have_techniques() {
        let evasion = techniques_by_tactic(Tactic::DefenseEvasion);
        assert!(evasion.len() >= 5, "Should have at least 5 evasion techniques");
    }

    #[test]
    fn test_all_ids_unique() {
        use std::collections::HashSet;
        let ids: HashSet<_> = TECHNIQUES.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), TECHNIQUES.len(), "All technique IDs must be unique");
    }
}
