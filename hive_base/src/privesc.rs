use std::process::Command;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone)]
pub struct PrivEscVector {
    pub technique: String,
    pub binary: String,
    pub confidence: f32,
    pub description: String,
    pub mitre_id: &'static str,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone)]
pub struct PrivEscResult {
    pub success: bool,
    pub technique: String,
    pub root_shell: bool,
    pub new_uid: Option<u32>,
    pub output: String,
}

pub fn scan_privilege_escalation() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    vectors.extend(scan_suid_binaries());
    vectors.extend(scan_sudo_misconfigs());
    vectors.extend(scan_capabilities());
    vectors.extend(scan_writable_paths());
    vectors.extend(scan_cron_jobs());
    vectors.extend(scan_docker_group());
    vectors.extend(scan_nfs_shares());
    vectors.extend(scan_kernel_exploits());
    vectors.extend(scan_container_escapes());
    vectors.sort_by_key(|v| v.risk);
    info!("PRIVESC: found {} potential vectors", vectors.len());
    vectors
}

fn scan_suid_binaries() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    let known_exploitable: [(&str, &str, &str); 9] = [
        ("find", "T1548.001", "find . -exec /bin/sh -p \\; -quit"),
        (
            "vim",
            "T1548.001",
            "vim -c ':py3 import os; os.execl(\"/bin/sh\",\"sh\")'",
        ),
        ("bash", "T1548.001", "bash -p"),
        (
            "python",
            "T1548.001",
            "python -c 'import os; os.execl(\"/bin/sh\",\"sh\")'",
        ),
        ("perl", "T1548.001", "perl -e 'exec \"/bin/sh\";'"),
        ("less", "T1548.001", "less /etc/passwd → !/bin/sh"),
        ("awk", "T1548.001", "awk 'BEGIN {system(\"/bin/sh\")}'"),
        ("nmap", "T1548.001", "nmap --interactive → !sh"),
        ("systemctl", "T1543.002", "systemctl → !sh"),
    ];
    if let Ok(out) = Command::new("find")
        .args(["/", "-perm", "-4000", "-type", "f", "-ls", "2>/dev/null"])
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for (name, mitre, technique) in &known_exploitable {
            if text.contains(name) {
                vectors.push(PrivEscVector {
                    technique: format!("SUID {}", technique),
                    binary: name.to_string(),
                    confidence: 0.9,
                    description: format!("SUID {} — {}", name, technique),
                    mitre_id: mitre,
                    risk: RiskLevel::Low,
                });
            }
        }
    }
    vectors
}

fn scan_sudo_misconfigs() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    if let Ok(out) = Command::new("sudo").arg("-l").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("(ALL) NOPASSWD:") {
            vectors.push(PrivEscVector {
                technique: "sudo NOPASSWD".into(),
                binary: "sudo".into(),
                confidence: 1.0,
                description: "Sudo NOPASSWD — full root access".into(),
                mitre_id: "T1548.003",
                risk: RiskLevel::Low,
            });
        }
        if text.contains("(root) SETENV:") {
            vectors.push(PrivEscVector {
                technique: "LD_PRELOAD via SETENV".into(),
                binary: "sudo".into(),
                confidence: 0.7,
                description: "Sudo SETENV allows LD_PRELOAD injection".into(),
                mitre_id: "T1574.006",
                risk: RiskLevel::Medium,
            });
        }
    }
    if let Ok(out) = Command::new("groups").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("sudo") || text.contains("wheel") {
            vectors.push(PrivEscVector {
                technique: "sudo group membership".into(),
                binary: "sudo".into(),
                confidence: 0.5,
                description: "User is in sudo/wheel group".into(),
                mitre_id: "T1548.003",
                risk: RiskLevel::Low,
            });
        }
    }
    vectors
}

fn scan_capabilities() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    if let Ok(out) = Command::new("getcap")
        .arg("-r")
        .arg("/")
        .arg("2>/dev/null")
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("cap_setuid") || line.contains("cap_sys_admin") {
                vectors.push(PrivEscVector {
                    technique: format!("capability abuse: {}", line),
                    binary: line.split_whitespace().next().unwrap_or("?").to_string(),
                    confidence: 0.6,
                    description: "Dangerous capability found".into(),
                    mitre_id: "T1548.001",
                    risk: RiskLevel::Medium,
                });
            }
        }
    }
    vectors
}

fn scan_writable_paths() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    for path in &[
        "/etc/cron.hourly",
        "/etc/cron.daily",
        "/usr/local/bin",
        "/opt",
        "/tmp",
        "/dev/shm",
    ] {
        if let Ok(meta) = std::fs::metadata(path) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o002 != 0 {
                    vectors.push(PrivEscVector {
                        technique: format!("writable path: {}", path),
                        binary: path.to_string(),
                        confidence: 0.4,
                        description: format!("{} is world-writable", path),
                        mitre_id: "T1574.001",
                        risk: RiskLevel::Medium,
                    });
                }
            }
        }
    }
    vectors
}

fn scan_cron_jobs() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    for dir in &[
        "/etc/cron.hourly",
        "/etc/cron.daily",
        "/etc/cron.weekly",
        "/var/spool/cron/crontabs",
    ] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = entry.metadata() {
                        if meta.permissions().mode() & 0o002 != 0 {
                            vectors.push(PrivEscVector {
                                technique: format!("writable cron: {}", entry.path().display()),
                                binary: entry.path().display().to_string(),
                                confidence: 0.8,
                                description: "Writable cron job found".into(),
                                mitre_id: "T1053.003",
                                risk: RiskLevel::Medium,
                            });
                        }
                    }
                }
            }
        }
    }
    vectors
}

fn scan_docker_group() -> Vec<PrivEscVector> {
    if let Ok(out) = Command::new("groups").output() {
        if String::from_utf8_lossy(&out.stdout).contains("docker") {
            return vec![PrivEscVector {
                technique: "docker run -v /:/mnt --rm -it alpine chroot /mnt".into(),
                binary: "docker".into(),
                confidence: 1.0,
                description: "Docker group = full root via volume mount".into(),
                mitre_id: "T1548.001",
                risk: RiskLevel::High,
            }];
        }
    }
    Vec::new()
}

fn scan_nfs_shares() -> Vec<PrivEscVector> {
    if let Ok(content) = std::fs::read_to_string("/etc/exports") {
        if content.contains("no_root_squash") {
            return vec![PrivEscVector {
                technique: "NFS no_root_squash exploit".into(),
                binary: "/etc/exports".into(),
                confidence: 0.7,
                description: "NFS export with no_root_squash".into(),
                mitre_id: "T1548.001",
                risk: RiskLevel::High,
            }];
        }
    }
    Vec::new()
}

fn scan_kernel_exploits() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    if let Ok(out) = Command::new("uname").arg("-r").output() {
        let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
        // DirtyPipe: 5.8 <= version < 5.16.11, 5.15.25, 5.10.102
        let dirty_pipe_affected = version.starts_with("5.8")
            || version.starts_with("5.9")
            || version.starts_with("5.10")
            || version.starts_with("5.11")
            || version.starts_with("5.12")
            || version.starts_with("5.13")
            || version.starts_with("5.14")
            || version.starts_with("5.15")
            || version.starts_with("5.16");
        if dirty_pipe_affected {
            vectors.push(PrivEscVector {
                technique: "CVE-2022-0847 (DirtyPipe)".into(),
                binary: "kernel".into(),
                confidence: 0.85,
                description: format!("DirtyPipe — kernel {} affected", version),
                mitre_id: "T1068",
                risk: RiskLevel::Critical,
            });
        }
        // PwnKit: any version with pkexec
        if Command::new("which")
            .arg("pkexec")
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false)
        {
            vectors.push(PrivEscVector {
                technique: "CVE-2021-4034 (PwnKit)".into(),
                binary: "pkexec".into(),
                confidence: 0.95,
                description: "PwnKit — pkexec vulnerable before 0.105".into(),
                mitre_id: "T1068",
                risk: RiskLevel::Critical,
            });
        }
    }
    vectors
}

fn scan_container_escapes() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    if let Ok(content) = std::fs::read_to_string("/proc/1/cgroup") {
        if content
            .lines()
            .any(|l| l.contains("docker") || l.contains("kubepods") || l.contains("containerd"))
        {
            vectors.push(PrivEscVector {
                binary: String::new(),
                technique: "cgroup escape".into(),
                confidence: 0.65,
                description: "Cgroup release_agent escape from container".into(),
                mitre_id: "T1611",
                risk: RiskLevel::Critical,
            });
            if std::path::Path::new("/proc/1/root").exists() {
                vectors.push(PrivEscVector {
                    binary: String::new(),
                    technique: "proc1 root".into(),
                    confidence: 0.70,
                    description: "/proc/1/root container escape".into(),
                    mitre_id: "T1611",
                    risk: RiskLevel::Critical,
                });
            }
        }
    }
    vectors
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Critical);
        assert!(RiskLevel::Medium < RiskLevel::High);
    }






    #[test]
    fn test_scan_sorted_by_risk() {
        let mut vecs = [
            PrivEscVector {
                technique: "high".into(),
                binary: "a".into(),
                confidence: 1.0,
                description: "".into(),
                mitre_id: "T1068",
                risk: RiskLevel::High,
            },
            PrivEscVector {
                technique: "low".into(),
                binary: "b".into(),
                confidence: 1.0,
                description: "".into(),
                mitre_id: "T1068",
                risk: RiskLevel::Low,
            },
            PrivEscVector {
                technique: "critical".into(),
                binary: "c".into(),
                confidence: 1.0,
                description: "".into(),
                mitre_id: "T1068",
                risk: RiskLevel::Critical,
            },
        ];
        vecs.sort_by_key(|v| v.risk);
        assert_eq!(vecs[0].risk, RiskLevel::Low);
        assert_eq!(vecs[1].risk, RiskLevel::High);
        assert_eq!(vecs[2].risk, RiskLevel::Critical);
    }


    #[test]
    fn test_scan_suid_binaries_readonly() {
        // La enumeración sigue siendo real (solo lectura) y no debe panicar.
        let _ = scan_suid_binaries();
    }
}
