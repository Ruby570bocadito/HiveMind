use std::io;
use std::path::Path;

/// Cross-platform filesystem operations.
pub fn path_exists(path: &str) -> bool { Path::new(path).exists() }

pub fn is_file(path: &str) -> bool { Path::new(path).is_file() }

pub fn is_dir(path: &str) -> bool { Path::new(path).is_dir() }

pub fn read_file(path: &str) -> io::Result<Vec<u8>> { std::fs::read(path) }

pub fn write_file(path: &str, data: &[u8]) -> io::Result<()> { std::fs::write(path, data) }

pub fn create_dir_all(path: &str) -> io::Result<()> { std::fs::create_dir_all(path) }

pub fn remove_file(path: &str) -> io::Result<()> { std::fs::remove_file(path) }

pub fn remove_dir_all(path: &str) -> io::Result<()> { std::fs::remove_dir_all(path) }

pub fn copy(src: &str, dst: &str) -> io::Result<u64> { std::fs::copy(src, dst) }

pub fn rename(src: &str, dst: &str) -> io::Result<()> { std::fs::rename(src, dst) }

/// Make file executable (no-op on Windows, chmod +x on Unix)
#[cfg(unix)]
pub fn make_executable(path: &str) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path)?;
    let mut perms = metadata.permissions();
    let mode = perms.mode();
    perms.set_mode(mode | 0o111);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
pub fn make_executable(_path: &str) -> io::Result<()> {
    Ok(())
}

/// Get file size
pub fn file_size(path: &str) -> io::Result<u64> {
    Ok(std::fs::metadata(path)?.len())
}

/// Get file modified time as UNIX timestamp in ms
pub fn modified_ms(path: &str) -> io::Result<u64> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    Ok(duration.as_millis() as u64)
}
