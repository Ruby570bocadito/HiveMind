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
  machinery); tactics modules simulate with labelled telemetry. Superseded
  by the ronda 6 removal — see [docs/CAPABILITIES.md](docs/CAPABILITIES.md).
- [x] **Remove module theater or make it real** (ronda 6): the owner decided
  "real or delete" — all simulated/offensive modules were **deleted**
  (exploits, exfil, nectar, leech, saboteur, kerberos, smb, hades_gate,
  stack_spoof, anti_analysis, anti_forensics, cloud_worker, honeycomb,
  seer, channel_rotator, death_dance, swarm worm agent, C2 /collect loot
  endpoint, syscalls::windows). `reactive_llm::reactive_cycle` and
  `io_uring_ops` ABI remain as P1 cleanup items.
- ✅ ML model converter (ronda 7): `training/export_bin.py` converts a sklearn
  RandomForest (`.joblib`) into the runtime `.bin` format consumed by
  `hive_base::ml::RandomForest::from_binary`; wired into `train_classifier.py`
  with a validation step that replicates the Rust evaluation in NumPy and
  checks parity against sklearn (byte-exact layout, leaf sentinels -1).
- [x] **Fix or delete the remaining dead paths** (ronda 7): `reactive_llm`
  (LLM polymorphic obfuscator + binary mutator — real evasion code, no
  callers) and `io_uring_ops` (RingReaper-style EDR-bypass I/O) were
  **deleted**, along with the legacy `train_model.py`. See
  `docs/CAPABILITIES.md`.
- [ ] **Split `hive_base` with cargo features** (`core`, `telemetry`,
  `windows`) — DEFERRED (ronda 8 analysis): the 43 remaining modules are all
  load-bearing for the colony/c2 binary set; gating transport (tokio/reqwest/
  tungstenite) would force every agent crate to opt in, for little real gain
  at this scale. Revisit if hive_base grows again or a no-std/embedded target
  appears. Platform code stays target-gated (`winapi`, `libc`).
- [x] **`mlua` migration** (ronda 8): `rlua 0.19` (archived upstream) replaced
  by `mlua 0.10` (lua54, vendored) in `beekeeper`; `scripting::LuaEngine`
  rewritten 1:1 (same public API, TUI untouched) + 4 unit tests. The
  future-incompat warning is gone from `cargo check`.
- [ ] **Concurrency test suite for the arena**: loom-based tests for the
  ring buffer (multi-writer / multi-reader), reader/writer cursor invariants.
- [x] **ML roundtrip in CI** (ronda 8): `worker::tests::embedded_scout_model_roundtrip`
  decrypts the embedded model, parses it with `hive_base::ml::RandomForest`
  and classifies — no Python needed. Also exposed and fixed a real bug:
  `from_binary` trusted untrusted headers and could attempt ~512 GB
  allocations on corrupt input (now bounded and rejected).
- [x] **CI repair** (ronda 8): `ci.yml` had a malformed trigger
  (`branches: aster]`) and still built `weaver` (removed in ronda 2) — CI
  could not have been running. Fixed triggers, modernized jobs (tests via
  default-members, builds of the 4 agents + c2-server, clippy surfaced,
  Python syntax check for the training pipeline, release builds include queen).

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
