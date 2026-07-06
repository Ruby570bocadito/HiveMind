use std::time::{Duration, Instant};
use tracing::warn;

pub struct AntiAnalysis {
    pub is_debugged: bool,
    pub is_sandbox: bool,
    pub is_vm: bool,
    pub suspicious_timing: bool,
}

impl AntiAnalysis {
    pub fn is_safe() -> bool {
        let checks = Self::run_checks();
        !checks.is_debugged && !checks.is_sandbox && !checks.is_vm && !checks.suspicious_timing
    }

    pub fn run_checks() -> Self {
        let result = Self {
            is_debugged: check_debugger(),
            is_sandbox: check_sandbox(),
            is_vm: check_vm(),
            suspicious_timing: check_timing(),
        };
        if result.is_debugged { warn!("Anti-analysis: debugger detected"); }
        if result.is_sandbox { warn!("Anti-analysis: sandbox detected"); }
        if result.is_vm { warn!("Anti-analysis: VM detected"); }
        if result.suspicious_timing { warn!("Anti-analysis: suspicious timing"); }
        result
    }
}

#[cfg(target_os = "linux")]
fn check_debugger() -> bool {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("TracerPid:") {
                let pid: i32 = line.split_whitespace()
                    .nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
                if pid != 0 { return true; }
            }
        }
    }
    #[cfg(target_arch = "x86_64")]
    {
        let ret = unsafe {
            let mut ret: i64;
            std::arch::asm!(
                "mov rax, 101", "mov rdi, 0",
                "xor rsi, rsi", "xor rdx, rdx", "xor r10, r10",
                "syscall", lateout("rax") ret,
                out("rdi") _, out("rsi") _, out("rdx") _, out("r10") _,
            );
            ret
        };
        if ret != 0 { return true; }
    }
    false
}

#[cfg(target_os = "windows")]
fn check_debugger() -> bool {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let peb: *const u8;
        std::arch::asm!("mov {0}, gs:[0x60]", out(reg) peb);
        let being_debugged = *((peb as usize + 2) as *const u8);
        if being_debugged != 0 { return true; }
    }
    false
}

#[cfg(target_os = "linux")]
fn check_sandbox() -> bool {
    let mut indicators = 0u8;
    if let Ok(uptime) = std::fs::read_to_string("/proc/uptime") {
        let seconds: f64 = uptime.split_whitespace()
            .next().and_then(|s| s.parse().ok()).unwrap_or(99999.0);
        if seconds < 600.0 { indicators += 1; }
    }
    if let Ok(user) = std::env::var("USER") {
        let lowers = user.to_lowercase();
        if lowers.contains("sandbox") || lowers.contains("malware")
            || lowers.contains("virus") || lowers.contains("test") || lowers == "user" {
            indicators += 1;
        }
    }
    let cores = num_cpus();
    if cores < 2 { indicators += 1; }
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let kb: u64 = line.split_whitespace()
                    .nth(1).and_then(|s| s.parse().ok()).unwrap_or(99999999);
                if kb < 2048000 { indicators += 1; }
                break;
            }
        }
    }
    indicators >= 3
}

#[cfg(target_os = "windows")]
fn check_sandbox() -> bool {
    let mut indicators = 0u8;
    if let Ok(user) = std::env::var("USERNAME") {
        let lowers = user.to_lowercase();
        if lowers.contains("sandbox") || lowers.contains("malware")
            || lowers.contains("virus") || lowers.contains("test")
            || lowers == "user" || lowers == "admin" || lowers == "wdagutilityaccount" {
            indicators += 1;
        }
    }
    let cores = num_cpus();
    if cores < 2 { indicators += 1; }
    if let Ok(comp) = std::env::var("COMPUTERNAME") {
        let lowers = comp.to_lowercase();
        if lowers.contains("sandbox") || lowers.contains(" malware")
            || lowers.contains("virus") || lowers.contains("test") {
            indicators += 1;
        }
    }
    indicators >= 2
}

#[cfg(target_os = "linux")]
fn check_vm() -> bool {
    if let Ok(product) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
        let lowers = product.to_lowercase();
        for vm_marker in &["virtualbox", "vmware", "qemu", "kvm", "xen", "hyper-v", "parallels"] {
            if lowers.contains(vm_marker) { return true; }
        }
    }
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        if cpuinfo.contains("hypervisor") { return true; }
    }
    if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
        for vm_mod in &["vboxguest", "vboxsf", "vmw_balloon", "vmwgfx",
                         "virtio", "xen_blkfront", "hv_vmbus"] {
            if modules.contains(vm_mod) { return true; }
        }
    }
    false
}

#[cfg(target_os = "windows")]
fn check_vm() -> bool {
    if let Ok(comp) = std::env::var("COMPUTERNAME") {
        let lowers = comp.to_lowercase();
        if lowers.contains("vbox") || lowers.contains("vmware") {
            return true;
        }
    }
    if let Ok(user) = std::env::var("USERNAME") {
        let lowers = user.to_lowercase();
        if lowers.contains("vbox") || lowers.contains("vmware") || lowers == "john" {
            return true;
        }
    }
    false
}

fn check_timing() -> bool {
    let start = Instant::now();
    let _ = (0..1000).fold(0u64, |acc, x| acc.wrapping_mul(x).wrapping_add(1));
    let elapsed = start.elapsed();
    elapsed > Duration::from_millis(100) || elapsed < Duration::from_nanos(1000)
}

fn num_cpus() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_checks_no_panic() {
        let result = AntiAnalysis::run_checks();
        // Should never panic regardless of platform
        let _ = result;
    }

    #[test]
    fn test_timing_check() {
        let suspicious = check_timing();
        // On normal hardware, should not be suspicious
        assert!(!suspicious);
    }

    #[test]
    fn test_num_cpus_positive() {
        let cores = num_cpus();
        assert!(cores >= 1);
    }

    #[test]
    fn test_is_safe_runs() {
        let safe = AntiAnalysis::is_safe();
        // Should not panic
        let _ = safe;
    }
}
