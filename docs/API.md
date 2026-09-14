# C2 Server API

The Rust C2 server (`c2/`, package `c2-server`) listens on **http://localhost:8444**
by default (override with `--port`). All endpoints return JSON unless stated
otherwise. There is no TLS in front of it by default — terminate TLS with a
reverse proxy if you expose it beyond localhost.

> Note: `tests/c2_server.py` is a standalone Python *lab* C2 used by test
> scripts. It exposes `/beacon`, `/health` and `/logs` only.

> Ronda 6: the exfil ingest endpoint (`POST /collect`) was **removed** —
> the C2 receives beacons, tasks and shell traffic only.

## Endpoints

### GET /health

Liveness + counters.

```json
{
  "status": "ok",
  "beacon_count": 340,
  "sessions": 1
}
```

### GET /

Operator dashboard (HTML).

### GET /logs

Last 50 activity entries (beacons), newest first.

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

**Task lifecycle (ronda 4):** agents run a TaskPoller (`hive_base::task_poller`)
that picks up queued tasks and posts results to `/beacon`. For tasks queued by
the operator shell (`command: "shell_exec"`, payload `{"cmd", "session"}`), the
result beacon carries `session` + `output` and is streamed to the operator's
WebSocket. Task command policy (ronda 6): `shell_exec` executes with audit;
`exfil`, `encrypt`, `wipe`, `destroy` and `sabotage` return `rejected`
(the capability does not exist in the build); anything else returns
`unsupported`.

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

## Server flags (hardening)

| Flag | Default | Effect |
|------|---------|--------|
| `--port` | `8444` | Listen port. |
| `--api-key <secret>` | *(empty = auth off)* | Requires the `x-api-key` header on agent/operator endpoints (`/logs`, `/beacon`, `/task/:id`, `/shell/:id`, `/admin/*`). `/` and `/health` stay public for monitoring. Comparison is constant-time (SHA-256 + fold). A missing/wrong key answers `401 UNAUTHORIZED`. |
| `--rate-limit <req/min>` | `120` | Per-client-IP fixed-window rate limit across all endpoints. Exceeding the quota answers `429 TOO_MANY_REQUESTS` without running the auth check. `0` disables it. Buckets are in-memory and evicted lazily. |
| `--cors-origin <url>` | *(none)* | Repeatable allow-list entry for browser origins (e.g. `--cors-origin https://ops.example.com`). With no entries the server sends **no CORS headers**, so browsers cannot read the API cross-origin. Allowed methods: GET/POST/OPTIONS; allowed headers: `content-type`, `x-api-key`. |
| `--cors-anywhere` | off | Opt-in escape hatch that restores the old `Access-Control-Allow-Origin: *`. Only for throwaway labs. |

Layer order on protected routes (outermost first): CORS → rate limit → auth,
so flooding is rejected before paying the auth comparison and preflight
requests are answered by the CORS layer.

## Internal agent protocol (context)

Agents exchange LdC messages over the shared-memory arena, not over HTTP.
The HTTP API above is the operator/ingest boundary. Message types are
defined in `hive_base/src/ldc.rs` (`Payload` enum) and documented in
[AGENTS.md](AGENTS.md).
