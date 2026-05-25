pub mod ipc;
pub mod os;
pub mod stinger;
pub mod process;
pub mod filesystem;
pub mod network;
pub mod runtime;

pub use ipc::ArenaTrait;
pub use os::OsTrait;
pub use stinger::StingerTrait;

/// Platform detection helpers
pub fn is_linux() -> bool { cfg!(target_os = "linux") }
pub fn is_windows() -> bool { cfg!(target_os = "windows") }
pub fn is_macos() -> bool { cfg!(target_os = "macos") }

/// Get current platform name
pub fn platform_name() -> &'static str {
    if cfg!(target_os = "linux") { "linux" }
    else if cfg!(target_os = "windows") { "windows" }
    else if cfg!(target_os = "macos") { "macos" }
    else { "unknown" }
}
