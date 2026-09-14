# C2 Server API

The Rust C2 server (`c2/`, package `c2-server`) listens on **http://localhost:8444**
by default (override with `--port`). All endpoints return JSON unless stated
otherwise. There is no TLS in front of it by default — terminate TLS with a
reverse proxy if you expose it beyond localhost.

> Note: `tests/c2_server.py` is a standalone Python *lab* C2 used by test
> scripts. Its extra endpoint (`/jndi`) is not part of this API.

## Endpoints

### GET /health

Liveness + counters.

```json
{
  "status": "ok",
  "exfil_count": 12,
  "beacon_count": 340,
  "sessions": 1
}
```

### GET /

Operator dashboard (HTML).

### GET /logs

Last 50 activity entries (beacons + exfils), newest first.

### POST /collect — exfil ingest

Headers:

| Header | Meaning |
|--------|---------|
| `X-Agent-ID` | agent UUID |
| `X-Agent-Role` | `worker`, `drone`, `honeybee`, `queen`, `swarm` |
| `Content-Type` | `application/octet-stream` |

Query: `?filename=report.log` (optional, defaults to `data.bin`).

Body: raw file bytes.

Response:

```json
{"status": "received", "sha256": "abc123…", "size": 12345}
```

### POST /beacon — agent heartbeat/status

Headers: `X-Agent-ID`, `X-Agent-Role`.

Body: JSON beacon (unknown fields are preserved under `extra`):

```json
{"status": "alive", "edr_detected": false}
```

**Shell results:** a beacon whose `extra` contains `session` and `output`
is streamed to the attached operator WebSocket (see `/shell/:session_id`).

Response:

```json
{"status": "ack", "beacon_count": 340}
```

### GET /task/:agent_id — task pickup

Returns up to 10 pending tasks for the agent and marks them claimed.

```json
{"tasks": [{"id": "sh-…-1", "command": "shell_exec", "payload": {"cmd": "id", "session": "…"}}]}
```

### POST /task/:agent_id — push a task

Body: `{"id": "t1", "command": "scan", "payload": {}}` → `201 CREATED`.

### GET /shell/:session_id — operator shell (WebSocket)

Upgrade to WebSocket. Protocol:

1. First text frame: `{"agent_id": "…"}`.
2. Each following text frame is a shell command for that agent. The server
   queues it via the task system and acknowledges (`[queued …]`).
3. Agent replies come back as beacon frames (`session` + `output`) and are
   streamed to the attached session. Pings every 30 s.

The relay map is shared at application level, so multiple operator sessions
can attach to different agents concurrently.

### GET /admin/sessions

List of shell sessions (id, agent, timestamps).

### GET /admin/agents

Summary of agents seen in beacons.

## Dashboard (`tests/dashboard.py`, port 8080)

Read-only process/health view served with Python's stdlib `http.server`
(it is **not** a Flask app):

- `GET /` — HTML dashboard
- `GET /api/state` — JSON snapshot of observed processes

## Internal agent protocol (context)

Agents exchange LdC messages over the shared-memory arena, not over HTTP.
The HTTP API above is the operator/ingest boundary. Message types are
defined in `hive_base/src/ldc.rs` (`Payload` enum) and documented in
[AGENTS.md](AGENTS.md).
