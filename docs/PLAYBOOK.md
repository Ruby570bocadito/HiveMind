# Hive Colony v3.0 — Playbook de Operador

> Ronda 11: reescrito de cero. La versión anterior era un fósil de la era
> ofensiva (playbooks de sabotaje de datos, evasión EDR y persistencia —
> capacidades ELIMINADAS en la ronda 6), con puertos, tests y servicios que
> ya no existen. Este playbook describe SOLO lo que el build actual hace.

## Arquitectura

```
┌─────────────────────────────────────────────────────┐
│                 BEEKEEPER (CLI + TUI)                │
│  status | inject | kill-switch | tui | config-check │
├─────────────────────────────────────────────────────┤
│              SHARED MEMORY ARENA (shm)               │
│  ┌────────┐  ┌───────┐  ┌──────────┐  ┌───────────┐ │
│  │ Worker │  │ Drone │  │ Honeybee │  │   Queen   │ │
│  └────────┘  └───────┘  └──────────┘  └───────────┘ │
│  perfil del sistema (solo lectura) | propuestas RL   │
│  votos de consenso | Tournament | HiveMind           │
│  WhisperNet (mesh interna) | Stigmergy | Phoenix     │
├─────────────────────────────────────────────────────┤
│              C2 SERVER (Rust, :8444)                 │
│  /beacon  /task/:id  /shell/:id (WS)  /health  /logs │
│  DASHBOARD WEB (:8080, tests/dashboard.py)           │
└─────────────────────────────────────────────────────┘
```

## Instalación

```bash
git clone https://github.com/Ruby570bocadito/HiveMind
cd HiveMind
./hive.sh build            # workspace completo (debug)
./hive.sh test             # suite completa del workspace
```

## Uso Rápido — Laboratorio Local

```bash
# 1. C2 server (Rust)
./hive.sh c2 &

# 2. Arena compartida
export __HIVE_ARENA=hive_lab

# 3. C2 base para el TaskPoller de los agentes (BASE, sin /beacon)
export HIVE_C2_URL=http://127.0.0.1:8444

# 4. Lanzar agentes (los 4 binarios del build)
./target/release/worker &
./target/release/drone &
./target/release/honeybee &
./target/release/queen &

# 5. Monitorear — TUI del operador
./target/release/beekeeper tui

# 6. Alternativa en Docker: C2 + agentes + dashboard
./hive.sh colony
# o bare metal sin Docker:
./scripts/launch_colony.sh --release
```

## Módulos por Agente (reales, post-ronda 6)

| Agente | Sistemas activos | Función |
|--------|------------------|---------|
| **Worker** | perfil del sistema, ML scout, TaskPoller | Recoge SOLO lectura (OS, CPU, EDR/backup presentes), publica beliefs, clasifica con RandomForest embebido, vota propuestas |
| **Drone** | propuestas operativas, Phoenix (memoria), regeneración del worker | Propone acciones, vota, re-lanza el worker si muere |
| **Honeybee** | WhisperNet relay, votos, TaskPoller | Relay P2P interno, participa en consenso |
| **Queen** | HiveMind (tally), Tournament, WhisperNet | Procesa votos con reputaciones reales, aprueba directivas por consenso, torneos darwinianos |

## Playbook 1: Ver el consenso de punta a punta (ronda 11)

```bash
# 1. Arranca la colonia (ver "Uso Rápido") y abre el TUI
./target/release/beekeeper tui

# 2. Pestaña 3 (Consensus): el drone propone ("prop_to_network_segment"),
#    worker/drone/honeybee votan con la política de la colonia
#    (deny-list ronda 6: exfil/encrypt/wipe/destroy/sabotage/ransom → Reject),
#    la Queen tally con reputaciones reales (umbral 0.66) y broadcast:
#    StatusEvent "hive_directive_approved" + belief "directive:<id>".

# 3. Comprueba el flujo sin TUI:
./target/release/beekeeper hive-mind
```

## Playbook 2: Verificación honesta del build

```bash
./target/release/beekeeper validate
# Salida esperada (checks REALES, sin anti-debug/anti-sandbox — eliminados):
#   ✓ TCP ports     — ningún puerto TCP del enjambre escuchando
#   ✓ ONNX sigs     — modelo cifrado (XOR), sin ONNX legible en el binario
#   ✓ Bus addr      — sin IPs hardcodeadas en el tráfico
#   ✓ Memfd         — memfd_create disponible (gate: HIVE_LAB_AUTHORIZED=1)
#   ✓ Agent names   — nombres antiguos no presentes en el binario

# Config:
./target/release/beekeeper config-check   # exit 1 si hive.toml no parsea
./hive.sh doctor                          # entorno + config + C2 health
```

## Playbook 3: Tasking del operador (C2 + shell auditado)

```bash
# Tarea individual (el agente la recoge por GET /task/:id; los comandos
# destructivos/exfil los RECHAZA el TaskPoller — deny-list ronda 6):
curl -s -X POST http://127.0.0.1:8444/task/<agent_id> \
  -H 'Content-Type: application/json' \
  -d '{"id":"t-1","command":"shell_exec","payload":{"cmd":"uptime"}}'

# Shell interactivo por WebSocket (misma política):
#   ws://127.0.0.1:8444/shell/<session_id>  →  {"agent_id": "..."}

# Ajuste fino del ciclo:
export HIVE_POLL_SECS=5       # intervalo de sondeo (mín. 2)
export HIVE_C2_API_KEY=...    # si el C2 arrancó con --api-key
```

## Playbook 4: Apagado limpio (kill switch)

```bash
# CLI:
./target/release/beekeeper kill-switch --confirm

# O desde el TUI: pulsa K dos veces (arma y confirma; la status bar se
# pone en rojo). Los agentes salen al leer el evento kill_switch.
```

## Configuración (`hive.toml`)

El fichero enviado parsea desde la ronda 11 (antes: `[eartbeat]` roto —
los agentes corrían con defaults en silencio). `beekeeper config-check`
sale 1 si el fichero no parsea:

```toml
[arena]
max_agents = 16

[heartbeat]
interval_secs = 10
timeout_secs = 30

[consensus]
threshold = 0.66        # umbral de aprobación de directivas
hoarder_threshold = 0.80

[timing]
scan_interval_secs = 15
decision_interval_secs = 30
```

## Resolución de Problemas

| Síntoma | Causa | Solución |
|---------|-------|----------|
| `HiveChamber::connect` falla | Arena no existe | Exportar `__HIVE_ARENA` idéntico en todos los procesos |
| Agentes no se ven entre sí | IPC namespace | Usar `ipc: host` en Docker o `--ipc=host` |
| El poller no recoge tareas | `HIVE_C2_URL` con `/beacon` | Usar la BASE (`http://127.0.0.1:8444`); el poller añade las rutas (acepta ambas formas desde ronda 11) |
| Tournament no avanza | Pocos competidores | Queen necesita al menos 2 generaciones |
| Windows build falla | Faltan librerías | `sudo apt-get install mingw-w64` |
| WhisperNet no enruta | Sin peers | Los peers se registran automáticamente vía arena |

## Comandos Rápidos

```bash
beekeeper status --watch        # dashboard terminal en vivo
beekeeper inject -a target_ip -v 10.0.0.5 -c 0.95  # inyectar creencia
beekeeper validate              # verificación honesta del build
beekeeper config-check          # valida hive.toml (exit 1 = roto)
beekeeper kill-switch --confirm # apagar colonia
beekeeper tournament            # ver torneos
beekeeper hive-mind             # flujo de consenso real
```

## Docker Compose

```bash
# Stack completo
./hive.sh colony     # = docker compose up --build -d

# Servicios:
#   c2-server   :8444 — C2 endpoint (Rust)
#   queen       :—    — tally de consenso + torneos
#   worker      :—    — perfil del sistema (solo lectura)
#   drone       :—    — propuestas + regeneración
#   honeybee    :—    — relay P2P + votos
#   monitor     :—    — monitor de detecciones (tests/)
#   dashboard   :8080 — Web UI (tests/dashboard.py)
#   ollama      :—    — opcional (perfil "llm")

# Ver resultados:
docker compose logs monitor
open http://localhost:8080
```

---
*Hive Colony v3.0 — framework multi-agente para labs aislados (cero payloads
de ataque desde la ronda 6), suite completa en CI, clippy limpio*
