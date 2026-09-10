# Roadmap

Prioritized list of known gaps and planned work. Items marked ✅ are done.

## P0 — done

- ✅ Workspace compiles from a clean clone (`stinger` no longer requires
  pre-built binaries to compile; it warns and skips the spawn step instead)
- ✅ Honest CI: `fmt --check`, `clippy -D warnings`, full workspace build,
  release artifacts uploaded
- ✅ `LICENSE` added (MIT), scripts made executable, `hive.sh` wrapper created
- ✅ `hive.toml` actually loaded by `HiveConfig::load()` (safe defaults shipped)
- ✅ Arena read-path regressions fixed and covered by tests (seq-0 wedge,
  telemetry flush data loss, timestamp underflows)
- ✅ Kill switch implemented end-to-end (operator broadcast → every agent exits)
- ✅ Dead dependencies removed; panic strategy changed so the ML fallback
  (`catch_unwind`) actually works in release builds

## P1 — next

- [ ] **Agent-side C2 task poller**: agents currently receive commands through
  the shared arena; the C2 shell (`/shell/:session_id`) queues tasks at
  `GET /task/:agent_id` and streams results back, but no agent polls the HTTP
  task queue yet. This is the last missing link of the operator shell.
- [ ] **ML model converter**: script that converts the `training/` exports
  (`.onnx` / `.joblib` / `.json`) into the runtime `.bin` format consumed by
  `hive_base::ml::RandomForest::from_binary`, plus a test on roundtrip parity.
- [ ] **Remove module theater or make it real**: `seer` feedback loop returns
  zeros (acknowledged in code), `reactive_llm::reactive_cycle` compiles a
  never-declared `variant.rs`, `io_uring_ops` has an incorrect ABI and no
  callers, `stack_spoof` has no consumers. Each one: implement honestly or
  delete.
- [ ] **Split `hive_base` with cargo features** (`core`, `tactics`, `chaos`,
  `cloud`, `telemetry-file`, `windows`) — 67 unconditioned `pub mod`s is too
  much surface for one crate.
- [ ] **C2 hardening**: optional bearer token on `/task` and `/admin/*`,
  replace `CorsLayer::permissive()`, document a TLS reverse-proxy setup.
- [ ] **`mlua` migration** (`rlua` is archived and emits a future-incompat note).
- [ ] **Concurrency test suite for the arena**: loom-based tests for the
  ring buffer (multi-writer / multi-reader), reader/writer cursor invariants.

## P2 — polish

- [ ] Windows CI job + cross-compile artifacts (`setup_cross.sh` already
  installs the toolchains).
- [ ] Deduplicate: two arena implementations (`arena_mgr` vs
  `platform_layer::ipc`), three C2-channel-selection systems, agent
  boilerplate (`new`/`run`/`publish_msg` per binary).
- [ ] Beekeeper TUI: wire the Log/Consensus tabs to real data sources or
  remove them; Lua input per-tab key handling.
- [ ] Helm chart: documented image build/tag flow, de-escalated
  securityContext defaults, remove unused ConfigMap keys.
- [ ] Progressive lint tightening: `#![deny(clippy::unwrap_used)]` for the
  core modules (`comms`, `shared_arena`, `telemetry`, `ldc`).
- [ ] Fuzz the telemetry ring buffer with concurrent writers/readers.
