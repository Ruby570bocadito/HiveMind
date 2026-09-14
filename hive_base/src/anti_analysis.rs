//! AntiAnalysis EMULATION — Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): las comprobaciones reales anti-debug/anti-VM/anti-
//! sandbox (ptrace sobre sí mismo, artefactos de VMware/VirtualBox, timing
//! attacks) fueron ELIMINADAS — son técnicas de evasión cuyo único propósito
//! es frustrar el análisis. Los métodos conservan las firmas y devuelven un
//! estado "entorno limpio" etiquetado como simulado, suficiente para que el
//! TUI de beekeeper y las rutas de `utils` sigan funcionando.
use tracing::info;

#[derive(Debug, Clone)]
pub struct AntiAnalysis {
    pub is_debugged: bool,
    pub is_sandbox: bool,
    pub is_vm: bool,
    pub suspicious_timing: bool,
}

impl AntiAnalysis {
    /// Comodín de seguridad usado por `utils::preflight`.
    /// En emulación devuelve `true` (entorno tratado como seguro) y deja
    /// constancia en telemetría.
    pub fn is_safe() -> bool {
        info!("ANTI_ANALYSIS (simulated): is_safe=true por defecto (emulation mode)");
        true
    }

    /// SIMULADO (ronda 4): sin comprobaciones reales. Todos los flags a
    /// `false` representan "nada detectado" bajo política de emulación.
    pub fn run_checks() -> Self {
        info!(
            "ANTI_ANALYSIS (simulated): run_checks — comprobaciones reales eliminadas; resultado neutro (emulation mode)"
        );
        Self {
            is_debugged: false,
            is_sandbox: false,
            is_vm: false,
            suspicious_timing: false,
        }
    }
}
