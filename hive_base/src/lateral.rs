//! Lateral movement EMULATION — Hive Colony (red-team lab edition).
//!
//! Ronda 4 (emulación): la recolección real de credenciales, la ejecución
//! remota por SSH y el despliegue de agentes en otros hosts fueron
//! DESACTIVADOS (sin `ssh` real, sin propagación). Se conserva:
//!
//! - `discover_hosts()`: barrido de descubrimiento (ping sweep) de solo
//!   lectura, útil en laboratorio para mapear el segmento de práctica.
//! - Tipos (`LateralResult`) y firmas, para que el flujo de tareas y la
//!   telemetría del enjambre no cambien.
use std::process::Command;
use tracing::info;

#[derive(Debug, Clone)]
pub struct LateralResult {
    pub success: bool,
    pub technique: String,
    pub target: String,
    pub output: String,
}

fn simulated(technique: &str, target: &str, detail: &str) -> LateralResult {
    info!(
        "LATERAL (simulated): {} contra {} — {} (emulation mode)",
        technique, target, detail
    );
    LateralResult {
        success: false,
        technique: technique.into(),
        target: target.into(),
        output: format!("simulated: {detail} (emulation mode)"),
    }
}

/// SIMULADO (ronda 4): no lee ficheros de credenciales. Devuelve una lista
/// vacía y registra el evento; el hallazgo real de fuentes de credenciales
/// vive en `leech::discover_credential_sources` (solo existencia de rutas).
pub fn harvest_credentials() -> Vec<(String, String, String)> {
    info!(
        "LATERAL (simulated): harvest_credentials invoked — recolección real deshabilitada (emulation mode)"
    );
    Vec::new()
}

/// SIMULADO (ronda 4): no ejecuta SSH remoto. Registra el intento y devuelve
/// un resultado simulado con el plan que se habría seguido.
pub fn exec_ssh(
    host: &str,
    username: &str,
    command: &str,
    key_path: Option<&str>,
    password: Option<&str>,
) -> LateralResult {
    // Gate BRAIN (se conserva): nunca contra objetivos marcados como seguros.
    let cfg = crate::config::HiveConfig::load();
    if crate::panal::is_safe_target(host, &cfg.brain) {
        return LateralResult {
            success: false,
            technique: "ssh_exec".into(),
            target: format!("{username}@{host}"),
            output: "BLOCKED by BRAIN: safe target".into(),
        };
    }
    let _ = password; // ignorada en emulación
    let plan = format!(
        "plan: ssh {}@{} (key={}) ejecutando '{}' — ejecución no realizada",
        username,
        host,
        key_path.unwrap_or("<agente>"),
        command
    );
    simulated("exec_ssh", host, &plan)
}

/// SIMULADO (ronda 4): no despliega ni propaga agentes. En el marco red-team
/// de laboratorio la propagación real fue eliminada (ronda 2: weaver; ronda 4:
/// deploy SSH). Devuelve el plan simulado.
pub fn deploy_agent_ssh(target: &str, agent_path: &str, key_path: Option<&str>) -> LateralResult {
    let plan = format!(
        "plan: copiar '{}' a {} y registrarlo como servicio — despliegue no realizado (key={})",
        agent_path,
        target,
        key_path.unwrap_or("<agente>")
    );
    simulated("deploy_agent_ssh", target, &plan)
}

/// Barrido de descubrimiento REAL (solo lectura): ping sweep del segmento.
/// En un ejercicio de laboratorio esto permite mapear objetivos disponibles.
pub fn discover_hosts(subnet: &str) -> Vec<String> {
    let mut alive = Vec::new();
    // Acepta formatos "10.0.0" (rango /24 implícito) o "10.0.0.0/24".
    let base = subnet
        .split('/')
        .next()
        .unwrap_or(subnet)
        .trim()
        .to_string();
    let octets: Vec<u8> = base.split('.').filter_map(|o| o.parse().ok()).collect();
    if octets.len() < 3 {
        info!("LATERAL: subred no válida para barrido: {subnet}");
        return alive;
    }
    let prefix = format!("{}.{}.{}", octets[0], octets[1], octets[2]);
    let start = if octets.len() == 4 { octets[3] } else { 1 };
    for host in start..=254u8 {
        let ip = format!("{prefix}.{host}");
        // Barrido con timeout corto; solo lectura de alcanzabilidad.
        if let Ok(out) = Command::new("ping")
            .args(["-c", "1", "-W", "1", &ip])
            .output()
        {
            if out.status.success() {
                alive.push(ip);
            }
        }
    }
    info!(
        "LATERAL (recon): {} host(s) vivos en {prefix}.x — barrido de solo lectura",
        alive.len()
    );
    alive
}
