# Hive Colony — Documentation Index

Bee-inspired multi-agent autonomous framework. Agents communicate through a
shared-memory arena (zero TCP ports between agents), spread via SSH in lab
environments, and run under an LLM strategic director.

> **Scope notice:** this project is a distributed-systems engineering
> portfolio piece built around a security narrative. Run it only against
> lab machines you own. The shipped `hive.toml` defaults to `safe_mode = true`
> with the exploits module disabled.

## Reading order

| Doc | Contents | Status |
|-----|----------|--------|
| [`../README.md`](../README.md) | Project overview, architecture, quick start | current |
| [`AGENTS.md`](AGENTS.md) | Per-agent reference: roles, message payloads, env vars | current |
| [`NAMING.md`](NAMING.md) | Map between old docs names (Scout/Shaper/...) and code names (Worker/Drone/...) | current |
| [`OPERATOR_GUIDE.md`](OPERATOR_GUIDE.md) | Environment variables, TUI, subcommands | current |
| [`API.md`](API.md) | C2 HTTP API reference (Rust server) | current |
| [`DEVELOPMENT.md`](DEVELOPMENT.md) | Build, test, project layout, contribution notes | current |
| [`DEPLOYMENT.md`](DEPLOYMENT.md) | Lab deployment walkthrough (authorized labs only) | current |
| [`EVASION.md`](EVASION.md) | Overview of the defense-evasion modules from a blue-team perspective | current |
| [`MITRE_MAPPING.md`](MITRE_MAPPING.md) | Techniques implemented per module | current |
| [`PLAYBOOK.md`](PLAYBOOK.md) | Demo scenario walkthrough | current |

## Quick start (verified)

```bash
# Build everything (11 crates)
./hive.sh build

# Run the test suite (360+ tests)
./hive.sh test

# Start the C2 server (http://localhost:8444)
./hive.sh c2

# Or bring up the whole stack in Docker
./hive.sh colony
```

The old `build_env.sh` / `colmena.sh` helper scripts are gone; `hive.sh`
and plain `cargo` are the only entry points you need.
