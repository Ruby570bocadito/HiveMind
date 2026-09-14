# Operator Guide 🎮

```
╔═══════════════════════════════════════════════════════════════╗
║                                                               ║
║                     HIVE COLONY OPERATOR                      ║
║                                                               ║
║   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       ║
║   │ BUILD   │→│ DEPLOY  │→│ EXECUTE │→│ MONITOR │       ║
║   │ cargo   │  │ scripts │  │ target  │  │ C2 API  │       ║
║   └─────────┘  └─────────┘  └─────────┘  └─────────┘       ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
```

## Índice

| Sección | Descripción |
|---------|-------------|
| [1. Prerrequisitos](#1-prerrequisitos) | Lo que necesitas instalar |
| [2. Quick Start](#2-quick-start) | Primer despliegue en 3 pasos |
| [3. Scripts](#3-scripts) | Catálogo de herramientas |
| [4. Configuración](#4-configuración) | hive.toml y variables de entorno |
| [5. Monitoreo](#5-monitoreo) | C2 API, logs, health check |
| [6. Cross-compile Windows](#6-cross-compile-windows) | Build para targets Windows |
| [7. Kill Switch](#7-kill-switch) | Apagado de emergencia |
| [8. Solución de problemas](#8-solución-de-problemas) | Errores comunes |

---

## 1. Prerrequisitos

```
┌─────────────────────────────────────────────────────────────┐
│                    SISTEMA REQUERIDO                         │
├─────────────────────────────────────────────────────────────┤
│  Rust    → 1.70+    (rustup default stable)                 │
│  OpenSSL → dev      (apt install libssl-dev pkg-config)     │
│  Python  → 3.10+    (pip install -r requirements.txt)       │
│  Kernel  → 3.17+    (para shm_open / memfd_create)          │
│  Docker  → opcional (para despliegue containerizado)        │
│  Ollama  → opcional (para LLM estratégico)                  │
│  mingw   → opcional (para cross-compile Windows)            │
└─────────────────────────────────────────────────────────────┘
```

```bash
# Debian/Ubuntu
sudo apt update && sudo apt install -y \
    build-essential pkg-config libssl-dev \
    python3 python3-pip docker.io

# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# Windows cross-compile (opcional)
sudo apt install mingw-w64
rustup target add x86_64-pc-windows-gnu

# Python
pip install -r requirements.txt
```

---

## 2. Quick Start

```
PASO 1                          PASO 2                          PASO 3
╔═══════════════════╗    ╔═══════════════════════╗    ╔══════════════════════╗
║                   ║    ║                       ║    ║                      ║
║  ./hive.sh build  ║───▶║  ./hive.sh colony     ║───▶║  ./hive.sh tui       ║
║                   ║    ║                       ║    ║                      ║
║  todo el enjambre ║    ║  C2 + 6 agentes       ║    ║  operador conectado  ║
╚═══════════════════╝    ╚═══════════════════════╝    ╚══════════════════════╝
```

```bash
# Paso 1: Compilar todo
./hive.sh build

# Paso 2: Desplegar colonia en Docker (C2 + agentes)
./hive.sh colony

# Paso 3: Conectar el TUI del operador
./hive.sh tui

# Verificar
curl http://localhost:8444/health
```

> Ronda 4: las herramientas de armado de payloads (`deploy.sh`,
> `build_payload.sh`, `obfuscate_pe.py`) fueron retiradas. Las tácticas se
> ejecutan en modo emulación — ver `docs/CAPABILITIES.md`.

---

## 3. Scripts

```
scripts/
│
├── launch_colony.sh   ◀── Despliegue local Docker
│                        Lanza C2 + agentes + dashboard
│
├── lab_setup.sh       ◀── Preparación del laboratorio SSH
│                        Objetivos de práctica aislados
│
└── scenario.sh        ◀── Tests de escenarios
                         Validación de comportamiento (simulado)
```

---

## 4. Configuración

### hive.toml

```toml
[c2]
url = "https://tu-c2.com:8444/collect"
api_key = "supersecreto"

[agents]
edr_processes = [
    "csfalcon",       # CrowdStrike
    "csagent",        # CrowdStrike
    "msmpeng",        # Microsoft Defender
    "sentinelone",    # SentinelOne
    "carbonblack",    # VMware Carbon Black
    "cylancesvc",     # Cylance
    "symantec",       # Symantec
    "mcafee",         # McAfee
    "sesvc",          # Sophos
    "taniumclient",   # Tanium
    "elastic-endpoint", # Elastic
]

[exploits]
safe_mode = true
operator_approved = false
target_whitelist = ["10.0.0.0/8", "192.168.0.0/16"]

[consensus]
threshold = 0.66
```

### Variables de entorno

| Variable | Default | Propósito |
|----------|---------|-----------|
| `__HIVE_ARENA` | `/dev/shm/hive_arena` | Ruta del archivo de arena IPC |
| `HIVE_C2_URL` | `https://c2:8444/collect` | Endpoint HTTP C2 |
| `HIVE_C2_API_KEY` | — | Clave si el C2 exige `x-api-key` (TaskPoller) |
| `HIVE_POLL_SECS` | `10` | Intervalo del TaskPoller (mín. 2 s) |
| `HIVE_LAB_AUTHORIZED` | — | `1` habilita ejecución fileless en lab autorizado |
| `HIVE_LAB_MODE` | `0` | Modo laboratorio (1=simulado). Sin esta variable, todos los clientes TLS del enjambre verifican certificados estrictamente (ronda 5) |
| `HIVE_MASTER_KEY` | — | Clave de colmena (32B derivadas) para trails stigmergy y cifrado de fragmentos phoenix. En producción real, genera una única por despliegue (p. ej. `openssl rand -base64 32`); si falta, se usa una clave por defecto documentada (solo compatibilidad) |
| `HIVE_PERSISTENCE_DRY_RUN` | — | `1` = la remediación de persistencia (`honeycomb::uninstall_persistence`) no toca el host; solo registra lo que haría (ronda 5) |
| `HIVE_TELEMETRY_DIR` | `/tmp/hive_telemetry` | Directorio de telemetría |
| `HIVE_EXEC_TIMEOUT` | `30` | Timeout para comandos (s) |
| `RUST_LOG` | `info` | Nivel de logging |
| `HIVE_HIDE` | — | Modo oculto (sin stdout) |

---

## 5. Monitoreo

```
┌─────────────────────────────────────────────────────────────────┐
│                        MONITOREO                                │
│                                                                 │
│  ┌───────────┐    ┌────────────┐    ┌────────────────────┐     │
│  │ C2 API    │───▶│ Health     │───▶│ curl               │     │
│  │ :8444     │    │ Check      │    │ /health → {"ok"}   │     │
│  └───────────┘    └────────────┘    └────────────────────┘     │
│                                                                 │
│  ┌───────────┐    ┌────────────┐    ┌────────────────────┐     │
│  │ Logs      │───▶│ File       │───▶│ tail -f            │     │
│  │ ocultos   │    │ /tmp/      │    │ /tmp/hive_queen.log│     │
│  └───────────┘    │ hive_*.log │    └────────────────────┘     │
│                   └────────────┘                                │
│  ┌───────────┐    ┌────────────┐    ┌────────────────────┐     │
│  │ Dashboard │───▶│ Web UI     │───▶│ http://localhost:   │     │
│  │ (Docker)  │    │ :8080      │    │ 8080               │     │
│  └───────────┘    └────────────┘    └────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

### Health check

```bash
# C2 server vivo?
curl http://localhost:8444/health
# → {"status":"ok","agents":6,"uptime":12345}
```

### Logs de agente

```bash
# Modo normal: stdout
# Modo oculto (--hide / --silent): archivo
tail -f /tmp/hive_queen.log
tail -f /tmp/hive_worker.log
tail -f /tmp/hive_honeybee.log
```

### Modo oculto

Los agentes soportan `--hide` o `--silent` para suprimir toda salida a terminal:

```bash
# Sin output visible
./target/release/queen --hide

# Los logs van a /tmp/hive_queen.log
# stdout/stderr redirigidos a /dev/null via dup2()
```

### Ver procesos

```bash
ps aux | grep -E 'queen|worker|drone|honeybee|swarm'

# o
pgrep -a queen
pgrep -a worker
```

---

## 6. Cross-compile Windows

```
┌─────────────────────────────────────────────────────────────────┐
│          CROSS-COMPILE: LINUX → WINDOWS x86_64                  │
│                                                                 │
│  cargo build \                                                  │
│    --release \                                                  │
│    --target x86_64-pc-windows-gnu \                             │
│    -p queen                                                     │
│                                                                 │
│  target/x86_64-pc-windows-gnu/release/queen.exe  ◀── .exe listo │
└─────────────────────────────────────────────────────────────────┘
```

```bash
# 1. Instalar toolchain (una vez)
./setup_cross.sh win
#   → apt install mingw-w64
#   → rustup target add x86_64-pc-windows-gnu

# 2. Compilar queen
cargo build --release --target x86_64-pc-windows-gnu -p queen

# 3. Compilar todos los agentes
for p in queen worker drone honeybee swarm c2-server; do
    cargo build --release --target x86_64-pc-windows-gnu -p "$p"
done

# 4. Artefactos: la entrega de payloads armados fue retirada (ronda 4).
#    Despliega los binarios del build directo en tu laboratorio o usa
#    docker compose (ver DEPLOYMENT.md).
```

### Módulos Windows disponibles

| Módulo | Archivo | Capacidad |
|--------|---------|-----------|
| Syscalls | `syscalls.rs` | Hell's Gate + Halo's Gate + Hades Gate |
| Stack spoof | `stack_spoof.rs` | Ret-spoofing + RBP chain sintética |
| Fileless | `fileless.rs` | NtCreateSection + NtMapViewOfSection |
| Credentials | `leech.rs` | LSASS (syscalls), SAM, DPAPI |
| Anti-analysis | `anti_analysis.rs` | PEB BeingDebugged, sandbox detection |
| EDR detection | `system_info.rs` | 30+ firmas (Defender, CrowdStrike...) |
| Persistence | `phoenix.rs` | Registry Run, Startup, SchTasks, WMI |

---

## 7. Kill Switch

```
┌─────────────────────────────────────────────────────────────────┐
│                    EMERGENCY KILL SWITCH                        │
│                                                                 │
│  POST /beacon {"action":"kill_switch"}                          │
│         │                                                       │
│         ▼                                                       │
│  ┌──────────────────┐                                          │
│  │ C2 Server        │                                          │
│  │ └── broadcast    │────────────────▶ Todos los agentes       │
│  │     kill_switch  │                  se autodestruyen        │
│  └──────────────────┘                  en ≤5 segundos          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

```bash
# Vía API
curl -X POST http://localhost:8444/beacon \
  -H "Content-Type: application/json" \
  -d '{"action":"kill_switch"}'

# Vía dashboard (si está corriendo)
# Botón "Kill Switch" en http://localhost:8080

# O manualmente
pkill -9 queen worker drone honeybee swarm c2-server
rm -rf /tmp/.hive /tmp/.h /dev/shm/hive_arena
```

---

## 8. Solución de problemas

| Problema | Causa probable | Solución |
|----------|---------------|----------|
| `cargo build` falla | Falta OpenSSL | `apt install libssl-dev pkg-config` |
| `cargo build --target windows` falla | Falta mingw | `./setup_cross.sh win` |
| Agente no conecta al C2 | Firewall | Verificar puerto 8444 accesible |
| TaskPoller inactivo | Falta `HIVE_C2_URL` | Exportar `HIVE_C2_URL=http://127.0.0.1:8444` |
| Tarea sin respuesta | Falta API key | Exportar `HIVE_C2_API_KEY` si el C2 exige `x-api-key` |
| Fileless bloqueado | Política de lab | `HIVE_LAB_AUTHORIZED=1` en laboratorio autorizado |
| Logs de agente vacíos | Modo oculto | `tail -f /tmp/hive_<agent>.log` |
| Queen no lanza agentes | Arena no disponible | `ls -la /dev/shm/hive_arena` |
| Docker no arranca | Puerto ocupado | `netstat -tlnp \| grep 8444` |

### Debug mode

```bash
# Compilar con debug symbols
cargo build -p queen

# Ejecutar con RUST_LOG=trace para máximo detalle
RUST_LOG=trace ./target/debug/queen

# Sin modo oculto (ver todo en terminal)
./target/debug/queen
```

---

## Referencias rápidas

### Lo que más vas a usar

```bash
# Compilar
cargo build --release --workspace

# Despliegue local de la colonia
./scripts/launch_colony.sh

# Ciclo de tareas del operador (ronda 4):
curl -X POST http://localhost:8444/task/<agent_id> \
  -H 'x-api-key: ...' -H 'Content-Type: application/json' \
  -d '{"id":"t1","command":"shell","payload":"uptime"}'
# El agente lo recoge por TaskPoller y responde vía beacon al shell del C2
```

### Documentación relacionada

| Documento | Contenido |
|-----------|----------|
| [DEPLOYMENT.md](DEPLOYMENT.md) | Build y despliegue en laboratorio |
| [AGENTS.md](AGENTS.md) | Referencia de cada agente |
| [PLAYBOOK.md](PLAYBOOK.md) | Playbook operativo completo |
| [CAPABILITIES.md](CAPABILITIES.md) | Matriz de capacidades (ronda 6): real vs eliminado |
