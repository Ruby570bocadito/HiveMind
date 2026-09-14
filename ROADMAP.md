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

- [x] **Agent-side C2 task poller** (ronda 4): `hive_base::task_poller` polls
  `GET /task/:agent_id`, executes under the emulation policy and streams
  results back via `POST /beacon`; wired into all five agents via
  `HIVE_C2_URL` / `HIVE_C2_API_KEY` / `HIVE_POLL_SECS`.
- [x] **C2 hardening** (ronda 3): `x-api-key` auth (constant-time compare),
  rate limiter, CORS allow-list, documented TLS reverse-proxy setup.
- [x] **Emulation policy** (ronda 4): weaponized code removed (exploit
  payloads, PE obfuscator, stagers, deploy vectors, NtCreateSection
  machinery); tactics modules simulate with labelled telemetry;
  [docs/EMULATION.md](docs/EMULATION.md) is the reference matrix.
- [ ] **ML model converter**: script that converts the `training/` exports
  (`.onnx` / `.joblib` / `.json`) into the runtime `.bin` format consumed by
  `hive_base::ml::RandomForest::from_binary`, plus a test on roundtrip parity.
- [ ] **Remove module theater or make it real**: `seer` feedback loop returns
  zeros (acknowledged in code), `reactive_llm::reactive_cycle` compiles a
  never-declared `variant.rs`, `io_uring_ops` has an incorrect ABI and no
  callers. Each one: implement honestly or delete. (`stack_spoof` was
  resolved by the ronda 4 emulation policy.)
- [ ] **Split `hive_base` with cargo features** (`core`, `tactics`, `chaos`,
  `cloud`, `telemetry-file`, `windows`) — still ~60 unconditioned `pub mod`s.
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
