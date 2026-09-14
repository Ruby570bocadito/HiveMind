//! AntiForensics EMULATION — Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): toda la manipulación forense real (borrado de logs,
//! timestomping, limpieza de temporales) fue ELIMINADA. Los métodos se
//! conservan con las mismas firmas para no romper el flujo de tareas, pero
//! solo registran telemetría del "paso simulado". Valor de entrenamiento:
//! el defensor puede validar que su correlación alerta cuando una tarea de
//! este tipo atraviesa el enjambre, sin que exista código que altere logs.
use tracing::info;

pub struct AntiForensics;

impl AntiForensics {
    /// SIMULADO: no toca ningún log. Devuelve la lista de rutas que la
    /// técnica *habría* considerado (documentación de la técnica).
    pub fn wipe_logs() -> Vec<String> {
        let considered = vec![
            "/var/log/auth.log".into(),
            "/var/log/syslog".into(),
            "/var/log/wtmp".into(),
        ];
        info!(
            "ANTI_FORENSICS (simulated): wipe_logs sobre {} ruta(s) considerada(s) — ninguna modificada (emulation mode)",
            considered.len()
        );
        considered
    }

    /// SIMULADO: no modifica historiales de shell.
    pub fn wipe_history() -> Vec<String> {
        let considered = vec![
            "~/.bash_history".into(),
            "~/.zsh_history".into(),
        ];
        info!(
            "ANTI_FORENSICS (simulated): wipe_history sobre {} historial(es) considerado(s) — ninguno modificado (emulation mode)",
            considered.len()
        );
        considered
    }

    /// SIMULADO: no altera timestamps de ningún fichero.
    pub fn timestomp(_path: &str, _spoof_timestamp: Option<i64>) -> bool {
        info!(
            "ANTI_FORENSICS (simulated): timestomp sobre '{}' NO ejecutado (emulation mode)",
            _path
        );
        false
    }

    /// SIMULADO: no elimina ficheros temporales.
    pub fn clean_temp() -> Vec<String> {
        info!("ANTI_FORENSICS (simulated): clean_temp — sin acción (emulation mode)");
        Vec::new()
    }

    /// SIMULADO: secuencia completa documentada, sin efectos.
    pub fn full_sweep() {
        info!(
            "ANTI_FORENSICS (simulated): full_sweep = wipe_logs + wipe_history + timestomp + clean_temp — sin efectos (emulation mode)"
        );
        let _ = Self::wipe_logs();
        let _ = Self::wipe_history();
        let _ = Self::clean_temp();
    }
}
