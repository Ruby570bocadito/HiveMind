// Honeycomb: persistence module — EMULATION MODE (ronda 5).
//
// Ronda 5 policy (same as phoenix, ronda 4): persistence mechanisms are
// DESCRIBED, never installed. This module keeps its public API so existing
// callers and lab scenarios keep compiling, but every write-path is a
// documented simulation that emits tagged telemetry instead of touching the
// host. Read-only feasibility/inspection helpers remain real (enumeration is
// allowed); uninstall/removal helpers remain real but strictly REMEDIAL: they
// can only delete artifacts previously created by this crate (identified by
// the HIVE_PERSISTENCE_MARKER / .hive_bak markers) and honor
// HIVE_PERSISTENCE_DRY_RUN=1 so tests and previews never touch the host.
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// SIMULADO (ronda 5): no instala ningún mecanismo de persistencia.
/// Devuelve siempre `false` (nada instalado) y emite telemetría etiquetada
/// describiendo los mecanismos *que se habrían instalado*.
pub fn install_persistence() -> bool {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".into());
    info!(
        "HONEYCOMB (simulated): install_persistence — mecanismos que se habrían instalado: \
         crontab @reboot (stinger en {}), systemd --user hive.service, .bashrc marker \
         (emulation mode, ronda 5; nada escrito)",
        exe
    );
    false
}

/// Remediación (real, quirúrgica): elimina SOLO artefactos creados
/// históricamente por este crate (líneas con HIVE_PERSISTENCE_MARKER en el
/// crontab del usuario, unidad systemd hive.service, líneas con el marker en
/// .bashrc). Nunca usa `crontab -r` (borraría el crontab completo del
/// usuario). Con HIVE_PERSISTENCE_DRY_RUN=1 solo registra lo que haría.
pub fn uninstall_persistence() {
    if std::env::var("HIVE_PERSISTENCE_DRY_RUN").is_ok() {
        info!(
            "HONEYCOMB (dry-run): uninstall_persistence — no se toca el host \
             (HIVE_PERSISTENCE_DRY_RUN activo)"
        );
        return;
    }

    // Remove ONLY the hive crontab entry (never `crontab -r`: that wipes the
    // user's whole crontab, including unrelated jobs).
    let removed = remove_hive_crontab_entries();
    if removed > 0 {
        info!("HONEYCOMB: removed {} hive crontab entries (remediation)", removed);
    }

    // Remove systemd service
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "disable", "hive.service"])
        .output();
    let _ = std::fs::remove_file(PathBuf::from(&home).join(".config/systemd/user/hive.service"));

    // Remove bashrc marker lines (only lines written by this crate)
    let bashrc = PathBuf::from(&home).join(".bashrc");
    if let Ok(content) = std::fs::read_to_string(&bashrc) {
        if content.contains("HIVE_PERSISTENCE_MARKER") {
            let cleaned: String = content
                .lines()
                .filter(|l| !l.contains("HIVE_PERSISTENCE_MARKER"))
                .collect::<Vec<_>>()
                .join("\n");
            let _ = std::fs::write(&bashrc, cleaned);
        }
    }
    info!("HONEYCOMB: persistence artifacts removed (remediation)");
}

/// Filtra del crontab del usuario únicamente las líneas con el marker de la
/// colmena y reescribe el crontab por stdin. Devuelve el número de líneas
/// eliminadas.
fn remove_hive_crontab_entries() -> usize {
    let Ok(out) = std::process::Command::new("crontab").arg("-l").output() else {
        return 0;
    };
    if !out.status.success() {
        return 0;
    }
    let current = String::from_utf8_lossy(&out.stdout);
    let kept: Vec<&str> = current
        .lines()
        .filter(|l| !l.contains("HIVE_PERSISTENCE_MARKER"))
        .collect();
    let removed = current.lines().count().saturating_sub(kept.len());
    if removed == 0 {
        return 0;
    }
    if let Ok(mut child) = std::process::Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(kept.join("\n").as_bytes());
        }
        let _ = child.wait_with_output();
    }
    removed
}

// ── UEFI bootkit (emulation) ──────────────────────────────────────────

/// Check if UEFI bootkit persistence is *possible* on this system.
/// Read-only enumeration (allowed): touches nothing.
pub fn uefi_bootkit_feasible() -> bool {
    // Check for EFI variables (Linux)
    Path::new("/sys/firmware/efi").exists()
}

/// SIMULADO (ronda 5): NO escribe nada en la partición EFI. Devuelve una
/// descripción documental de la ubicación *que habría sido sobrescrita*
/// (bootx64.efi del entry de boot elegido) y emite telemetría etiquetada.
pub fn install_uefi_bootkit(payload_binary: &[u8]) -> Result<String, String> {
    if !uefi_bootkit_feasible() {
        return Err("UEFI not available on this system".into());
    }

    // Find the EFI partition (read-only)
    let efi_dirs = ["/boot/efi/EFI", "/boot/EFI", "/efi/EFI"];

    let efi_path = efi_dirs
        .iter()
        .find(|d| Path::new(d).exists())
        .ok_or_else(|| "EFI partition not found".to_string())?;

    let boot_entries = ["Boot", "boot", "BOOT", "Microsoft", "ubuntu", "debian", "fedora"];
    let mut target_dir = None;

    for entry in &boot_entries {
        let path = Path::new(efi_path).join(entry);
        if path.exists() {
            target_dir = Some(path);
            break;
        }
    }

    let target = target_dir.ok_or_else(|| "No boot entry found in EFI partition".to_string())?;
    let would_overwrite = target.join("bootx64.efi");

    info!(
        "HONEYCOMB (simulated): UEFI bootkit {}B would overwrite {} — no data written \
         (emulation mode, ronda 5)",
        payload_binary.len(),
        would_overwrite.display()
    );
    Ok(format!(
        "simulated: bootkit would overwrite {} (no data written; emulation mode)",
        would_overwrite.display()
    ))
}

/// Remediación (real): restaura el bootloader original desde el backup
/// `bootx64.efi.hive_bak` creado por instalaciones históricas de este crate.
/// Solo actúa si existe nuestro backup; nunca toca nada más.
pub fn remove_uefi_bootkit() -> bool {
    let efi_dirs = ["/boot/efi/EFI", "/boot/EFI", "/efi/EFI"];

    for efi_path in &efi_dirs {
        if !Path::new(efi_path).exists() {
            continue;
        }

        let boot_entries = ["Boot", "boot", "BOOT", "Microsoft", "ubuntu"];
        for entry in &boot_entries {
            let target = Path::new(efi_path).join(entry);
            if !target.exists() {
                continue;
            }

            let backup = target.join("bootx64.efi.hive_bak");
            let original = target.join("bootx64.efi");

            if backup.exists() && std::fs::copy(&backup, &original).is_ok() {
                let _ = std::fs::remove_file(&backup);
                info!("HONEYCOMB: UEFI bootkit removed, original restored (remediation)");
                return true;
            }
        }
    }
    warn!("HONEYCOMB: no bootkit found to remove");
    false
}

/// SIMULADO (ronda 5): en modo emulación no se genera ningún binario de
/// bootkit. Devuelve un `Vec` vacío (la API se conserva por compatibilidad).
pub fn generate_bootkit_stub() -> Vec<u8> {
    info!(
        "HONEYCOMB (simulated): generate_bootkit_stub — no payload generated (emulation mode)"
    );
    Vec::new()
}

/// Check if a bootkit backup marker is currently present.
/// Read-only enumeration (allowed).
pub fn bootkit_installed() -> bool {
    let efi_dirs = ["/boot/efi/EFI", "/boot/EFI"];
    for efi_path in &efi_dirs {
        if !Path::new(efi_path).exists() {
            continue;
        }
        for entry in &["Boot", "boot", "BOOT", "Microsoft", "ubuntu"] {
            let target = Path::new(efi_path).join(entry);
            let backup = target.join("bootx64.efi.hive_bak");
            if backup.exists() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootkit_stub_emulation() {
        // EMULATION MODE (ronda 5): no bootkit payload is generated.
        let stub = generate_bootkit_stub();
        assert!(stub.is_empty(), "emulation mode must not generate payloads");
    }

    #[test]
    fn test_feasibility_check() {
        // On non-UEFI systems this should return false
        let feasible = uefi_bootkit_feasible();
        // Don't assert false — CI might run on UEFI
        info!("UEFI bootkit feasible: {}", feasible);
    }

    #[test]
    fn test_install_persistence_simulated() {
        // EMULATION MODE: nothing is installed, always returns false, and the
        // systemd unit for the hive must NOT exist afterwards on a clean host.
        let installed = install_persistence();
        assert!(!installed, "emulation mode must not install persistence");
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let unit = PathBuf::from(&home).join(".config/systemd/user/hive.service");
        assert!(
            !unit.exists(),
            "simulated install must not create the real systemd unit at {:?}",
            unit
        );
    }

    #[test]
    fn test_uninstall_dry_run_never_touches_host() {
        // DRY-RUN: uninstall must be a no-op against the real crontab/bashrc.
        std::env::set_var("HIVE_PERSISTENCE_DRY_RUN", "1");
        uninstall_persistence(); // must not panic and must not modify anything
    }
}
