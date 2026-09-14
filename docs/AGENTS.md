# Agent Reference 🐝

```
╔══════════════════════════════════════════════════════════════════╗
║                    HIVE COLONY — AGENTES                        ║
║                                                                 ║
║  ┌─────────┐                                                    ║
║  │  QUEEN  │ ◀── Overmind: estrategia LLM + bridge C2          ║
║  │  (1)    │                                                    ║
║  └────┬────┘                                                    ║
║       │                                                         ║
║       ▼                                                         ║
║  ┌─────────┐  ┌─────────┐  ┌─────────┐                        ║
║  │ WORKER  │  │  DRONE  │  │HONEYBEE │                        ║
║  │ Scout   │  │ Shaper  │  │ Hoarder │                        ║
║  └─────────┘  └─────────┘  └─────────┘                        ║
║       │                                                         ║
║       ▼                                                         ║
║  ┌─────────┐                                                    ║
║  │  SWARM  │  Worm auto-propagante                             ║
║  └─────────┘                                                    ║
║                                                                 ║
║  Todos se comunican vía ARENA (memoria compartida, sin TCP)    ║
╚══════════════════════════════════════════════════════════════════╝
```

## Índice

| Agente | Símbolo | Rol | Archivo |
|--------|---------|-----|---------|
| [Queen](#queen--overmind) | ◇ | Overmind — estrategia LLM + C2 bridge | `agents/queen/` |
| [Worker](#worker--scout) | ◈ | Scout — reconocimiento + EDR detection | `agents/worker/` |
| [Drone](#drone--shaper) | ◆ | Shaper — decisiones + movimiento lateral | `agents/drone/` |
| [Honeybee](#honeybee--hoarder) | ◉ | Hoarder — ejecución final **solo simulación** (destructivo deshabilitado) | `agents/honeybee/` |
| [Swarm](#swarm--worm) | ⬡ | Worm — auto-propagación autónoma | `agents/swarm/` |

---

## Queen ◇ — Overmind

```
┌─────────────────────────────────────────────────────────────────┐
│  QUEEN                                                          │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ Ollama LLM   │───▶│ HiveMind     │───▶│ C2 Bridge    │     │
│  │ (estratégico)│    │ consensus    │    │ HTTP / DNS   │     │
│  └──────────────┘    └──────────────┘    │ ICMP / Dead  │     │
│                                          └──────────────┘     │
│                                                                 │
│  Receptor de órdenes del operador vía C2                        │
│  Traductor entre LdC (Lenguaje de la Colmena) y C2 externo     │
│  Orquestador: decide QUÉ hacer basado en creencias de Worker   │
└─────────────────────────────────────────────────────────────────┘
```

**Archivo:** `agents/queen/src/main.rs`
**Rol:** Overmind — estrategia LLM, bridge C2, HiveMind consensus

### Capacidades

| Capacidad | Detalle |
|-----------|---------|
| LLM estratégico | Consulta Ollama para decisiones tácticas |
| C2 Bridge | Traduce LdC → HTTP/DNS/ICMP/Dead Drop |
| HiveMind Consensus | Coordina votación entre agentes |
| Seer predictivo | Predice eventos basado en telemetría |
| Failover | Cambia de canal C2 si uno falla |

### Comunicación

```
┌──────────┐     LdC (Arena)     ┌──────────┐
│  Worker   │◀──────────────────▶│  Queen   │
│  Drone    │                    │          │
│  Honeybee │                    │  C2 🡕   │
│  Swarm    │                    │  HTTP    │
└──────────┘                    │  ICMP    │
                                  │  Dead    │
                                  └──────────┘
```

### C2 Bridge Commands (HTTP)

| Comando | Traducción LdC | Efecto |
|---------|----------------|--------|
| `scan` | `Request("scan")` | Worker escanea |
| `exfiltrate` | `Desire("exfiltrate", 0.9)` | Honeybee **simula** la exfiltración (egress deshabilitado) |
| `encrypt` | `Desire("encrypt", 0.8)` | Honeybee **simula** el cifrado (destructivo deshabilitado) |
| `kill` | `StatusEvent("kill_switch")` | Todos se destruyen |
| `inject_belief` | `Belief(asset, value, 1.0)` | Inyecta creencia |

### MITRE ATT&CK

| Técnica | ID | Descripción |
|---------|----|-------------|
| Encrypted Channel | T1573.002 | Cifrado AES-GCM en comunicaciones C2 |
| Proxy: CDN Fronting | T1090.004 | Dead Drop vía servicios legítimos |

---

## Worker ◈ — Scout

```
┌─────────────────────────────────────────────────────────────────┐
│  WORKER                                                         │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ System       │───▶│ EDR          │───▶│ Publica      │     │
│  │ Profiling    │    │ Detection    │    │ creencias    │     │
│  └──────────────┘    └──────────────┘    │ a la Arena   │     │
│                                          └──────────────┘     │
│  Lee: /proc, /sys, cgroups, hostname, user                     │
│  Detecta: CrowdStrike, Defender, SentinelOne, CarbonBlack...   │
│  Clasifica: Random Forest embebido (o heurísticas)             │
└─────────────────────────────────────────────────────────────────┘
```

**Archivo:** `agents/worker/src/main.rs`
**Rol:** Perception — reconocimiento y detección

### Capacidades

| Capacidad | Detalle |
|-----------|---------|
| System profiling | OS, arquitectura, hostname, usuario, procesos |
| EDR detection | 8 firmas en Linux / 34 en Windows (CrowdStrike, Defender, SentinelOne, etc.) |
| Backup detection | Veeam, Backup Exec, CommVault, NetBackup |
| Network enum | Interfaces, IPs, MACs, gateway |
| ML classification | Random Forest embebido (formato binario propio, fallback a heurísticas) |

### EDRs detectados

```
┌─────────────────────────────────────────────────────────────────┐
│  FIRMAS EDR DETECTADAS                                          │
│                                                                 │
│  CrowdStrike    │ csfalcon, CSAgent                             │
│  Microsoft      │ MsMpEng (Defender)                            │
│  SentinelOne    │ SentinelService, SentinelAgent               │
│  Carbon Black   │ carbonblack                                   │
│  Cylance        │ CylanceSvc                                    │
│  Symantec       │ Symantec, Norton                              │
│  McAfee         │ mcafee, MfeTDI                                │
│  Sophos         │ sesvc, sophos                                  │
│  Tanium         │ taniumclient                                   │
│  Elastic        │ elastic-endpoint                               │
│  Palo Alto      │ trap, trapcord                                 │
│  Trend Micro    │ tmlisten, amsp                                 │
│  Kaspersky      │ kavfs, avp                                     │
│  ESET           │ ekrn, eset                                     │
│  BitDefender    │ bdredline, bdagent                             │
│  ... y 15+ más                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Creencias publicadas

| Creencia | Tipo | Significado |
|----------|------|-------------|
| `edr_present` | bool | Hay EDR corriendo |
| `backup_present` | bool | Hay backup software |
| `network_interfaces` | vec | IPs y MACs del host |
| `process_count` | int | Número de procesos |
| `os_type` | string | Linux / Windows |
| `arch` | string | x86_64 / aarch64 |
| `hostname` | string | Nombre del host |
| `user` | string | Usuario actual |

### Configuración

```toml
[agents]
worker_scan_interval_secs = 15
edr_processes = ["csfalcon", "csagent", "msmpeng", "sentinelone",
                 "carbonblack", "cylancesvc", "symantec", "mcafee"]
```

### MITRE ATT&CK

| Técnica | ID | Descripción |
|---------|----|-------------|
| Process Discovery | T1057 | Lista procesos |
| System Info Discovery | T1082 | OS, hostname, arch |
| Security Software Discovery | T1518.001 | 8 firmas EDR (Linux) / 34 (Windows) |
| Network Service Discovery | T1046 | Interfaces de red |
| System Location Discovery | T1614.001 | Geo-localización |

---

## Drone ◆ — Shaper

```
┌─────────────────────────────────────────────────────────────────┐
│  DRONE                                                          │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ Lee          │───▶│ Decide       │───▶│ Ejecuta      │     │
│  │ creencias    │    │ acción       │    │ movimiento   │     │
│  │ de Worker   │    │ óptima      │    │ lateral     │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│                                                                 │
│  Estrategias:                                                   │
│    • Colony mode → atacar TODO lo alcanzable                   │
│    • Heuristic  → EDR? esperar. Backup? atacar backup.         │
│    • MARL       → 62-dim state → Q-network                    │
│                                                                 │
│  Si un Worker muere, Drone lo regenera (spawn directo)         │
└─────────────────────────────────────────────────────────────────┘
```

**Archivo:** `agents/drone/src/main.rs`
**Rol:** Decision-making, lateral propagation, agent regeneration

### Capacidades

| Capacidad | Detalle |
|-----------|---------|
| Toma decisiones | Basado en creencias de Worker |
| Network discovery | nmap / ARP scan de subredes |
| Movimiento lateral | SSH con claves cosechadas |
| Regeneración | Cuando un Worker muere, Drone hace spawn directo |
| Persistencia | Instala claves SSH autorizadas |

### Decision Logic

```
                           ┌──────────────┐
                           │  Creencias   │
                           │  de Worker   │
                           └──────┬───────┘
                                  ▼
                    ┌─────────────────────────┐
                    │   ¿EDR presente?        │
                    │   ┌───┐    ┌───┐       │
                    │   │ SI│    │ NO│       │
                    │   └─┬─┘    └─┬─┘       │
                    │     ▼        ▼         │
                    │  Esperar   ¿Backup?    │
                    │           ┌───┐ ┌───┐  │
                    │           │ SI│ │ NO│  │
                    │           └─┬─┘ └─┬─┘  │
                    │             ▼     ▼    │
                    │        Atacar  Propaga │
                    │        backup  a red   │
                    └─────────────────────────┘
```

### Configuración

```toml
[agents]
drone_decision_interval_secs = 30

[colony]
aggressive = true
scan_subnets = ["192.168.1.0/24", "10.0.0.0/24"]
max_concurrent_infections = 5
```

### MITRE ATT&CK

| Técnica | ID | Descripción |
|---------|----|-------------|
| SSH Remote Services | T1021.004 | Movimiento lateral |
| Lateral Tool Transfer | T1570 | SCP de bins |
| System Process Creation | T1543.002 | Regeneración de workers |
| Boot/Logon Autostart | T1547.001 | Persistencia SSH |

---

## Honeybee ◉ — Hoarder

```
┌─────────────────────────────────────────────────────────────────┐
│  HONEYBEE (build solo-simulación, 2026-09-14)                   │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ Recibe       │───▶│ SIMULA       │───▶│ SIMULA       │     │
│  │ proposals    │    │ encrypt      │    │ exfil/destroy│     │
│  │ del enjambre │    │ (no-op +     │    │ (no-op +     │     │
│  │              │    │  telemetry)  │    │  telemetry)  │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│                                                                 │
│  Ningún archivo se lee, cifra, envía ni borra.                  │
│  Solo ejecuta con consenso ≥80% (HiveMind)                      │
│  Soporta: privesc (SUID, sudo, Docker, PwnKit)                  │
│           remote shell, cloud pivot (AWS, GCP, Azure)           │
└─────────────────────────────────────────────────────────────────┘
```

**Archivo:** `agents/honeybee/src/main.rs`
**Rol:** Action execution (simulation-only), remote shell, privesc

> ⚠️ **Deshabilitado por diseño:** las rutas destructivas (cifrado
> AES-256-GCM de archivos, borrado seguro, exfiltración por C2 y las
> cápsulas Chrononaut asociadas) fueron eliminadas del binario el
> 2026-09-14 a petición del propietario. Ese mismo día (ronda 3) el
> módulo `chrononaut` completo y el módulo `propaganda` (phishing con
> IA) se eliminaron de `hive_base`. En la ronda 4 la política se extendió
> a TODO el enjambre (matriz en `docs/CAPABILITIES.md`): exfil, sabotaje,
> persistencia, escalada, evasión y anti-forense son simulados con
> telemetría etiquetada. Independiente de `safe_mode`:
> las acciones se registran como simuladas y publican telemetría
> honesta (`"simulated (destructive actions disabled)"`).

### Capacidades

| Capacidad | Detalle |
|-----------|---------|
| Acciones destructivas | **Solo simulación** — no-op + log + belief `simulated` |
| Consensus-gated | Requiere 80% de aprobación HiveMind (el flujo de votación se conserva) |
| Remote shell | Shell interactivo WebSocket bajo demanda del C2 |
| Privesc | SUID, sudo, LD_PRELOAD, Docker, PwnKit, DirtyPipe |
| Cloud pivot | AWS STS/EC2/S3, GCP Compute/IAM, Azure VM/KeyVault |
| Target discovery | Solo para métricas de simulación (conteo de rutas) |

### Acciones simuladas (formato de telemetría)

```
┌─────────────────────────────────────────────────────────────────┐
│  RESULTADO DE ACCIÓN (belief publicado al enjambre)             │
│                                                                 │
│  encrypt_result : "simulated (destructive actions disabled)"    │
│  exfil_result   : 0 bytes (egress deshabilitado)                │
│  destroy_result : log "destroy simulated" (no-op)               │
└─────────────────────────────────────────────────────────────────┘
```

### Configuración

```toml
[c2]
url = "https://tu-c2.com:8444/collect"
api_key = "supersecreto"

[consensus]
threshold = 0.8
```

### MITRE ATT&CK

Técnicas que este agente **simulaba** (conservadas como referencia
formativa para detectarlos en un entorno real; el binario ya no las
implementa):

| Técnica | ID | Descripción (ya no ejecutada) |
|---------|----|-------------------------------|
| Data Destruction | T1485 | 3-pass wipe (eliminado) |
| Exfiltration Over HTTP | T1048.002 | POST a C2 (eliminado) |
| Data from Local System | T1005 | Documentos, .ssh, .aws (eliminado) |

---

## Swarm ⬡ — Worm

```
┌─────────────────────────────────────────────────────────────────┐
│  SWARM                                                          │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ Lee          │───▶│ Selecciona   │───▶│ Se propaga   │     │
│  │ creencias    │    │ targets via  │    │ vía SSH + SCP│     │
│  │ de Worker   │    │ MARL policy  │    │              │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│                                                                 │
│  Auto-limitante:                                                │
│    • Max 10 hops → self-destruct                                │
│    • Max 2 infecciones/min                                      │
│    • Self-destruct después de 1h                                │
│    • Evita hosts con EDR (lee creencias de Worker)             │
│    • No requiere consenso — propaga autónomamente              │
└─────────────────────────────────────────────────────────────────┘
```

**Archivo:** `agents/swarm/src/main.rs`
**Rol:** Autonomous propagation, no-consensus spreading

### Capacidades

| Capacidad | Detalle |
|-----------|---------|
| Autónomo | Propaga sin esperar consenso HiveMind |
| Target scoring (Q-values heurísticos sobre el clasificador scout; RL real es trabajo pendiente, ver ROADMAP) | Prioriza hosts de alto valor y bajo EDR |
| SSH key auth | Prueba todas las claves cosechadas |
| SCP deploy | Copia binario y ejecuta remoto |
| Auto-limitante | 10 hops, 2/min, 1h de vida |
| EDR avoidance | Lee creencias de Worker |

### Ciclo de vida

```
NACE ────────────────────────────────────────────────────────── MUERE
  │                                                              │
  ▼                                                              ▼
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ Spawnea  │───▶│ Escanea  │───▶│ Infecta  │───▶│ Salta a  │
│ target 1  │    │ target 2 │    │ target 3 │    │ target 4 │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
                                                     │
                                                     ▼    (hop ≥ 10
                                                  ┌──────────┐  o 1h
                                                  │ SELF-    │  pasado)
                                                  │ DESTRUCT │
                                                  └──────────┘
```

### Configuración

```toml
[agents]
swarm_max_hops = 10
swarm_max_infections_per_minute = 2
swarm_self_destruct_secs = 3600

[brain]
safe_ips = ["192.168.1.100", "192.168.1.1"]
```

### MITRE ATT&CK

| Técnica | ID | Descripción |
|---------|----|-------------|
| SSH Remote Services | T1021.004 | Propagación |
| Lateral Tool Transfer | T1570 | SCP de binarios |
| System Checks | T1497.001 | Evita hosts con EDR |

---

## Comunicación entre agentes (Arena)

```
┌─────────────────────────────────────────────────────────────────┐
│  ARENA — Memoria compartida (shm_open / mmap)                  │
│                                                                 │
│  ┌────────────┐                                                 │
│  │ Arena      │  /dev/shm/hive_arena                           │
│  │ Header     │  Magic: 0x48495645 ("HIVE")                    │
│  ├────────────┤  Slots: 16                                     │
│  │ Slot 0     │  Tamaño: 8KB por slot                         │
│  │ Slot 1     │                                                 │
│  │ Slot 2     │  Lock-free: seq counters + atomic flags        │
│  │ ...        │                                                 │
│  │ Slot 15    │  Serialización: MessagePack (rmp-serde)        │
│  └────────────┘                                                 │
│                                                                 │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │  QUEEN   │  │ WORKER   │  │  DRONE   │  │HONEYBEE  │       │
│  │  Slot 0  │  │  Slot 1  │  │  Slot 2  │  │  Slot 3  │       │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘       │
│                                                                 │
│  Ventajas:                                                      │
│    • Sin TCP → sin puertos abiertos                             │
│    • Sin sockets → invisible a netstat                         │
│    • Velocidad de RAM → ~50ns por mensaje                      │
│    • Fileless → no hay archivos de socket                       │
└─────────────────────────────────────────────────────────────────┘
```

### Protocolo LdC (Lenguaje de la Colmena)

| Tipo de mensaje | Campos | Ejemplo |
|-----------------|--------|---------|
| `Belief` | asset, value, confidence | `("edr_present", true, 0.95)` |
| `Desire` | action, priority | `("encrypt", 0.8)` |
| `Request` | command, args | `("scan", "192.168.1.0/24")` |
| `Query` | dilemma, context | `("should_move?", {...})` |
| `StatusEvent` | event_type, detail | `("kill_switch", "")` |

---

## Resumen de arquitectura

| Aspecto | Detalle |
|---------|---------|
| Lenguaje | Rust 1.70+ |
| IPC | Memoria compartida (shm_open / mmap) |
| Serialización | MessagePack (rmp-serde) |
| C2 | HTTP(S), DNS Tunnel, ICMP Tunnel, Dead Drop |
| Failover | Priority → Race → RoundRobin |
| Consenso | HiveMind (voting, 66% threshold) |
| ML | Random Forest embebido en runtime; DQN/PPO en training/ (export a runtime pendiente, ver ROADMAP) |
| Evasión | 10 capas (IPC fileless, syscalls, anti-debug, ...) |
| Target | Linux x86_64, Windows x86_64 (cross-compile) |

> **Nota ronda 5 (2026-09-14):** integrado PR #1 (`audit-improvements`): licencia
> MIT, TLS sin `danger_accept_invalid_certs` salvo en `HIVE_LAB_MODE`, fix de
> doble `close()` de fd en `fileless`, `colony_key()` (`HIVE_MASTER_KEY`) para
> stigmergy/phoenix, `default-members` para el orden de build de stinger.
> `honeycomb` pasa a EMULADO: persistencia y EFI sin escrituras; desinstalación
> quirúrgica de remediación (sin `crontab -r`) que respeta
> `HIVE_PERSISTENCE_DRY_RUN=1`.

> **Nota ronda 6 (2026-09-14):** ELIMINACIÓN total de módulos ofensivos y
> simulados por decisión del propietario ("hazlo real o elimínalo"): exploits,
> exfil, nectar, leech, saboteur, kerberos, smb, hades_gate, stack_spoof,
> anti_analysis, anti_forensics, cloud_worker, honeycomb, seer,
> channel_rotator, death_dance, agente worm (swarm), `syscalls::windows` y el
> endpoint `POST /collect` del C2. Podas: privesc (solo scan), lateral (solo
> discover_hosts), phoenix (solo genoma en memoria), swarming (solo decisión).
> TaskPoller: tareas destructivas → `rejected`. Matriz: `docs/CAPABILITIES.md`.
