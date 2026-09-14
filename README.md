<p align="center">
  <img src="https://capsule-render.vercel.app/api?type=waving&color=0:0d1117,100:6C63FF&height=240&section=header&text=Hive%20Colony&fontSize=64&fontAlignY=34&animation=twinkling&fontColor=ffffff" alt="Hive Colony" width="100%"/>
</p>

<h1 align="center">🐝 Hive Colony v3.0</h1>

<p align="center">
  <strong>A multi-agent autonomous swarm framework in Rust</strong><br/>
  <sub>6 specialized agents · lock-free shared-memory IPC · zero TCP ports between agents · LLM-driven strategy</sub>
</p>

<p align="center">
  <a href="https://github.com/Ruby570bocadito/HiveMind/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/Ruby570bocadito/HiveMind/ci.yml?branch=master&style=flat-square&label=CI" alt="CI"/></a>
  <img src="https://img.shields.io/badge/rust-stable%201.82%2B-000000?style=flat-square&logo=rust" alt="Rust"/>
  <img src="https://img.shields.io/badge/version-3.0.0-6C63FF?style=flat-square" alt="Version"/>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-6C63FF?style=flat-square" alt="MIT License"/></a>
  <img src="https://img.shields.io/badge/tests-286%20passing-73d0a0?style=flat-square" alt="Tests"/>
  <img src="https://img.shields.io/badge/platform-linux%20x86__64%20%7C%20windows%20(partial)-0d1117?style=flat-square&logo=linux" alt="Platform"/>
</p>

---

> [!IMPORTANT]
> **Multi-agent swarm framework — real infrastructure, zero attack payloads.**
> Hive Colony is a multi-agent swarm framework designed for **isolated lab
> environments that you own**. Since ronda 6 the repository contains **no
> offensive modules at all**: exploits, exfiltration, sabotage, credential
> harvesting, persistence, evasion and anti-forensics were removed from the
> codebase. What remains is the real engineering: shared-memory IPC, signed
> message protocol, consensus, tasking, C2, telemetry and read-only system
> enumeration.

## Why this project is interesting

- **Agents talk over shared memory, not sockets.** Every agent memory-maps the
  same arena (`memfd_create` + `mmap`) and exchanges Ed25519-signed LdC
  messages through a **lock-free ring buffer** — no agent opens a TCP port to
  talk to another agent.
- **A real consensus layer.** Directives need weighted-reputation voting
  across agents before anything executes; reputation decays over time and
  rehabilitates after good behavior.
- **Observability built in.** A structured telemetry pipeline (HTL events,
  criticality levels, rotating JSONL files with rotation on size *and* age),
  chaos-engineering recipes with replay, and an IPC contract validator with
  fuzz targets.
- **LLM optional, not required.** The Queen can consult a local Ollama model
  for strategy, and the whole colony degrades gracefully to heuristic mode
  when no model is present.
- **Tested like a library, not a script.** 286 tests across five suites
  (237 unit incl. loom concurrency models · 9 integration · 34 lab/e2e ·
  6 c2-server), two criterion benches, and cargo-fuzz targets on the IPC and
  ring-buffer paths.

## Architecture

![Hive Colony architecture](assets/architecture.png)

**The message path:** an agent publishes an LdC message → it lands in the
next ring-buffer slot of the shared arena (sequence numbers, per-slot
signatures) → every other agent's reader picks it up on its next poll →
consensus engine tallies it. The C2 server is the *operator* boundary
(HTTP + WebSocket on `:8444`), not the agent bus.

## Quick start

```bash
git clone https://github.com/Ruby570bocadito/HiveMind
cd HiveMind

./hive.sh build          # build all 9 crates
./hive.sh test           # run the hive_base test suite (270+ tests)

./hive.sh c2             # C2 server on http://localhost:8444
./hive.sh tui            # Beekeeper operator TUI (in another terminal)
```

Full lab via Docker (C2 + six agents + SSH targets + dashboard):

```bash
./hive.sh colony         # main stack
./hive.sh lab            # SSH target lab (see scripts/lab_setup.sh)
```

`hive.sh` is a thin wrapper over `cargo`/`docker compose` — plain cargo
works too:

```bash
cargo build --workspace
cargo test -p hive_base -- --test-threads=2
cargo run -p c2-server -- --port 8444
cargo run -p beekeeper
```

<p align="center">
  <img src="assets/tui_demo.gif" alt="Beekeeper TUI demo" width="80%"/>
</p>

## The colony

| Agent | Role | What it actually does |
|-------|------|----------------------|
| **Queen** | Overmind | HiveMind directive consensus, reputation ledger, Ollama LLM bridge, failover decisions |
| **Worker** | Scout | System profiling, EDR/backup process detection (8 signatures on Linux, 34 on Windows), embedded Random-Forest classifier with heuristic fallback |
| **Drone** | Shaper | Belief-driven decisions, lab host discovery, dead-agent genome regeneration (in-memory) |
| **Honeybee** | Hoarder | Consensus participant, remote shell exec, in-memory genome snapshots (destructive actions removed from the build) |

Every agent:

- checks the colony-wide **kill switch** (`beekeeper kill-switch --confirm`)
  before anything else in its message loop,
- runs the **TaskPoller** (ronda 4): pulls `GET /task/:agent_id` from the C2
  and streams results back via `POST /beacon`, closing the operator loop
  (shell tasks execute with audit; destructive/exfil task types are
  **rejected**, ronda 6),
- respects `hive.toml` (`HiveConfig::load()`),
- shuts down cleanly when the Queen dies (the "death dance").

## Engineering quality

| Area | Status |
|------|--------|
| Tests | **286 passing** — 237 unit (incl. loom models over the arena, Lua console + embedded-ML roundtrip) + 43 integration (phase-A scenarios, arena regressions, lab/e2e) + 6 c2-server (auth, rate limiter) |
| CI | GitHub Actions: `cargo fmt --check` + `cargo clippy -D warnings` (blocking), full workspace tests, loom job over `shared_arena.rs`, end-to-end ML pipeline (train → export → parity gate), release artifacts uploaded |
| Fuzzing | `cargo-fuzz` targets: IPC contract validation, ring-buffer ops |
| Benches | criterion: HTL throughput, IPC validation |
| Lints | zero clippy warnings across all 9 crates |
| Config | single `hive.toml`, loaded by every agent, loud failures on parse errors |

Repository layout:

```
hive_base/    shared library: arena IPC, LdC protocol, consensus, telemetry,
              config (43 modules)
agents/       queen · worker · drone · honeybee
c2/           Rust C2 server (axum + SQLite, :8444)
beekeeper/    operator TUI (ratatui + Lua scripting)
stinger/      launcher: fileless agent execution via memfd (lab-gated)
buzz/         dev harness: boots a local colony, tears it down
training/     Python ML: dataset gen, RF classifier, .bin exporter, DQN/PPO experiments
tests/        end-to-end lab scripts + Python reference C2
deploy/       Helm chart · docker compose labs
```

## Documentation

| Doc | Contents |
|-----|----------|
| [docs/README.md](docs/README.md) | documentation index & reading order |
| [docs/AGENTS.md](docs/AGENTS.md) | per-agent reference, message payloads, env vars |
| [docs/OPERATOR_GUIDE.md](docs/OPERATOR_GUIDE.md) | environment variables, TUI, subcommands |
| [docs/API.md](docs/API.md) | C2 HTTP/WS API (routes verified against code) |
| [docs/CAPABILITIES.md](docs/CAPABILITIES.md) | capability matrix: what is real, what is gated |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | build, test, conventions |
| [docs/NAMING.md](docs/NAMING.md) | old ↔ current name mapping (Scout→Worker, …) |
| [ROADMAP.md](ROADMAP.md) | known gaps and planned work |

## Honest limitations

Kept public on purpose — a portfolio should know what it isn't:

- The Windows agent paths compile but have no CI coverage or tested artifacts.
- The scout `.bin` model shipped in `agents/worker/models/` is a pre-trained
  fixture; the full regeneration path is now reproducible end-to-end:
  `generate_dataset.py` → `train_classifier.py` → `export_bin.py` (validated
  against sklearn predictions before build.rs embeds it).
- `rlua` is archived upstream; migration to `mlua` is tracked in the roadmap.
- Destructive/offensive capabilities **do not exist** in this codebase
  (ronda 6): exploits, exfiltration, sabotage, credential harvesting,
  persistence, evasion and anti-forensics were **deleted**, not emulated
  (history: hard-disabled in ronda 1, emulated in rondas 4–5, removed in
  ronda 6). [docs/CAPABILITIES.md](docs/CAPABILITIES.md) is the matrix of
  what remains.

## Security & ethics

- **Target only systems you own or have written authorization to test.**
- The repository contains no offensive modules; nothing to enable. The C2
  enforces optional `x-api-key` auth, per-IP rate limiting and closed CORS
  by default.
- All credentials in `docker/` are throwaway lab credentials; no secrets are
  committed (a lab SSH keypair is generated at image build time).
- The kill switch (`beekeeper kill-switch --confirm`) is the operator's
  emergency stop for every agent in the colony.

## License

MIT — see [LICENSE](LICENSE).
