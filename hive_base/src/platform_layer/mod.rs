pub mod filesystem;
pub mod ipc;
pub mod network;
pub mod os;
pub mod process;
pub mod runtime;
pub mod stinger;

pub use ipc::ArenaTrait;
pub use os::OsTrait;
pub use stinger::StingerTrait;

/// Platform detection helpers
pub fn is_linux() -> bool {
    cfg!(target_os = "linux")
}
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Get current platform name
pub fn platform_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unknown"
    }
}
