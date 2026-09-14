//! Lateral RECON — Hive Colony.
//!
//! Ronda 6: la recolección de credenciales, la ejecución remota por SSH y el
//! despliegue/propagación de agentes fueron ELIMINADOS del repositorio
//! (histórico: weaver fuera en ronda 2, deploy SSH simulado en ronda 4,
//! eliminación total en ronda 6). Lo que queda aquí es únicamente
//! descubrimiento de red de solo lectura para laboratorios propios:
//!
//! - `discover_hosts()`: barrido ping sweep del segmento de laboratorio.
use std::process::Command;
use tracing::info;

/// Barrido de descubrimiento REAL (solo lectura): ping sweep del segmento.
/// En un ejercicio de laboratorio esto permite mapear hosts disponibles.
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
