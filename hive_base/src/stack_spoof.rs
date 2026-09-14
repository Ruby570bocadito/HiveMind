//! Stack spoofing EMULATION — Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): la construcción de pilas sintéticas (SilentMoonwalk-
//! style, frames falsos que apuntan a módulos plausibles) fue ELIMINADA — es
//! maquinaria pura de evasión de EDR. Se conserva la superficie de tipos y:
//! - introspección de solo lectura del propio proceso (`get_rbp`,
//!   `walk_stack`, `find_module_base`) — técnicas de depuración benignas;
//! - `detect_stack_spoofing()` — detector DEFENSIVO: comprueba si los frames
//!   de la pila caen fuera de la región `[stack]` del proceso;
//! - `build_fake_stack` / `SyntheticFrame::for_module` → SIMULADOS (vacíos).
//!
//! Así, los tests de detección pueden ejercitar el detector sin que exista
//! código que fabrique pilas falsas.

// ── Linux ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub mod linux {
    use tracing::info;

    /// Lee el puntero de marco actual (RBP) del hilo que invoca. Solo lectura.
    #[inline(always)]
    pub fn get_rbp() -> usize {
        let rbp: usize;
        unsafe { std::arch::asm!("mov {}, rbp", out(reg) rbp, options(nomem, nostack)) };
        rbp
    }

    /// Recorre la cadena de RBP del propio hilo (máx. `max_frames`).
    /// Solo lectura; se detiene ante punteros no plausibles.
    pub fn walk_stack(max_frames: usize) -> Vec<usize> {
        let mut frames = Vec::new();
        let mut rbp = get_rbp();
        for _ in 0..max_frames {
            if rbp == 0 {
                break;
            }
            let saved_rbp = unsafe { *(rbp as *const usize) };
            let ret_addr = unsafe { *((rbp as *const usize).add(1)) };
            if ret_addr == 0 {
                break;
            }
            frames.push(ret_addr);
            if saved_rbp <= rbp {
                break; // cadena no creciente: fin o pila corrupta
            }
            rbp = saved_rbp;
        }
        frames
    }

    /// Frame sintético DESACTIVADO (ronda 4): `for_module` siempre `None`.
    #[derive(Debug, Clone, Copy)]
    pub struct SyntheticFrame {
        pub return_address: usize,
    }

    impl SyntheticFrame {
        /// SIMULADO (ronda 4): no fabrica frames que apunten a módulos reales.
        pub fn for_module(_module_name: &str) -> Option<Self> {
            info!("STACK_SPOOF (simulated): SyntheticFrame::for_module — no se fabrican frames (emulation mode)");
            None
        }

        pub fn as_ptr(&self) -> *const SyntheticFrame {
            self as *const _
        }
    }

    /// Localiza la dirección base de un módulo en el propio proceso vía
    /// `/proc/self/maps`. Solo lectura.
    pub fn find_module_base(name: &str) -> Option<usize> {
        let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
        for line in maps.lines() {
            if line.contains(name) {
                if let Some(first) = line.split('-').next() {
                    if let Ok(base) = usize::from_str_radix(first, 16) {
                        return Some(base);
                    }
                }
            }
        }
        None
    }

    /// SIMULADO (ronda 4): no fabrica pilas falsas. Devuelve un buffer vacío.
    pub fn build_fake_stack(num_frames: usize) -> (Vec<u8>, usize) {
        info!(
            "STACK_SPOOF (simulated): build_fake_stack({num_frames}) — no se construye pila sintética (emulation mode)"
        );
        (Vec::new(), 0)
    }

    /// Detector defensivo: ¿algún frame de la pila actual cae fuera de la
    /// región `[stack]` del proceso? Heurística de solo lectura.
    pub fn detect_stack_spoofing() -> bool {
        let frames = walk_stack(32);
        let maps = match std::fs::read_to_string("/proc/self/maps") {
            Ok(m) => m,
            Err(_) => return false,
        };
        let mut stack_range: Option<(usize, usize)> = None;
        for line in maps.lines() {
            if line.contains("[stack]") {
                let mut it = line.split_whitespace();
                if let Some(range) = it.next() {
                    let mut parts = range.split('-');
                    if let (Some(lo), Some(hi)) = (parts.next(), parts.next()) {
                        if let (Ok(lo), Ok(hi)) =
                            (usize::from_str_radix(lo, 16), usize::from_str_radix(hi, 16))
                        {
                            stack_range = Some((lo, hi));
                        }
                    }
                }
                break;
            }
        }
        match stack_range {
            Some((lo, hi)) => frames.iter().any(|f| *f < lo || *f > hi),
            None => false,
        }
    }
}

// ── Windows ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub mod windows {
    use tracing::info;

    /// SIMULADO (ronda 4): recorrido de pila deshabilitado.
    pub fn walk_stack(_max: usize) -> Vec<usize> {
        info!("STACK_SPOOF (simulated): walk_stack — deshabilitado (emulation mode)");
        Vec::new()
    }

    /// SIMULADO (ronda 4): enumeración de módulos deshabilitada.
    pub fn find_module_base(_name: &str) -> Option<usize> {
        info!("STACK_SPOOF (simulated): find_module_base — deshabilitado (emulation mode)");
        None
    }

    /// SIMULADO (ronda 4): no fabrica pilas sintéticas.
    pub fn build_fake_stack(num_frames: usize) -> (Vec<u8>, usize) {
        info!(
            "STACK_SPOOF (simulated): build_fake_stack({num_frames}) — deshabilitado (emulation mode)"
        );
        (Vec::new(), 0)
    }

    /// SIMULADO (ronda 4): detector informativo.
    pub fn detect_stack_spoofing() -> bool {
        info!("STACK_SPOOF (simulated): detect_stack_spoofing — heurística deshabilitada (emulation mode)");
        false
    }
}

// ── Re-exports ───────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
pub use windows::*;
