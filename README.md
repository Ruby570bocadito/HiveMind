# Hive Colony

```
                          .' '.
        _/__)           .       .
       (8|)_}}- .      .        .
        `\__)    '. . ' ' .  . '
    H I V E   C O L O N Y
```

**Rust-based post-exploitation framework.**
Bee-inspired swarm architecture con agentes especializados, evasión por capas y C2 diverso.

## Quick Start

```bash
# 1. Compilar todo
cargo build --release --workspace

# 2. Generar payloads
./scripts/deploy.sh all

# 3. Despliegue local
./scripts/launch_colony.sh

# 4. Payload monolítico (auto-extraíble)
./scripts/build_payload.sh
```

## Vectores de despliegue

| Vector | Comando | Output |
|--------|---------|--------|
| Network | `./scripts/deploy.sh network` | `payloads/network/` — stager bash + payload.b64 + oneliner |
| USB | `./scripts/deploy.sh usb` | `payloads/usb/` — `.install.sh` + `manifest.dat` + `.ps1` |
| Phishing | `./scripts/deploy.sh phishing` | `payloads/phishing/` — HTML smuggling + VBA macro |
| EXE | `./scripts/deploy.sh exe --windows` | `payloads/executable/` — C# loader + queen cifrado |

Opciones: `--obfuscate` (PE obfuscation), `--windows`, `--c2-host HOST`, `--c2-port PORT`

## 6 Agent Types

| Agent | Rol | Capacidad |
|-------|-----|-----------|
| **Queen** | Overmind | Estrategia LLM (Ollama), bridge C2, HiveMind consensus, Seer predictivo |
| **Worker** | Scout | Reconocimiento, EDR detection (30+ firmas), ONNX classifier, Leech harvesting |
| **Drone** | Shaper | Movimiento lateral SSH, descubrimiento de red, regeneración fileless |
| **Honeybee** | Hoarder | Ejecución final: cifrado AES-256-GCM, wipe 3-pasada, exfiltración C2, privesc, cloud pivot |
| **Weaver** | Morph | Ofuscación polimórfica, 4 técnicas de mutación |
| **Swarm** | Worm | Auto-propagación SSH, selección de targets via MARL |

## Comunicación

### Interna (Arena)
```
Worker ──┐
Drone  ──┤── Arena compartida (shm_open / mmap) ── sin red, sin puertos
Honeybee─┤   16 slots lock-free, MessagePack, 8KB mensajes
Weaver ──┘
```

### Externa (C2)
```
Queen ─── HTTP(S) ─── C2 Server
       ─── DNS Tunnel ─── (txt records)
       ─── ICMP Tunnel ─── (raw socket)
       ─── Dead Drop ─── Gist / Pastebin / S3
Failover: Priority → Race → RoundRobin con backoff exponencial
```

## Evasión por Capas

| Capa | Técnica | Estado |
|------|---------|--------|
| 1 | IPC por memoria compartida (sin TCP) | ✅ |
| 2 | Fileless `memfd_create` / NtCreateSection | ✅ Linux + Windows |
| 3 | ASM syscalls directas (Hell's Gate / Halo's Gate) | ✅ Linux + Windows |
| 4 | Stack spoofing (ret-spoofing / RBP chain) | ✅ Linux + Windows |
| 5 | Anti-debug (PEB BeingDebugged, ptrace) | ✅ Linux + Windows |
| 6 | Anti-sandbox (USER/CPU/tiempo de actividad) | ✅ Linux + Windows |
| 7 | EDR detection (30+ firmas: Defender, CrowdStrike, SentinelOne...) | ✅ Windows |
| 8 | String obfuscation | ✅ |
| 9 | OPSEC: Jitter, DecoyProfile, ActivitySchedule, TrafficMimic | ✅ |
| 10 | Hibernación adaptativa + channel rotation | ✅ |

## Windows Support

```bash
./setup_cross.sh win          # Toolchain
cargo build --release --target x86_64-pc-windows-gnu -p queen
./scripts/deploy.sh exe --windows --obfuscate
```

| Módulo | Capacidad |
|--------|-----------|
| `syscalls` | Hell's Gate + Halo's Gate + Hades Gate + indirect syscall |
| `hades_gate` | Resolución dinámica de SSN desde ntdll.dll en memoria |
| `stack_spoof` | Ret-spoofing con stack swap + RBP chain sintética |
| `fileless` | NtCreateSection + NtMapViewOfSection |
| `leech` | LSASS (syscalls directas), SAM, DPAPI |
| `anti_analysis` | PEB BeingDebugged, sandbox por USER/CPU |
| `phoenix` | Persistencia: Registry Run, Startup, SchTasks, WMI |

## MITRE ATT&CK

36+ técnicas en 10+ tácticas. [Ver mapeo completo](docs/MITRE_MAPPING.md).

## Scripts

```
scripts/
├── deploy.sh            # Generador de payloads (4 vectores)
├── build_payload.sh     # Stager monolítico auto-extraíble
├── launch_colony.sh     # Despliegue local (Docker)
├── obfuscate_pe.py      # PE obfuscator v2.2
└── scenario.sh          # Tests de escenarios
```

## Docker Compose

```bash
docker compose up -d
docker compose logs -f
docker compose down
```

## Requirements

- **Rust** 1.70+
- **OpenSSL** dev (`apt install libssl-dev pkg-config`)
- **Python** 3.10+ (ver `requirements.txt`)
- **Linux** 3.17+ (kernel con `shm_open`)
- **Docker** (opcional)
- **Opcional:** Ollama, nmap, mingw-w64

## Research Use

Este proyecto es exclusivamente para **investigación y educación** en ciberseguridad defensiva. No usar en sistemas sin autorización explícita por escrito.
