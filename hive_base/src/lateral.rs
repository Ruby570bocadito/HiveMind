//! Lateral RECON — Hive Colony.
//!
//! Ronda 6: la recolección de credenciales, la ejecución remota por SSH y el
//! despliegue/propagación de agentes fueron ELIMINADOS del repositorio
//! (histórico: weaver fuera en ronda 2, deploy SSH simulado en ronda 4,
//! eliminación total en ronda 6). Lo que queda aquí es únicamente
//! descubrimiento de red de solo lectura para laboratorios propios:
//!
//! - `discover_hosts()`: barrido ping sweep del segmento de laboratorio.
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
use std::process::Command;
use tracing::info;

/// Concurrencia del pool de pings (ronda 12): 32 sondas simultáneas.
/// Antes el barrido era secuencial — 254 pings de 1 s podían tardar
/// ~4 minutos, inasumible para un ejecutor de directivas en runtime.
const SCAN_CONCURRENCY: usize = 32;

/// Barrido de descubrimiento REAL (solo lectura): ping sweep del segmento.
/// En un ejercicio de laboratorio esto permite mapear hosts disponibles.
///
/// Ronda 12: paralelizado con un pool acotado de hilos (`SCAN_CONCURRENCY`),
/// resultados en orden determinista (mismo orden que el barrido secuencial).
pub fn discover_hosts(subnet: &str) -> Vec<String> {
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
        return Vec::new();
    }
    let prefix = format!("{}.{}.{}", octets[0], octets[1], octets[2]);
    let start = if octets.len() == 4 { octets[3] } else { 1 };
    let targets: Vec<String> = (start..=254u8)
        .map(|host| format!("{prefix}.{host}"))
        .collect();

    // Pool acotado: trocea la lista y sondea cada trozo en paralelo.
    let alive: Vec<String> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in targets.chunks(SCAN_CONCURRENCY) {
            let chunk = chunk.to_vec();
            handles.push(scope.spawn(move || {
                chunk
                    .into_iter()
                    .filter(|ip| host_reachable(ip))
                    .collect::<Vec<String>>()
            }));
        }
        let mut alive = Vec::new();
        for handle in handles {
            // El cuerpo solo hace join de hilos propios que ya terminan;
            // un panic en un hijo (no esperado) no debe tumbar al barrido.
            if let Ok(part) = handle.join() {
                alive.extend(part);
            }
        }
        alive
    });

    info!(
        "LATERAL (recon): {} host(s) vivos en {prefix}.x — barrido de solo lectura",
        alive.len()
    );
    alive
}

/// Sonda de alcanzabilidad (solo lectura): un ping con timeout corto.
fn host_reachable(ip: &str) -> bool {
    matches!(
        Command::new("ping").args(["-c", "1", "-W", "1", ip]).output(),
        Ok(out) if out.status.success()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_hosts_invalid_subnet_is_empty_and_fast() {
        // Sin tercer octeto no hay barrido: devuelve vacío sin lanzar pings.
        let started = std::time::Instant::now();
        assert!(discover_hosts("no-es-una-subred").is_empty());
        assert!(started.elapsed().as_secs() < 2);
    }

    #[test]
    fn test_discover_hosts_loopback_finds_self() {
        // El loopback SIEMPRE responde… si el entorno permite sockets raw.
        // En contenedores sin cap_net_raw el propio `ping` falla para
        // CUALQUIER host — el barrido honesto devuelve vacío. El test
        // comprueba primero la capacidad y afirma según ella.
        let ping_capable = Command::new("ping")
            .args(["-c", "1", "-W", "1", "127.0.0.1"])
            .output()
            .is_ok_and(|out| out.status.success());
        let alive = discover_hosts("127.0.0");
        if ping_capable {
            assert!(alive.contains(&"127.0.0.1".to_string()));
        } else {
            // Sin capacidad raw: barrido vacío, no simulación.
            assert!(alive.is_empty(), "sin cap_net_raw no puede haber vivos");
        }
    }
}
