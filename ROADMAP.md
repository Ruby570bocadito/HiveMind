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
- [x] **Concurrency test suite for the arena** (ronda 9): three exhaustive
  loom models over the REAL `shared_arena.rs` (included by `#[path]` in the
  `loom-model` crate, compiled with `--cfg loom`): concurrent registry claims
  (distinct slots, no garbage identities), message publish→acquire-read
  (unique seqs, incl. the seq==0 corner), and DEAD-mark racing a reservation
  (sticky bit survives). Writing them forced the ronda-9 memory-model
  hardening of the registry (see below) and surfaced a loom gotcha now
  documented in `loom-model`: its atomics must be CONSTRUCTED — casting
  zeroed shm memory into them produces unregistered cells and impossible
  behavior.
- [x] **ML roundtrip in CI** (ronda 8): `worker::tests::embedded_scout_model_roundtrip`
  decrypts the embedded model, parses it with `hive_base::ml::RandomForest`
  and classifies — no Python needed. Also exposed and fixed a real bug:
  `from_binary` trusted untrusted headers and could attempt ~512 GB
  allocations on corrupt input (now bounded and rejected).
- [x] **CI repair** (ronda 8 + ronda 9): `ci.yml` had a malformed trigger
  (`branches: aster]`) and still built `weaver` (removed in ronda 2) — CI
  could not have been running. Ronda 8 modernized the jobs (tests via
  default-members, builds of the 4 agents + c2-server, clippy surfaced,
  Python syntax check, release builds include queen) but its diff never
  touched the `on:` block — the trigger fix claimed in its commit message
  only landed in ronda 9. Ronda 9 completed the repair: real triggers
  (`branches: [master]`), `cargo fmt --check` + `clippy -D warnings` as
  blocking gates, a dedicated loom job, an end-to-end ML pipeline job
  (dataset → train → export → parity validation, failing the build on
  export errors) and release artifacts actually uploaded.
- [x] **Arena registry memory-model hardening** (ronda 9): the ronda-6 TOCTOU
  fix only made the claim a CAS — `flags` was still read non-atomically
  (pass-1/enumerate), mutated non-atomically in `mark_agent_dead` (`|=`), and
  identity fields were written AFTER the slot was published, with no
  happens-before edge for readers. Protocol now: CAS 0→RESERVED(0x80), fill
  identity, publish with `fetch_or(ACTIVE, Release)`; DEAD is a sticky
  `fetch_or`; `role` is atomic. Layout unchanged.
- [x] **Dual-use hygiene sweep** (ronda 9): dead `PrivEscResult` struct
  (leftover from an executing era) deleted; `privesc.rs` (read-only
  enumeration, linpeas-style) and `remote_shell.rs` (C2-core shell behind the
  task_poller audit + deny-list) carry explicit decision headers like
  `lateral.rs`/`fileless.rs` already did.

## P2 — polish

- [ ] Windows CI job + cross-compile artifacts (`setup_cross.sh` already
  installs the toolchains).
- [ ] Deduplicate: two arena implementations (`arena_mgr` vs
  `platform_layer::ipc`), three C2-channel-selection systems, agent
  boilerplate (`new`/`run`/`publish_msg` per binary).
- [x] **Beekeeper TUI: wire the Log/Consensus tabs to real data sources**
  (ronda 10): the Consensus tab now maintains observer-side directive state
  from the live arena message stream (`Payload::Proposal` → pending entry,
  `Payload::Vote` → vote count, `Payload::StatusEvent`/
  `directive:` belief → approved) and the Log tab records real operator
  events (agent joins/leaves, directive transitions, connection state,
  telemetry ring laps). Both tabs were permanently empty before — no
  producer existed. HTL Events also fixed: the TUI now attaches to the real
  shm arena and reads with a PRIVATE cursor (`TelemetryBuffer::read_from`)
  instead of re-peeking the same shared batch every frame.
- [x] **Helm chart: parseable values, de-escalated securityContext, unused
  keys removed, image build/tag flow documented** (ronda 10): `values.yaml`
  had a YAML syntax error (chart could not parse at all) and deployed the
  removed `swarm` agent; the template mounted a `loot` volume, forced
  `privileged: true` + `runAsUser: 0` + `hostPID`/`hostNetwork`, granted a
  k8s ClusterRole no binary uses, and passed a file path as a `shm_open`
  name (EINVAL — agents could never attach). Defaults now: non-root,
  unprivileged, ServiceAccount-only RBAC, shm name `hive_arena` via a
  node-local `/dev/shm` hostPath. See `deploy/charts/hive/README.md`.
- [ ] Docker/lab stack follow-ups: the removed `docker/lab/` compose (broken
  YAML + worm service) and `Dockerfile.lab` are gone; remaining work is a
  Windows CI job and composing the lab docs into one walkthrough.
- [ ] Progressive lint tightening: `#![deny(clippy::unwrap_used)]` for the
  core modules (`comms`, `shared_arena`, `telemetry`, `ldc`). Baseline
  `clippy -D warnings` + `fmt --check` are CI gates since ronda 9.
- [ ] Fuzz the telemetry ring buffer with concurrent writers/readers
  (ronda 10 added `read_from` with lapped-reader clamping + unit tests; a
  commit-marker per entry would also remove the writer-reserve/reader-decode
  race on unwritten slots — tracked here).
