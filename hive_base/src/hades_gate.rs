//! Hades Gate EMULATION — Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): la resolución dinámica de SSNs desde la ntdll cargada
//! (Hell's/Halo's Gate) era maquinaria de evasión de EDR y fue ELIMINADA.
//! Las firmas se conservan porque `syscalls` y `stack_spoof` las consumen;
//! ahora devuelven `None` con un evento de telemetría, de modo que las rutas
//! que dependían de ellas degradan de forma controlada.
//!
//! Valor de entrenamiento: los tests de detección pueden verificar que el
//! proceso *intenta* resolver SSNs (evento visible) y que el resultado queda
//! instrumentalizado, sin que exista código que realice el Sondeo de memoria
//! de ntdll.
#[cfg(target_os = "windows")]
pub mod windows {
    use tracing::info;

    /// SIMULADO (ronda 4): no recorre la memoria de ntdll. Devuelve `None`.
    pub fn hades_resolve_ssn(function_name: &str) -> Option<u32> {
        info!(
            "HADES_GATE (simulated): hades_resolve_ssn('{function_name}') — resolución no realizada (emulation mode)"
        );
        None
    }

    /// SIMULADO (ronda 4): no localiza la base de ntdll en memoria. Devuelve `None`.
    pub fn get_loaded_ntdll_base() -> Option<usize> {
        info!("HADES_GATE (simulated): get_loaded_ntdll_base — búsqueda no realizada (emulation mode)");
        None
    }
}

#[cfg(target_os = "windows")]
pub use windows::*;
