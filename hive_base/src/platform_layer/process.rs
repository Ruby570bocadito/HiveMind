use std::io;
use std::process::{Child, Command, Stdio};
use std::ffi::OsStr;

/// Cross-platform process spawning with stdin/stdout/stderr pipes.
///
/// Linux: standard fork+exec via std::process::Command.
/// Windows: same via std::process::Command (Win32 CreateProcess underneath).
pub struct ChildProcess {
    inner: Option<Child>,
}

impl ChildProcess {
    pub fn spawn<I, S>(program: &str, args: I) -> io::Result<Self>
    where I: IntoIterator<Item = S>, S: AsRef<OsStr>,
    {
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        Ok(Self { inner: Some(child) })
    }

    pub fn spawn_detached<I, S>(program: &str, args: I) -> io::Result<u32>
    where I: IntoIterator<Item = S>, S: AsRef<OsStr>,
    {
        let child = Self::spawn_detached_inner(program, args)?;
        Ok(child.id())
    }

    #[cfg(target_os = "linux")]
    fn spawn_detached_inner<I, S>(program: &str, args: I) -> io::Result<Child>
    where I: IntoIterator<Item = S>, S: AsRef<OsStr>,
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        unsafe {
            cmd.pre_exec(|| {
                // Create new session, detach from parent
                libc::setsid();
                Ok(())
            });
        }
        cmd.spawn()
    }

    #[cfg(not(target_os = "linux"))]
    fn spawn_detached_inner<I, S>(program: &str, args: I) -> io::Result<Child>
    where I: IntoIterator<Item = S>, S: AsRef<OsStr>,
    {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }

    pub fn id(&self) -> u32 {
        self.inner.as_ref().map(|c| c.id()).unwrap_or(0)
    }

    pub fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        if let Some(ref mut child) = self.inner {
            child.wait()
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "no child process"))
        }
    }

    pub fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        if let Some(ref mut child) = self.inner {
            child.try_wait()
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "no child process"))
        }
    }

    pub fn kill(&mut self) -> io::Result<()> {
        if let Some(ref mut child) = self.inner {
            child.kill()
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "no child process"))
        }
    }

    #[cfg(target_os = "linux")]
    pub fn kill_pid(pid: u32) -> io::Result<()> {
        let rc = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
    }

    #[cfg(target_os = "windows")]
    pub fn kill_pid(pid: u32) -> io::Result<()> {
        let handle = unsafe {
            winapi::um::processthreadsapi::OpenProcess(
                winapi::um::winnt::PROCESS_TERMINATE,
                0,
                pid,
            )
        };
        if handle.is_null() { return Err(io::Error::last_os_error()); }
        let rc = unsafe { winapi::um::processthreadsapi::TerminateProcess(handle, 1) };
        unsafe { winapi::um::handleapi::CloseHandle(handle); }
        if rc != 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    pub fn kill_pid(pid: u32) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "kill_pid not implemented"))
    }
}
