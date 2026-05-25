use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

/// OsTrait: all platform-specific OS operations.
/// LinuxOs: uses /proc/self/... , syscalls, etc.
/// WindowsOs: uses Win32 API.
pub trait OsTrait {
    /// Get current process ID
    fn getpid() -> u32;
    /// Get current thread ID
    fn gettid() -> u64;
    /// Get monotonic timestamp in milliseconds
    fn monotonic_ms() -> u64;
    /// Get wall clock timestamp in milliseconds
    fn wallclock_ms() -> u64;
    /// Sleep for milliseconds
    fn sleep_ms(ms: u64);
    /// Get executable path
    fn exe_path() -> io::Result<String>;
    /// Check if a process with given PID is alive
    fn is_process_alive(pid: u32) -> bool;
    /// Get a temporary directory path
    fn temp_dir() -> String;
}

// ── Linux implementation ──────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub struct LinuxOs;

#[cfg(target_os = "linux")]
impl OsTrait for LinuxOs {
    fn getpid() -> u32 {
        unsafe { libc::getpid() as u32 }
    }

    fn gettid() -> u64 {
        unsafe { libc::gettid() as u64 }
    }

    fn monotonic_ms() -> u64 {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts); }
        (ts.tv_sec as u64 * 1000) + (ts.tv_nsec as u64 / 1_000_000)
    }

    fn wallclock_ms() -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        now.as_millis() as u64
    }

    fn sleep_ms(ms: u64) {
        if ms > 0 {
            unsafe { libc::usleep((ms * 1000) as libc::useconds_t); }
        }
    }

    fn exe_path() -> io::Result<String> {
        let path = std::env::current_exe()?;
        Ok(path.to_string_lossy().to_string())
    }

    fn is_process_alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    fn temp_dir() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }
}

// ── Windows implementation ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub struct WindowsOs;

#[cfg(target_os = "windows")]
impl OsTrait for WindowsOs {
    fn getpid() -> u32 {
        unsafe { winapi::um::processthreadsapi::GetCurrentProcessId() }
    }

    fn gettid() -> u64 {
        unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() as u64 }
    }

    fn monotonic_ms() -> u64 {
        use std::mem;
        let mut freq: winapi::um::winnt::LARGE_INTEGER = unsafe { mem::zeroed() };
        let mut count: winapi::um::winnt::LARGE_INTEGER = unsafe { mem::zeroed() };
        unsafe {
            winapi::um::profileapi::QueryPerformanceFrequency(&mut freq);
            winapi::um::profileapi::QueryPerformanceCounter(&mut count);
        }
        let freq_val = unsafe { *freq.QuadPart() };
        let count_val = unsafe { *count.QuadPart() };
        if freq_val > 0 { (count_val as u64 * 1000) / freq_val as u64 } else { 0 }
    }

    fn wallclock_ms() -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        now.as_millis() as u64
    }

    fn sleep_ms(ms: u64) {
        unsafe { winapi::um::synchapi::Sleep(ms as u32); }
    }

    fn exe_path() -> io::Result<String> {
        let path = std::env::current_exe()?;
        Ok(path.to_string_lossy().to_string())
    }

    fn is_process_alive(pid: u32) -> bool {
        const STILL_ACTIVE: u32 = 259;
        let handle = unsafe { winapi::um::processthreadsapi::OpenProcess(
            winapi::um::winnt::PROCESS_QUERY_INFORMATION,
            0,
            pid,
        )};
        if handle.is_null() { return false; }
        let mut exit_code = 0u32;
        let alive = unsafe {
            winapi::um::processthreadsapi::GetExitCodeProcess(handle, &mut exit_code as *mut _) != 0
            && exit_code == STILL_ACTIVE
        };
        unsafe { winapi::um::handleapi::CloseHandle(handle); }
        alive
    }

    fn temp_dir() -> String {
        std::env::temp_dir().to_string_lossy().to_string()
    }
}

// ── Fallback (macOS/other) ────────────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub struct GenericOs;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl OsTrait for GenericOs {
    fn getpid() -> u32 { std::process::id() }
    fn gettid() -> u64 { std::process::id() as u64 }
    fn monotonic_ms() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }
    fn wallclock_ms() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }
    fn sleep_ms(ms: u64) { std::thread::sleep(std::time::Duration::from_millis(ms)); }
    fn exe_path() -> io::Result<String> {
        Ok(std::env::current_exe()?.to_string_lossy().to_string())
    }
    fn is_process_alive(_pid: u32) -> bool { true }
    fn temp_dir() -> String { std::env::temp_dir().to_string_lossy().to_string() }
}
