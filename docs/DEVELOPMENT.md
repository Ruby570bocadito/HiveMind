# Development Guide

## Building

```bash
source build_env.sh
cargo build --workspace       # Debug
cargo build --release --workspace  # Release
```

## Testing

```bash
./colmena.sh test
# 35 tests: 21 unit + 4 sandbox + 10 integration
```

### Test Categories

| Suite | Tests | Description |
|-------|-------|-------------|
| `agent_base` (lib) | 21 | Consensus, crypto, attack mapping, config, honey, obfuscation |
| `edr_sandbox` | 4 | TCP ports, bus address, ONNX signatures, crypto roundtrip |
| `integration_test` | 10 | Arena, consensus threshold, reputation weight, config, exfil, memfd, ATT&CK |

## Project Structure

```
agent_base/src/
├── comms.rs          # SwarmChamber (shared memory API)
├── ldc.rs            # LdC protocol types
├── consensus.rs      # Reputation-weighted voting
├── identity.rs       # Ed25519 signing
├── shared_arena.rs   # Lock-free ring buffer
├── arena_mgr.rs      # memfd_create / shm_open
├── crypto.rs         # XOR + ChaCha20
├── fileless.rs       # MemfdBinary execution
├── syscalls.rs       # Direct syscalls (Linux + Win Hell's Gate)
├── anti_analysis.rs  # Debug/VM/sandbox detection
├── lateral.rs        # SSH movement, cred harvesting
├── exfil.rs          # DNS/HTTP C2 exfiltration
├── attack.rs         # MITRE ATT&CK mapping
├── stack_spoof.rs    # Call stack spoofing
├── c2_bridge.rs      # Sliver/CS/HTTP protocol translation
├── exploits.rs       # EternalBlue, BlueKeep, Log4Shell
├── config.rs         # TOML configuration
├── obfstr.rs         # Compile-time string obfuscation
├── brain.rs          # Safe target protection
├── honey.rs          # Honeypot/honeyfile detection
├── ml.rs             # ONNX Runtime wrapper
├── utils.rs          # Logging, timestamps, delays
└── lib.rs            # Module registry
```

## Adding a New Agent

1. Create `agents/<name>/Cargo.toml` with `agent_base` dependency
2. Create `agents/<name>/src/main.rs` following the pattern:
   ```rust
   let identity = AgentIdentity::new();
   let comms = SwarmChamber::connect(&identity, Role::Xxx).await?;
   // Agent loop with heartbeat + task timers
   ```
3. Add to workspace: `Cargo.toml` members
4. Add to `swarm_run/src/main.rs` spawn list
5. Add to `ldc.rs` Role enum if new role type

## Adding a New Module to agent_base

1. Create `agent_base/src/<module>.rs`
2. Add `pub mod <module>;` to `agent_base/src/lib.rs`
3. Add any new dependencies to `agent_base/Cargo.toml`
4. Re-export public API in `lib.rs` if needed

## Agent Pattern

Every agent follows this structure:

```rust
struct MyAgent {
    comms: SwarmChamber,
    identity: AgentIdentity,
    consensus: ConsensusEngine,
    heartbeat_interval: Duration,
    task_interval: Duration,
}

impl MyAgent {
    async fn run(&mut self) {
        self.send_heartbeat().await;
        let mut heartbeat = interval(self.heartbeat_interval);
        let mut task = interval(self.task_interval);
        loop {
            select! {
                _ = heartbeat.tick() => self.send_heartbeat().await,
                _ = task.tick() => self.do_work().await,
                _ = sleep(Duration::from_millis(200)) => self.process_incoming().await,
            }
        }
    }
}
```

## Logging

Agents use `tracing` with environment variable filtering:

```bash
RUST_LOG=scout=debug,shaper=info cargo run -p scout
```

## Safety Rules

- Never commit compiled binaries
- Never hardcode real credentials or C2 URLs
- Always test with `safe_mode: true` in exploits
- Always configure `[brain] safe_ips` before colony mode
- Run `cargo test --workspace` before committing
