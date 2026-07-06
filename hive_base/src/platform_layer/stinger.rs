/// StingerTrait: bootstrap/loader for spawning agent processes
/// that inherit the shared memory arena.
///
/// On Linux: fork + execve with __HIVE_ARENA env var.
/// On Windows: CreateProcess with __HIVE_ARENA env var.
///
/// This is the "needle" that injects new drones into the hive.
use std::io;

pub trait StingerTrait {
    /// Spawn a child agent with the given arena name.
    fn spawn_agent(agent_path: &str, arena_name: &str, args: &[&str]) -> io::Result<u32>;
    /// Get the current executable path (for self-replication).
    fn current_exe() -> io::Result<String>;
}

// ── Linux implementation (execve with env var) ────────────────────────────────

#[cfg(target_os = "linux")]
pub struct LinuxStinger;

#[cfg(target_os = "linux")]
impl StingerTrait for LinuxStinger {
    fn spawn_agent(agent_path: &str, arena_name: &str, args: &[&str]) -> io::Result<u32> {
        use std::os::unix::process::CommandExt;
        use std::process::{Command, Stdio};

        let mut cmd = Command::new(agent_path);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("__HIVE_ARENA", arena_name);

        unsafe {
            cmd.pre_exec(|| {
                libc::setsid(); // detach from parent session
                Ok(())
            });
        }

        let child = cmd.spawn()?;
        Ok(child.id())
    }

    fn current_exe() -> io::Result<String> {
        let path = std::env::current_exe()?;
        Ok(path.to_string_lossy().to_string())
    }
}

// ── Windows implementation ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub struct WindowsStinger;

#[cfg(target_os = "windows")]
impl StingerTrait for WindowsStinger {
    fn spawn_agent(agent_path: &str, arena_name: &str, args: &[&str]) -> io::Result<u32> {
        // Build command line
        let mut cmdline = agent_path.to_string();
        for arg in args {
            cmdline.push(' ');
            cmdline.push_str(arg);
        }

        // Build environment block with __HIVE_ARENA
        let mut wide_cmd: Vec<u16> = cmdline.encode_utf16().chain(Some(0)).collect();
        let wide_dir: Vec<u16> = std::env::current_dir()
            .map(|p| p.to_string_lossy().encode_utf16().chain(Some(0)).collect())
            .unwrap_or_else(|_| vec![0u16]);

        // We set the env var in the current process before spawning;
        // child inherits it. In production, we'd build a custom env block.
        std::env::set_var("__HIVE_ARENA", arena_name);

        let mut si: winapi::um::processthreadsapi::STARTUPINFOW = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<winapi::um::processthreadsapi::STARTUPINFOW>() as u32;
        let mut pi: winapi::um::processthreadsapi::PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

        let rc = unsafe {
            winapi::um::processthreadsapi::CreateProcessW(
                std::ptr::null_mut(),       // lpApplicationName
                wide_cmd.as_mut_ptr(),      // lpCommandLine
                std::ptr::null_mut(),       // lpProcessAttributes
                std::ptr::null_mut(),       // lpThreadAttributes
                0,                          // bInheritHandles
                winapi::um::winbase::CREATE_NO_WINDOW | winapi::um::winbase::DETACHED_PROCESS,
                std::ptr::null_mut(),       // lpEnvironment
                wide_dir.as_ptr(),          // lpCurrentDirectory
                &mut si,
                &mut pi,
            )
        };

        if rc == 0 {
            return Err(io::Error::last_os_error());
        }

        let pid = pi.dwProcessId;
        unsafe {
            winapi::um::handleapi::CloseHandle(pi.hThread);
            winapi::um::handleapi::CloseHandle(pi.hProcess);
        }
        Ok(pid)
    }

    fn current_exe() -> io::Result<String> {
        let path = std::env::current_exe()?;
        Ok(path.to_string_lossy().to_string())
    }
}

// ── Fallback (std::process::Command) ──────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub struct GenericStinger;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl StingerTrait for GenericStinger {
    fn spawn_agent(agent_path: &str, arena_name: &str, args: &[&str]) -> io::Result<u32> {
        use std::process::{Command, Stdio};
        let child = Command::new(agent_path)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .env("__HIVE_ARENA", arena_name)
            .spawn()?;
        Ok(child.id())
    }

    fn current_exe() -> io::Result<String> {
        let path = std::env::current_exe()?;
        Ok(path.to_string_lossy().to_string())
    }
}
