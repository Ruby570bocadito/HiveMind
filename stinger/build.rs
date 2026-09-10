//! Build script for `stinger` (the dropper).
//!
//! Stinger embeds the agent binaries via `include_bytes!`. Those binaries are
//! produced by the workspace itself (`cargo build --release`), which means a
//! clean clone does not have them yet — and without this detection script the
//! whole workspace fails to compile out of the box.
//!
//! If the agent binaries exist we emit `cargo:rustc-cfg=have_bins` (Linux
//! targets) / `cargo:rustc-cfg=have_bins_win` (Windows targets) so that
//! `main.rs` can embed the real payloads. Otherwise stinger still compiles,
//! but logs a clear warning and skips the spawn step at runtime.

use std::path::{Path, PathBuf};

fn all_exist(manifest_dir: &Path, bins: &[&str]) -> bool {
    bins.iter().all(|rel| manifest_dir.join(rel).exists())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());

    let linux_bins = [
        "../../target/release/worker",
        "../../target/release/drone",
        "../../target/release/honeybee",
        "../../target/release/weaver",
        "../../target/release/queen",
    ];
    let windows_bins = [
        "../../target/x86_64-pc-windows-gnu/release/worker.exe",
        "../../target/x86_64-pc-windows-gnu/release/drone.exe",
        "../../target/x86_64-pc-windows-gnu/release/honeybee.exe",
        "../../target/x86_64-pc-windows-gnu/release/weaver.exe",
        "../../target/x86_64-pc-windows-gnu/release/queen.exe",
    ];

    if all_exist(&manifest_dir, &linux_bins) {
        println!("cargo:rustc-cfg=have_bins");
    } else {
        println!(
            "cargo:warning=stinger: agent binaries not found in target/release/. \
             Building with EMPTY embeds (spawn steps will be skipped). \
             Build the agents first: cargo build --release -p worker -p drone -p honeybee -p weaver -p queen"
        );
    }

    if all_exist(&manifest_dir, &windows_bins) {
        println!("cargo:rustc-cfg=have_bins_win");
    }
}
