// OS abstraction layer: system info, paths, and process enumeration.
// Provides a common trait with Linux and Windows implementations.

use std::path::PathBuf;

pub trait SystemInfo {
    /// List running process names
    fn running_processes() -> Vec<String>;
    /// Number of running processes
    fn process_count() -> usize;
    /// Number of network interfaces
    fn network_interface_count() -> usize;
    /// CPU usage percentage (0-100)
    fn cpu_usage_percent() -> f32;
    /// Memory usage percentage (0-100)
    fn memory_usage_percent() -> f32;
    /// Path to shared memory base directory
    fn shm_base_dir() -> PathBuf;
    /// Hostname
    fn hostname() -> String;
    /// Current username
    fn current_user() -> String;
    /// OS type string (e.g., "linux", "windows")
    fn os_type() -> &'static str;
    /// Architecture string (e.g., "x86_64")
    fn arch() -> &'static str;
}

// ── Linux implementation ──────────────────────────────────────────────

pub struct LinuxInfo;

impl SystemInfo for LinuxInfo {
    fn running_processes() -> Vec<String> {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().join("comm").exists())
                .filter_map(|e| std::fs::read_to_string(e.path().join("comm")).ok())
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            Vec::new()
        }
    }

    fn process_count() -> usize {
        if let Ok(entries) = std::fs::read_dir("/proc") {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().join("comm").exists())
                .count()
        } else {
            0
        }
    }

    fn network_interface_count() -> usize {
        std::fs::read_dir("/sys/class/net")
            .map(|e| e.count())
            .unwrap_or(0)
    }

    fn cpu_usage_percent() -> f32 {
        if let Ok(stat) = std::fs::read_to_string("/proc/stat") {
            let line = stat.lines().next().unwrap_or("");
            let parts: Vec<f32> = line.split_whitespace().skip(1)
                .filter_map(|v| v.parse().ok()).collect();
            if parts.len() >= 4 {
                let idle = parts[3];
                let total: f32 = parts.iter().sum();
                if total > 0.0 {
                    return 100.0 - (idle / total * 100.0);
                }
            }
        }
        25.0
    }

    fn memory_usage_percent() -> f32 {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total: u64 = 0;
            let mut avail: u64 = 0;
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    total = line.split_whitespace().nth(1)
                        .and_then(|v| v.parse().ok()).unwrap_or(0);
                }
                if line.starts_with("MemAvailable:") {
                    avail = line.split_whitespace().nth(1)
                        .and_then(|v| v.parse().ok()).unwrap_or(0);
                }
            }
            if total > 0 {
                return ((total - avail) as f32 / total as f32) * 100.0;
            }
        }
        50.0
    }

    fn shm_base_dir() -> PathBuf {
        PathBuf::from("/dev/shm")
    }

    fn hostname() -> String {
        std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".into())
    }

    fn current_user() -> String {
        std::env::var("USER").unwrap_or_else(|_| "unknown".into())
    }

    fn os_type() -> &'static str { "linux" }
    fn arch() -> &'static str { std::env::consts::ARCH }
}

// ── Windows implementation ────────────────────────────────────────────

pub struct WindowsInfo;

impl SystemInfo for WindowsInfo {
    fn running_processes() -> Vec<String> {
        // Windows: use WMI or toolhelp32 snapshot
        // Simplified: return empty for now (allows compilation)
        Vec::new()
    }

    fn process_count() -> usize { 0 }

    fn network_interface_count() -> usize {
        // Windows: use GetAdaptersInfo or iphlpapi
        0
    }

    fn cpu_usage_percent() -> f32 {
        // Windows: use GetSystemTimes
        25.0
    }

    fn memory_usage_percent() -> f32 {
        // Windows: use GlobalMemoryStatusEx
        50.0
    }

    fn shm_base_dir() -> PathBuf {
        // Windows: use %TEMP% or named pipe
        std::env::temp_dir()
    }

    fn hostname() -> String {
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
    }

    fn current_user() -> String {
        std::env::var("USERNAME").unwrap_or_else(|_| "unknown".into())
    }

    fn os_type() -> &'static str { "windows" }
    fn arch() -> &'static str { std::env::consts::ARCH }
}

// ── Auto-detect platform ──────────────────────────────────────────────

pub type PlatformInfo = LinuxInfo;

/// Check if a process name matches known EDR process signatures
pub fn is_edr_process(name: &str) -> bool {
    let edr: &[&str] = &["csfalcon", "csagent", "msmpeng", "sentinelone", "carbonblack", "cylancesvc", "symantec", "mcafee"];
    let lower = name.to_lowercase();
    edr.iter().any(|e| lower.contains(e))
}

/// Check if a process name matches known backup process signatures
pub fn is_backup_process(name: &str) -> bool {
    let backup: &[&str] = &["veeam", "backup_exec", "commvault", "netbackup", "backup_agent", "vss"];
    let lower = name.to_lowercase();
    backup.iter().any(|b| lower.contains(b))
}
