/// Runtime environment detection: sandbox, debugger, EDR presence.

/// Returns true if running in a known sandbox or VM environment.
#[cfg(target_os = "linux")]
pub fn detect_sandbox() -> bool {
    // Check common VM/sandbox indicators
    let indicators = [
        "/proc/self/status",          // always present
        "/sys/class/dmi/id/product_name",
        "/sys/class/dmi/id/sys_vendor",
    ];
    for path in &indicators {
        if let Ok(content) = std::fs::read_to_string(path) {
            let lower = content.to_lowercase();
            if lower.contains("virtualbox")
                || lower.contains("vmware")
                || lower.contains("qemu")
                || lower.contains("cuckoo")
                || lower.contains("sandbox")
            {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
pub fn detect_sandbox() -> bool {
    if let Ok(user) = std::env::var("USERNAME") {
        let lowers = user.to_lowercase();
        if lowers.contains("sandbox") || lowers.contains("wdagutilityaccount") || lowers == "john" {
            return true;
        }
    }
    if let Ok(comp) = std::env::var("COMPUTERNAME") {
        let lowers = comp.to_lowercase();
        if lowers.contains("sandbox") || lowers.contains("vbox") || lowers.contains("vmware") {
            return true;
        }
    }
    false
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn detect_sandbox() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub fn detect_debugger() -> bool {
    if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
            if line.starts_with("TracerPid:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    return val != "0";
                }
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
pub fn detect_debugger() -> bool {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let peb: *const u8;
        std::arch::asm!("mov {0}, gs:[0x60]", out(reg) peb);
        let being_debugged = *((peb as usize + 2) as *const u8);
        if being_debugged != 0 { return true; }
    }
    false
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn detect_debugger() -> bool {
    false
}

#[cfg(target_os = "linux")]
pub fn detect_edr() -> bool {
    let edr_procs = ["falcon_sensor", "osqueryd", "auditd", "sophos", "sav", "avast", "clamd"];
    if let Ok(dir) = std::fs::read_dir("/proc") {
        for entry in dir.flatten() {
            let pid = entry.file_name();
            if let Ok(pid) = pid.to_string_lossy().parse::<u32>() {
                let cmdline_path = format!("/proc/{}/cmdline", pid);
                if let Ok(cmdline) = std::fs::read_to_string(&cmdline_path) {
                    let lower = cmdline.to_lowercase();
                    for proc in &edr_procs {
                        if lower.contains(proc) { return true; }
                    }
                }
            }
        }
    }
    false
}

#[cfg(target_os = "windows")]
pub fn detect_edr() -> bool {
    let edr_procs = [
        "msmpeng", "sentinelhelper", "sentinelstaticengine", "csfalcon", "csagent",
        "carbonblack", "cb.exe", "sep", "symantec", "norton",
        "mcshield", "mfehav", "mcafee", "sophos", "savservice",
        "cylance", "cyservice", "tmccsf", "tmbmsrv", "pccntmon",
        "avast", "avg", "kaspersky", "kavfs", "ekrn",
        "bdagent", "bdredline", "bitdefender", "f-secure", "fsma",
        "trendmicro", "amsp", "coreserviceshell", "elastic-endpoint",
    ];
    unsafe {
        use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS, PROCESSENTRY32W};
        use winapi::um::handleapi::CloseHandle;
        use std::mem;
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot as isize == -1 { return false; }
        let mut entry: PROCESSENTRY32W = mem::zeroed();
        entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) == 0 { CloseHandle(snapshot); return false; }
        loop {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(260);
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
            for proc in &edr_procs {
                if name.contains(proc) { CloseHandle(snapshot); return true; }
            }
            if Process32NextW(snapshot, &mut entry) == 0 { break; }
        }
        CloseHandle(snapshot);
    }
    false
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn detect_edr() -> bool {
    false
}

/// Overall evasion check: returns Vec of strings describing risks.
pub fn evasion_check() -> Vec<String> {
    let mut risks = Vec::new();
    if detect_sandbox() { risks.push("sandbox_detected".into()); }
    if detect_debugger() { risks.push("debugger_detected".into()); }
    if detect_edr() { risks.push("edr_detected".into()); }
    risks
}
