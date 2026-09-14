//! Loom verification of `hive_base::shared_arena` — the real file.
//!
//! Why a separate crate: `--cfg loom` via RUSTFLAGS forces a full rebuild of
//! every crate in the build graph. Including `shared_arena.rs` here with
//! `#[path]` keeps that rebuild scoped to this zero-dependency crate while
//! still compiling the production source byte-for-byte (the file's own
//! `cfg(loom)` shims swap `std::sync::atomic` for `loom::sync::atomic` and
//! shrink MAX_AGENTS/MAX_MESSAGES to 2 so the state space stays enumerable).
//!
//! Run:
//! ```text
//! RUSTFLAGS="--cfg loom" cargo test -p loom-model --release
//! ```

#[path = "../../hive_base/src/shared_arena.rs"]
pub mod shared_arena;

#[cfg(all(test, loom))]
mod models {
    use super::shared_arena as arena;
    use loom::sync::atomic::{AtomicU64, AtomicU8};
    use loom::thread;

    /// Raw shared-memory stand-in: a buffer laid out exactly like the arena,
    /// with its base pointer handed to each thread. This mirrors the real
    /// topology (the same physical shm region mapped by every agent — no Arc
    /// refcounting involved). Send/Sync are safe here because every
    /// cross-thread access goes through the arena's atomic protocol (that
    /// IS the thing under test).
    struct ArenaPtr(*mut u8);
    unsafe impl Send for ArenaPtr {}
    unsafe impl Sync for ArenaPtr {}

    /// Builds a fresh arena buffer with EVERY atomic field CONSTRUCTED.
    ///
    /// Loom requirement: unlike std — where a zeroed AtomicU64/AtomicU8 is a
    /// valid object — loom tracks cells per constructed object; casting
    /// zeroed (alloc_zeroed / write_bytes) memory into its atomic types
    /// yields unregistered cells and impossible behavior (duplicate fetch_add
    /// results, phantom flag values, `failed to get object` panics). The
    /// header atomics are constructed inside init_arena (placement
    /// assignment); the slot/seq atomics are constructed here.
    fn new_arena() -> ArenaPtr {
        let mut buf = vec![0u8; arena::arena_layout_size()];
        let ptr = buf.as_mut_ptr();
        arena::init_arena(ptr);
        unsafe {
            for i in 0..arena::MAX_AGENTS {
                let slot = arena::agent_slot_mut(ptr, i);
                std::ptr::write(&raw mut (*slot).last_heartbeat, AtomicU64::new(0));
                std::ptr::write(&raw mut (*slot).role, AtomicU8::new(0));
                std::ptr::write(&raw mut (*slot).flags, AtomicU8::new(0));
            }
            for i in 0..arena::MAX_MESSAGES {
                let slot = arena::message_slot_mut(ptr, i);
                std::ptr::write(&raw mut (*slot).seq, AtomicU64::new(0));
            }
        }
        std::mem::forget(buf); // leaked on purpose: models run once per process
        ArenaPtr(ptr)
    }

    // ── Model 1: concurrent registry claims ────────────────────────────────
    //
    // Two agents booting together. Regression net for the ronda-6 TOCTOU fix
    // (CAS claim) AND the ronda-9 publish protocol (identity written while
    // RESERVED, published with Release): whatever the interleaving, agents
    // never share a slot and an enumerated identity is never garbage.
    #[test]
    fn registry_concurrent_claims_distinct_and_consistent() {
        loom::model(|| {
            let ArenaPtr(ptr) = new_arena();
            let id_a: [u8; 16] = *b"AAAAAAAAAAAAAAAA";
            let id_b: [u8; 16] = *b"BBBBBBBBBBBBBBBB";
            let key: [u8; 32] = [7u8; 32];

            let t1 = thread::spawn(move || unsafe {
                arena::find_or_claim_agent_slot(ptr, id_a, 1, key)
            });
            let t2 = thread::spawn(move || unsafe {
                arena::find_or_claim_agent_slot(ptr, id_b, 2, key)
            });
            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            match (r1, r2) {
                (Some(i), Some(j)) => assert_ne!(i, j, "two agents landed on one slot"),
                (Some(_), None) | (None, Some(_)) | (None, None) => {}
            }

            let mut seen = 0;
            unsafe {
                arena::enumerate_agents(ptr, |id, role, _hb, vk| {
                    assert!(
                        id == id_a || id == id_b,
                        "active slot with foreign/garbage identity: {id:?}"
                    );
                    assert!(role == 1 || role == 2, "torn role: {role}");
                    assert_eq!(vk, key, "torn verifying key");
                    seen += 1;
                });
            }
            assert!(seen <= 2);
        });
    }

    // ── Model 2: message publish → acquire read ───────────────────────────
    //
    // Two writers claim seqs via the header cursor, fill their slots
    // non-atomically and publish with a Release store on seq. Includes the
    // seq == 0 corner (the first published message of a fresh arena).
    #[test]
    fn message_publish_visible_to_acquire_reader() {
        loom::model(|| {
            let ArenaPtr(ptr) = new_arena();

            let t1 = thread::spawn(move || {
                let (seq, idx) = arena::claim_slot(ptr);
                let slot = arena::message_slot_mut(ptr, idx);
                arena::write_message_slot(
                    slot, seq, 42, [1u8; 16], [2u8; 32], [3u8; 64], 5, b"hello-a",
                );
                (seq, idx)
            });
            let t2 = thread::spawn(move || {
                let (seq, idx) = arena::claim_slot(ptr);
                let slot = arena::message_slot_mut(ptr, idx);
                arena::write_message_slot(
                    slot, seq, 42, [4u8; 16], [5u8; 32], [6u8; 64], 6, b"hello-b",
                );
                (seq, idx)
            });
            let r1 = t1.join().unwrap();
            let r2 = t2.join().unwrap();

            assert_ne!(r1.0, r2.0, "claim_slot must yield unique seqs");
            assert_ne!(r1.1, r2.1, "distinct seqs must map to distinct slots");

            unsafe {
                for (seq, idx, tag) in [(r1.0, r1.1, b"hello-a"), (r2.0, r2.1, b"hello-b")] {
                    let slot = arena::message_slot_ptr(ptr, idx);
                    let observed = arena::read_slot_seq(slot);
                    assert_eq!(observed, seq, "published seq not visible");
                    let len = (*slot).payload_len as usize;
                    assert!(len > 0 && len <= arena::MAX_MSG_SIZE, "torn payload_len");
                    let payload = &(*slot).payload;
                    assert_eq!(&payload[..len], tag, "torn payload");
                }
            }
        });
    }

    // ── Model 3: DEAD mark vs claim/publish ────────────────────────────────
    //
    // A reaper marks slot 0 dead while another thread claims it. The mark is
    // a fetch_or (sticky), the publish a fetch_or: no interleaving may lose
    // the DEAD bit, and no interleaving may resurrect a dead slot as active.
    #[test]
    fn dead_mark_racing_reservation_is_never_lost() {
        loom::model(|| {
            let ArenaPtr(ptr) = new_arena();
            let id_a: [u8; 16] = *b"AAAAAAAAAAAAAAAA";
            let key: [u8; 32] = [7u8; 32];

            let t1 = thread::spawn(move || unsafe {
                arena::find_or_claim_agent_slot(ptr, id_a, 3, key)
            });
            let t2 = thread::spawn(move || {
                // The reaper/chaos sweep marks slot 0 dead regardless of its
                // state — free, reserved, or already published.
                unsafe { arena::mark_agent_dead(ptr, 0) };
            });
            let r1 = t1.join().unwrap();
            t2.join().unwrap();

            unsafe {
                let flags0 = arena::agent_flags_val(ptr, 0);
                if flags0 & arena::FLAGS_ACTIVE != 0 {
                    assert_eq!(r1, Some(0), "active slot 0 without a claim on it");
                    assert_eq!(
                        flags0 & arena::FLAGS_DEAD,
                        arena::FLAGS_DEAD,
                        "ACTIVE published over a lost DEAD mark"
                    );
                }
                arena::enumerate_agents(ptr, |id, role, _hb, vk| {
                    assert_eq!(id, id_a, "garbage identity survived publish");
                    assert_eq!(role, 3);
                    assert_eq!(vk, key);
                });
            }
        });
    }
}
