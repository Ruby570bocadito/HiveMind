// Shared memory arena for inter-agent communication.
// Replaces the TCP bus (127.0.0.1:4242) with an anonymous file-backed
// memory-mapped ring buffer. No sockets, no ports, no listen() footprint.
//
// Cross-process: uses shm_open with random name on Linux.
// All agents mmap the same region and communicate via lock-free atomics.
//
// All public functions accept raw pointers intentionally — callers
// obtain valid pointers via arena_mgr::SharedArenaMapping.
//
// Ronda 9 — memory-model hardening (loom-verified):
//
// The registry slot `flags` byte is now accessed ATOMICALLY everywhere.
// History: the ronda-6 TOCTOU fix made only the claim a CAS; the rest of
// the accesses to `flags` stayed non-atomic (pass-1 reads, enumerate,
// `flags |= 2` in mark_agent_dead). Under a memory model that is a mixed
// atomic/non-atomic race on the same cell, with two real consequences:
//
//   1. mark_agent_dead's non-atomic read-modify-write could clobber a
//      concurrent CAS claim (lost update → registered agent turns
//      invisible) or be lost itself.
//   2. Identity fields (agent_id/verifying_key) used to be written AFTER
//      the slot was published as active, so a reader synchronizing on
//      flags had no happens-before edge to those writes and could match
//      or enumerate garbage.
//
// Protocol now (per slot):
//   FREE(0) --CAS--> RESERVED(0x80) --fill identity--> fetch_or(ACTIVE,
//   Release) => published. Readers gate on flags.load(Acquire): ACTIVE
//   set implies the identity writes happen-before the observation.
//   DEAD is a sticky bit applied with fetch_or, so a dead-mark racing a
//   reservation is never lost (published slot ends ACTIVE|DEAD and is
//   excluded from pass-1/enumerate).
//
// Message slots keep the publish protocol: all fields are written
// non-atomically BEFORE the seq Release store; readers load seq with
// Acquire and only then touch the rest of the slot.
//
// Loom: this file is compiled as-is into the `loom-model` crate via
// #[path] with `--cfg loom` (atomic shims below); MAX_AGENTS/MAX_MESSAGES
// shrink under loom to keep the state space enumerable.

#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::mem;
use std::ptr;

#[cfg(loom)]
use loom::sync::atomic::{AtomicU64, AtomicU8, Ordering};
#[cfg(not(loom))]
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

// ── constants ────────────────────────────────────────────────────────────────

pub const MAX_MSG_SIZE: usize = 8192;
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 30;

// Loom explores every interleaving of atomic operations, so the model runs
// with the smallest layout that still exercises the protocols (2 agents, 2
// message slots). Production keeps the real dimensions.
#[cfg(not(loom))]
pub const MAX_AGENTS: usize = 16;
#[cfg(loom)]
pub const MAX_AGENTS: usize = 2;
#[cfg(not(loom))]
pub const MAX_MESSAGES: usize = 2048;
#[cfg(loom)]
pub const MAX_MESSAGES: usize = 2;

// ── agent slot flag bits (protocol shared by all processes) ───────────────

/// Free slot — the only value a CAS claim accepts.
pub const FLAGS_FREE: u8 = 0;
/// Published/active agent (bit 0).
pub const FLAGS_ACTIVE: u8 = 1;
/// Sticky tombstone (bit 1) — set with fetch_or, never cleared.
pub const FLAGS_DEAD: u8 = 2;
/// Reservation mark (bit 7) — owned by the winning CAS until publish.
pub const FLAGS_RESERVED: u8 = 0x80;

const ARENA_MAGIC: [u8; 8] = [0x53, 0x57, 0x52, 0x4D, 0x01, 0x00, 0x00, 0x00]; // "SWRM\1\0\0\0"

// ── arena header (first cache line) ──────────────────────────────────────────

#[repr(C, align(64))]
pub struct ArenaHeader {
    pub magic: [u8; 8],
    pub arena_size: u64,
    pub write_cursor: AtomicU64,
    pub agent_registry_seq: AtomicU64,
    pub _pad: [u8; 40],
}

// ── agent registry slot ──────────────────────────────────────────────────────

#[repr(C)]
pub struct AgentSlot {
    pub agent_id: [u8; 16],
    pub last_heartbeat: AtomicU64,
    pub verifying_key: [u8; 32],
    /// Read concurrently by enumerate/pass-1 — atomic so role updates are
    /// never torn. Same size/alignment as the old u8 field: the on-disk
    /// layout is unchanged.
    pub role: AtomicU8,
    /// The registry state byte. Only ever touched with atomic RMW/load
    /// (see module docs for the state machine).
    pub flags: AtomicU8,
    pub _pad: [u8; 6],
}

// ── message slot ─────────────────────────────────────────────────────────────

#[repr(C, align(64))]
pub struct MessageSlot {
    pub seq: AtomicU64,
    pub timestamp: u64,
    pub agent_id: [u8; 16],
    pub verifying_key: [u8; 32],
    pub signature: [u8; 64],
    pub role: u8,
    pub payload_len: u32,
    pub _pad: [u8; 3],
    pub payload: [u8; MAX_MSG_SIZE],
}

// ── sizes ────────────────────────────────────────────────────────────────────

const HEADER_SIZE: usize = mem::size_of::<ArenaHeader>();
const AGENT_SLOT_SIZE: usize = mem::size_of::<AgentSlot>();
const MESSAGE_SLOT_SIZE: usize = mem::size_of::<MessageSlot>();

pub const fn arena_layout_size() -> usize {
    HEADER_SIZE + (MAX_AGENTS * AGENT_SLOT_SIZE) + (MAX_MESSAGES * MESSAGE_SLOT_SIZE)
}

// ── operations ───────────────────────────────────────────────────────────────

pub fn verify_arena(ptr: *const u8) -> bool {
    unsafe {
        let header: *const ArenaHeader = ptr as *const ArenaHeader;
        (*header).magic == ARENA_MAGIC
    }
}

pub fn init_arena(ptr: *mut u8) {
    unsafe {
        let header = ptr as *mut ArenaHeader;
        ptr::write_bytes(ptr, 0, arena_layout_size());
        (*header).magic = ARENA_MAGIC;
        (*header).arena_size = arena_layout_size() as u64;
        (*header).write_cursor = AtomicU64::new(0);
        (*header).agent_registry_seq = AtomicU64::new(0);
    }
}

pub fn write_cursor_ref(ptr: *const u8) -> &'static AtomicU64 {
    unsafe {
        let header = ptr as *const ArenaHeader;
        &(*header).write_cursor
    }
}

pub fn claim_slot(ptr: *mut u8) -> (u64, usize) {
    let cursor_ref = write_cursor_ref(ptr);
    let seq = cursor_ref.fetch_add(1, Ordering::AcqRel);
    let slot_idx = (seq as usize) % MAX_MESSAGES;
    (seq, slot_idx)
}

pub fn message_slot_mut(ptr: *mut u8, slot_idx: usize) -> *mut MessageSlot {
    unsafe {
        let offset = HEADER_SIZE + (MAX_AGENTS * AGENT_SLOT_SIZE) + (slot_idx * MESSAGE_SLOT_SIZE);
        (ptr.add(offset)) as *mut MessageSlot
    }
}

pub fn message_slot_ptr(ptr: *const u8, slot_idx: usize) -> *const MessageSlot {
    unsafe {
        let offset = HEADER_SIZE + (MAX_AGENTS * AGENT_SLOT_SIZE) + (slot_idx * MESSAGE_SLOT_SIZE);
        (ptr.add(offset)) as *const MessageSlot
    }
}

pub fn read_slot_seq(slot: *const MessageSlot) -> u64 {
    unsafe { (*slot).seq.load(Ordering::Acquire) }
}

#[allow(clippy::too_many_arguments)]
pub fn write_message_slot(
    slot: *mut MessageSlot,
    seq: u64,
    timestamp: u64,
    agent_id: [u8; 16],
    verifying_key: [u8; 32],
    signature: [u8; 64],
    role: u8,
    data: &[u8],
) {
    unsafe {
        let len = data.len().min(MAX_MSG_SIZE);
        (*slot).timestamp = timestamp;
        (*slot).agent_id = agent_id;
        (*slot).verifying_key = verifying_key;
        (*slot).signature = signature;
        (*slot).role = role;
        (*slot).payload_len = len as u32;
        ptr::copy_nonoverlapping(data.as_ptr(), (*slot).payload.as_mut_ptr(), len);
        (*slot).seq.store(seq, Ordering::Release);
    }
}

// ── agent registry ───────────────────────────────────────────────────────────

pub fn agent_slot_mut(ptr: *mut u8, idx: usize) -> *mut AgentSlot {
    unsafe {
        let offset = HEADER_SIZE + (idx * AGENT_SLOT_SIZE);
        (ptr.add(offset)) as *mut AgentSlot
    }
}

pub fn agent_slot_ptr(ptr: *const u8, idx: usize) -> *const AgentSlot {
    unsafe {
        let offset = HEADER_SIZE + (idx * AGENT_SLOT_SIZE);
        (ptr.add(offset)) as *const AgentSlot
    }
}

/// Registers an agent, claiming (or reusing) its registry slot.
///
/// `role` and `verifying_key` are part of the claim itself: they are written
/// while the slot is still RESERVED and become visible exactly when the
/// ACTIVE bit is published with Release — no post-publish writes to
/// non-atomic identity fields, no torn reads for enumerate/pass-1.
///
/// Returns the slot index, `None` if the registry is full (or every free
/// slot was lost to the CAS race, which is the correct outcome for the
/// losers).
pub fn find_or_claim_agent_slot(
    ptr: *mut u8,
    agent_id: [u8; 16],
    role: u8,
    verifying_key: [u8; 32],
) -> Option<usize> {
    unsafe {
        // Pass 1: already registered with this exact id?
        for i in 0..MAX_AGENTS {
            let slot = agent_slot_ptr(ptr, i);
            let flags_ptr = (&raw const (*slot).flags).cast::<AtomicU8>();
            let flags = (*flags_ptr).load(Ordering::Acquire);
            if (flags & FLAGS_ACTIVE) != 0
                && (flags & FLAGS_DEAD) == 0
                && (*slot).agent_id == agent_id
            {
                return Some(i);
            }
        }
        // Pass 2: atomically reserve a FREE slot (0 -> RESERVED). The
        // ronda-6 TOCTOU note applies here: the CAS is what makes exactly
        // one concurrent claimer win each slot. Identity is filled while
        // RESERVED, then published by setting ACTIVE with fetch_or so a
        // dead-mark racing the reservation survives (sticky DEAD bit).
        for i in 0..MAX_AGENTS {
            let slot = agent_slot_mut(ptr, i);
            let flags = (&raw mut (*slot).flags).cast::<AtomicU8>();
            if (*flags)
                .compare_exchange(
                    FLAGS_FREE,
                    FLAGS_RESERVED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                (*slot).agent_id = agent_id;
                (*slot).verifying_key = verifying_key;
                (*slot).last_heartbeat = AtomicU64::new(0);
                (*slot).role.store(role, Ordering::Release);
                // Publish. fetch_or (not store): preserves a DEAD mark set
                // by a reaper/chaos sweep while this thread was reserving.
                (*flags).fetch_or(FLAGS_ACTIVE, Ordering::Release);
                return Some(i);
            }
        }
        None
    }
}

pub fn update_heartbeat(ptr: *mut u8, idx: usize, ts: u64) {
    unsafe {
        let slot = agent_slot_mut(ptr, idx);
        (*slot).last_heartbeat.store(ts, Ordering::Release);
    }
}

pub fn enumerate_agents<F>(ptr: *const u8, mut f: F)
where
    F: FnMut([u8; 16], u8, u64, [u8; 32]),
{
    for i in 0..MAX_AGENTS {
        unsafe {
            let slot = agent_slot_ptr(ptr, i);
            // Acquire on the flag byte is the synchronization point: an
            // ACTIVE observation guarantees the identity fields written
            // before the publisher's Release are visible here.
            let flags_ptr = (&raw const (*slot).flags).cast::<AtomicU8>();
            let flags = (*flags_ptr).load(Ordering::Acquire);
            if (flags & FLAGS_ACTIVE) != 0 && (flags & FLAGS_DEAD) == 0 {
                let hb = (*slot).last_heartbeat.load(Ordering::Acquire);
                let role = (*slot).role.load(Ordering::Acquire);
                f((*slot).agent_id, role, hb, (*slot).verifying_key);
            }
        }
    }
}

pub fn mark_agent_dead(ptr: *mut u8, idx: usize) {
    unsafe {
        let slot = agent_slot_mut(ptr, idx);
        let flags_ptr = (&raw mut (*slot).flags).cast::<AtomicU8>();
        // Atomic RMW: can no longer clobber a concurrent CAS claim nor lose
        // its own mark to one (the ronda-6 non-atomic `flags |= 2`).
        (*flags_ptr).fetch_or(FLAGS_DEAD, Ordering::AcqRel);
    }
}

pub fn last_heartbeat_val(ptr: *const u8, idx: usize) -> u64 {
    unsafe {
        let slot = agent_slot_ptr(ptr, idx);
        (*slot).last_heartbeat.load(Ordering::Acquire)
    }
}

pub fn agent_role_val(ptr: *const u8, idx: usize) -> u8 {
    unsafe {
        let slot = agent_slot_ptr(ptr, idx);
        (*slot).role.load(Ordering::Acquire)
    }
}

pub fn agent_id_val(ptr: *const u8, idx: usize) -> [u8; 16] {
    unsafe {
        let slot = agent_slot_ptr(ptr, idx);
        (*slot).agent_id
    }
}

pub fn agent_flags_val(ptr: *const u8, idx: usize) -> u8 {
    unsafe {
        let slot = agent_slot_ptr(ptr, idx);
        let flags_ptr = (&raw const (*slot).flags).cast::<AtomicU8>();
        (*flags_ptr).load(Ordering::Acquire)
    }
}

pub fn generate_arena_name() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let suffix: String = (0..16)
        .map(|_| format!("{:02x}", rng.gen::<u8>()))
        .collect();
    format!("/swarm_{}", suffix)
}

pub fn arena_size() -> usize {
    let sz = arena_layout_size() + TELEMETRY_REGION_SIZE;
    (sz + 4095) & !4095
}

// ── telemetry region (appended after message slots) ─────────────────────

/// Size of the telemetry ring buffer data area (4 MB)
pub const TELEMETRY_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Offset from arena start where telemetry region begins
pub fn telemetry_region_offset() -> usize {
    let sz = arena_layout_size();
    (sz + 63) & !63 // align to 64 bytes
}

/// Total telemetry region size (header + data buffer)
pub const TELEMETRY_REGION_SIZE: usize = 64 + TELEMETRY_BUFFER_SIZE;

#[repr(C, align(64))]
pub struct TelemetryHeader {
    pub write_cursor: AtomicU64,
    pub read_cursor: AtomicU64,
    pub _pad: [u8; 48],
}

pub fn telemetry_header_ref(ptr: *const u8) -> &'static TelemetryHeader {
    unsafe {
        let offset = telemetry_region_offset();
        &*(ptr.add(offset) as *const TelemetryHeader)
    }
}

pub fn telemetry_header_mut(ptr: *mut u8) -> &'static mut TelemetryHeader {
    unsafe {
        let offset = telemetry_region_offset();
        &mut *(ptr.add(offset) as *mut TelemetryHeader)
    }
}

pub fn telemetry_data_ptr(ptr: *const u8) -> *const u8 {
    unsafe {
        let offset = telemetry_region_offset() + 64;
        ptr.add(offset)
    }
}

pub fn telemetry_data_mut(ptr: *mut u8) -> *mut u8 {
    unsafe {
        let offset = telemetry_region_offset() + 64;
        ptr.add(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    #[cfg(not(loom))] // production dimensions (loom shrinks both consts)
    fn test_arena_constants() {
        const _: () = {
            assert!(MAX_AGENTS >= 8);
            assert!(MAX_MESSAGES >= 512);
            assert!(MAX_MSG_SIZE >= 4096);
            assert!(HEARTBEAT_TIMEOUT_SECS >= 10);
        };
    }

    #[test]
    fn test_arena_magic() {
        assert_eq!(&ARENA_MAGIC, b"SWRM\x01\x00\x00\x00");
    }

    #[test]
    fn test_header_size_alignment() {
        assert_eq!(
            mem::size_of::<ArenaHeader>() % 64,
            0,
            "Header must be cache-line aligned"
        );
    }

    #[test]
    fn test_agent_slot_size() {
        let sz = mem::size_of::<AgentSlot>();
        assert!(sz >= 64, "Agent slot too small: {}", sz);
    }

    #[test]
    fn test_message_slot_size() {
        let sz = mem::size_of::<MessageSlot>();
        assert!(sz >= MAX_MSG_SIZE, "Message slot too small for max message");
    }
}
