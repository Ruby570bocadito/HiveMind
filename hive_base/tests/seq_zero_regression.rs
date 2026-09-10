// Regression tests for the shared-memory arena read path.
//
// Bug being guarded against: `HiveChamber::read_new` used to treat
// `slot_seq == 0` as "slot not yet written", but the very first message
// published in an arena legitimately has sequence number 0. Any reader
// that connected *before* the first publish therefore wedged at seq 0
// and missed every message until the ring buffer wrapped around
// (MAX_MESSAGES publishes later).

use hive_base::ldc::Payload;
use hive_base::{AgentIdentity, HiveChamber, Message, Role};

/// A reader that connects BEFORE the first publish must still receive the
/// first message (seq == 0) as soon as it appears.
#[test]
fn reader_connected_before_first_publish_receives_seq_zero_message() {
    // Isolated arena for this test: unique shm name + restore env afterwards.
    let arena_name = format!("/hive_test_seq0_{}", std::process::id());
    std::env::set_var("__HIVE_ARENA", &arena_name);
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");

    rt.block_on(async {
        // 1. Reader connects to a fresh (empty) arena.
        let reader_identity = AgentIdentity::new();
        let reader = HiveChamber::connect(&reader_identity, Role::Worker)
            .await
            .expect("reader connect");

        // 2. Nothing published yet: read must return an empty list.
        let empty = reader.read_new().await;
        assert!(empty.is_empty(), "fresh arena should have no messages");

        // 3. A second agent publishes the FIRST message of the arena
        //    (slot sequence number 0).
        let writer_identity = AgentIdentity::new();
        let writer = HiveChamber::connect(&writer_identity, Role::Queen)
            .await
            .expect("writer connect");
        let msg = Message::status_event(
            writer_identity.id(),
            Role::Queen,
            "test_ping",
            writer_identity.id(),
            Role::Queen,
            "seq zero regression",
        );
        writer.publish(msg).await;

        // 4. Reader must pick up message seq 0 immediately.
        let msgs = reader.read_new().await;
        assert_eq!(
            msgs.len(),
            1,
            "reader missed the first arena message (seq 0 wedge regression)"
        );
        match &msgs[0].payload {
            Payload::StatusEvent { event_type, .. } => {
                assert_eq!(event_type, "test_ping");
            }
            other => panic!("unexpected payload: {other:?}"),
        }
    });

    // Cleanup: remove the shm object so repeated runs start fresh.
    let cname = std::ffi::CString::new(arena_name.trim_start_matches('/')).expect("cstring");
    unsafe {
        libc::shm_unlink(cname.as_ptr());
    }
    std::env::remove_var("__HIVE_ARENA");
}

/// A kill-switch broadcast (Payload::StatusEvent with event_type
/// "kill_switch") must be recognized by Message::is_kill_switch.
#[test]
fn kill_switch_message_is_detected() {
    let id = uuid::Uuid::new_v4();
    let kill = Message::status_event(
        id,
        Role::Queen,
        "kill_switch",
        id,
        Role::Queen,
        "self_destruct",
    );
    assert!(kill.is_kill_switch(), "kill_switch event must be detected");

    let normal = Message::status_event(id, Role::Queen, "agent_dead", id, Role::Worker, "x");
    assert!(!normal.is_kill_switch(), "other events must not match");
}
