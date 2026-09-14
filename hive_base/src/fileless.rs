// Fileless process execution via memfd_create (Linux) — Hive Colony
// (red-team lab edition).
//
// Ronda 4 (emulación): la ejecución fileless es una técnica dual-use de
// laboratorio (demos de detección de artefactos en memoria). `new()` sigue
// siendo un `memfd_create` benigno (lo usan tests y el TUI), pero `spawn()`
// requiere autorización explícita del laboratorio (`HIVE_LAB_AUTHORIZED=1`)
// y deja constancia en telemetría. La maquinaria de secciones de memoria de
// Windows (NtCreateSection) fue ELIMINADA.

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
        let fd = unsafe { libc::memfd_create(cname.as_ptr(), libc::MFD_CLOEXEC) };

        if fd == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        // The File owns the fd and closes it on drop; no manual close here
        // (a second close could release an fd reused by another thread).
        file.write_all(binary_data)?;

        let raw_fd = file.into_raw_fd();

        Ok(Self {
            fd: raw_fd,
            name: name.to_string(),
        })
    }

    pub fn spawn(&self, env_vars: &[(&str, &str)]) -> io::Result<std::process::Child> {
        // Gate de laboratorio (ronda 4): sin autorización explícita no se
        // ejecuta nada desde memoria.
        if std::env::var("HIVE_LAB_AUTHORIZED").as_deref() != Ok("1") {
            info!(
                "FILELESS: spawn '{}' bloqueado — requiere HIVE_LAB_AUTHORIZED=1 (lab authorization policy)",
                self.name
            );
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fileless spawn requires HIVE_LAB_AUTHORIZED=1 in an authorized lab",
            ));
        }
        let fd_path = format!("/proc/self/fd/{}", self.fd);

        let mut cmd = Command::new(&fd_path);
        for (key, val) in env_vars {
            cmd.env(key, val);
        }

        unsafe {
            cmd.pre_exec(|| Ok(()));
        }

        let child = cmd.spawn()?;
        info!(
            "Fileless spawn: {} (PID: {}, fd: {})",
            self.name,
            child.id(),
            self.fd
        );
        Ok(child)
    }

    pub fn raw_fd(&self) -> i32 {
        self.fd
    }

    pub fn seal(&self) -> io::Result<()> {
        let rc = unsafe {
            libc::fcntl(
                self.fd,
                libc::F_ADD_SEALS,
                libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE,
            )
        };
        if rc == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for MemfdBinary {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

// ── Non-Linux fallback (macOS, etc.) ──────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub struct MemfdBinary {
    data: Vec<u8>,
    name: String,
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl MemfdBinary {
    pub fn new(name: &str, binary_data: &[u8]) -> io::Result<Self> {
        Ok(Self {
            data: binary_data.to_vec(),
            name: name.to_string(),
        })
    }

    pub fn spawn(&self, env_vars: &[(&str, &str)]) -> io::Result<std::process::Child> {
        // Gate de laboratorio (ronda 4).
        if std::env::var("HIVE_LAB_AUTHORIZED").as_deref() != Ok("1") {
            info!(
                "FILELESS: spawn '{}' bloqueado — requiere HIVE_LAB_AUTHORIZED=1 (lab authorization policy)",
                self.name
            );
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fileless spawn requires HIVE_LAB_AUTHORIZED=1 in an authorized lab",
            ));
        }
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
    pub fn raw_fd(&self) -> i32 {
        -1
    }
    pub fn seal(&self) -> io::Result<()> {
        Ok(())
    }
}

// ── Windows: Memory-backed section execution ─────────────────────────────────

#[cfg(target_os = "windows")]
pub struct MemfdBinary {
    data: Vec<u8>,
    name: String,
}

#[cfg(target_os = "windows")]
impl MemfdBinary {
    pub fn new(name: &str, binary_data: &[u8]) -> io::Result<Self> {
        Ok(Self {
            data: binary_data.to_vec(),
            name: name.to_string(),
        })
    }

    /// SIMULADO (ronda 4): la creación de procesos desde secciones de memoria
    /// (NtCreateSection + mapeo ejecutable) era maquinaria de evasión y fue
    /// ELIMINADA. Devuelve siempre error de política.
    pub fn spawn(&self, _env_vars: &[(&str, &str)]) -> io::Result<std::process::Child> {
        info!(
            "FILELESS (simulated): spawn '{}' bloqueado — ejecución desde sección de memoria eliminada (emulation mode)",
            self.name
        );
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "fileless execution disabled (emulation mode, ronda 4)",
        ))
    }

    pub fn raw_fd(&self) -> i32 {
        -1
    }
    pub fn seal(&self) -> io::Result<()> {
        Ok(())
    }
}
