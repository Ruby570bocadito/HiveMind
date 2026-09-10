# Development Guide

How to build, test and work on Hive Colony.

## Prerequisites

- Rust stable (1.82+) — `https://rustup.rs`
- Linux x86_64 for the agent binaries (the syscall layer is x86_64-gated;
  Windows paths exist but are not CI-tested)
- Optional: Docker + docker compose for the lab environments
- Optional: Python 3.10+ with `requirements.txt` for the ML training scripts

## Build

```bash
cargo build --workspace          # all 11 crates, debug profile
./hive.sh release                # agents in release profile (stinger embeds these)
cargo check --workspace --all-targets
```

`stinger` embeds the agent binaries via `include_bytes!`. On a clean clone
the build script detects that they are missing and compiles with empty
embeds (runtime warnings instead of a hard error). To produce a functional
stinger:

```bash
./hive.sh release
cargo build --release -p stinger
```

## Test

```bash
cargo test -p hive_base -- --test-threads=2   # unit + integration (360+ tests)
cargo test -p hive_base --test seq_zero_regression
```

Test layout:

| Location | Covers |
|----------|--------|
| `hive_base/src/**` `#[cfg(test)]` | unit tests per module (319) |
| `hive_base/tests/integration_test.rs` | no-TCP-port invariant, consensus, crypto roundtrips |
| `hive_base/tests/phase_a_integration.rs` | HTL + chaos + IPC contract pipeline scenarios |
| `hive_base/tests/seq_zero_regression.rs` | arena read-path regressions, kill switch |
| `hive_base/fuzz/` | cargo-fuzz targets for IPC validation and ring ops |
| `tests/*.sh`, `tests/*.py` | lab-level end-to-end scripts (not run in CI) |

## Lints & formatting

CI enforces both — keep them clean locally:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Project layout

```
hive_base/        Shared library: arena IPC, LdC protocol, consensus,
                  telemetry, config, and the tactics modules
agents/           Six binaries: queen, worker, drone, honeybee, weaver, swarm
c2/               Rust C2 server (axum + SQLite), port 8444
beekeeper/        Operator TUI (ratatui + Lua scripting)
stinger/          Dropper that fileless-executes the embedded agents
buzz/             Dev harness that boots a local colony and tears it down
training/         Python: dataset generation + RF/DQN/PPO training scripts
tests/            End-to-end lab scripts and a Python reference C2
docker/, deploy/  Compose labs and the Helm chart
hive.toml         Single configuration file loaded by every agent
```

## Adding an agent message type

1. Add the payload variant in `hive_base/src/ldc.rs` (`Payload` enum).
2. Extend the validator in `hive_base/src/ipc_contract.rs`.
3. Handle it in the relevant agents' `process_incoming`.
4. Add a roundtrip test (`ldc.rs` unit tests) and, if it affects the read
   path, a regression test under `hive_base/tests/`.

## Conventions

- No new `unwrap()`/`expect()` in non-test code of `hive_base` core modules
  (`comms`, `shared_arena`, `telemetry`, `ldc`) — propagate or log.
- Timestamps: always use `hive_base::utils::timestamp_now()`; never
  `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`.
- Locks held across `.await` must be `tokio::sync::Mutex`, never `std`.
- Every new shared-memory ABI field needs a `check-cfg`/test note in
  `shared_arena.rs` docs.
