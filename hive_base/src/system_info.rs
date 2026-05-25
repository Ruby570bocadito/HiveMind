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
        #[cfg(target_os = "windows")]
        {
            use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS, PROCESSENTRY32W};
            use winapi::um::handleapi::CloseHandle;
            use winapi::um::winnt::WCHAR;
            use std::mem;
            let mut processes = Vec::new();
            unsafe {
                let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
                if snapshot as isize != -1 {
                    let mut entry: PROCESSENTRY32W = mem::zeroed();
                    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
                    if Process32FirstW(snapshot, &mut entry) != 0 {
                        loop {
                            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(260);
                            let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                            processes.push(name);
                            if Process32NextW(snapshot, &mut entry) == 0 {
                                break;
                            }
                        }
                    }
                    CloseHandle(snapshot);
                }
            }
            return processes;
        }
        #[cfg(not(target_os = "windows"))]
        Vec::new()
    }

    fn process_count() -> usize {
        Self::running_processes().len()
    }

    fn network_interface_count() -> usize {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::iphlpapi::GetAdaptersAddresses;
            use winapi::um::iptypes::{GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GAA_FLAG_SKIP_DNS_SERVER, IP_ADAPTER_ADDRESSES_LH};
            use winapi::shared::ws2def::AF_UNSPEC;
            use winapi::shared::winerror::ERROR_BUFFER_OVERFLOW;
            use std::mem;
            unsafe {
                let mut size: u32 = 0;
                let ret = GetAdaptersAddresses(AF_UNSPEC as u32, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER, std::ptr::null_mut(), std::ptr::null_mut(), &mut size);
                if ret == winapi::shared::winerror::ERROR_BUFFER_OVERFLOW || ret == winapi::shared::winerror::NO_ERROR {
                    let buf = std::alloc::alloc(std::alloc::Layout::from_size_align(size as usize, 1).unwrap());
                    let ptr = buf as *mut IP_ADAPTER_ADDRESSES_LH;
                    let ret2 = GetAdaptersAddresses(AF_UNSPEC as u32, GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER, std::ptr::null_mut(), ptr, &mut size);
                    if ret2 == 0 {
                        let mut count = 0;
                        let mut current = ptr;
                        while !current.is_null() {
                            count += 1;
                            current = (*current).Next as *mut IP_ADAPTER_ADDRESSES_LH;
                        }
                        std::alloc::dealloc(buf, std::alloc::Layout::from_size_align(size as usize, 1).unwrap());
                        return count;
                    }
                    std::alloc::dealloc(buf, std::alloc::Layout::from_size_align(size as usize, 1).unwrap());
                }
            }
            0
        }
        #[cfg(not(target_os = "windows"))]
        0
    }

    fn cpu_usage_percent() -> f32 {
        #[cfg(target_os = "windows")]
        {
            use winapi::shared::minwindef::{BOOL, FILETIME};
            extern "system" {
                fn GetSystemTimes(
                    lpIdleTime: *mut FILETIME,
                    lpKernelTime: *mut FILETIME,
                    lpUserTime: *mut FILETIME,
                ) -> BOOL;
            }
            use std::mem;
            unsafe {
                let mut idle: FILETIME = mem::zeroed();
                let mut kernel: FILETIME = mem::zeroed();
                let mut user: FILETIME = mem::zeroed();
                if GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0 {
                    let idle_val = (idle.dwHighDateTime as u64) << 32 | idle.dwLowDateTime as u64;
                    let kernel_val = (kernel.dwHighDateTime as u64) << 32 | kernel.dwLowDateTime as u64;
                    let user_val = (user.dwHighDateTime as u64) << 32 | user.dwLowDateTime as u64;
                    let total = kernel_val + user_val;
                    if total > 0 {
                        return 100.0 - (idle_val as f32 / total as f32) * 100.0;
                    }
                }
            }
        }
        25.0
    }

    fn memory_usage_percent() -> f32 {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::sysinfoapi::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
            use std::mem;
            unsafe {
                let mut mem: MEMORYSTATUSEX = mem::zeroed();
                mem.dwLength = mem::size_of::<MEMORYSTATUSEX>() as u32;
                if GlobalMemoryStatusEx(&mut mem) != 0 {
                    return mem.dwMemoryLoad as f32;
                }
            }
        }
        50.0
    }

    fn shm_base_dir() -> PathBuf {
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

#[cfg(target_os = "linux")]
pub type PlatformInfo = LinuxInfo;
#[cfg(target_os = "windows")]
pub type PlatformInfo = WindowsInfo;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
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
