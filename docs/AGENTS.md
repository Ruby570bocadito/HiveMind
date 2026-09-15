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
| [Honeybee](#honeybee--hoarder) | ◉ | Hoarder — tareas del operador; acciones destructivas ELIMINADAS (ronda 6) | `agents/honeybee/` |
| ~~Swarm~~ | ⬡ | Worm — ELIMINADO en ronda 6 (ver `docs/CAPABILITIES.md`) | *(sin código)* |

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
url = "https://tu-c2.com:8444"  # BASE del C2 (sin /collect: eliminado en ronda 6)
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

## Swarm ⬡ — ELIMINADO (ronda 6)

El agente worm autónomo (`agents/swarm/`: propagación SSH/SCP, scoring de
targets con Q-values heurísticos, auto-límites de hops/velocidad) fue
**eliminado del repositorio** en la ronda 6 por decisión del propietario
("hazlo real o elimínalo"). No queda código ni binario; las referencias que
quedaban en compose/Helm/scripts de despliegue se retiraron en la ronda 10.

Historial: descrito como agente en las rondas 1–5 (emulación), eliminado en
la ronda 6. Matriz de capacidades vigente: `docs/CAPABILITIES.md`.

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
| ML | Random Forest embebido en runtime; pipeline reproducible (dataset → train → `training/export_bin.py` → `.bin`); DQN/PPO en training/ |
| Evasión | ELIMINADA (ronda 6–7); solo queda opsec timing y transporte TLS |
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

> **Nota ronda 7 (2026-09-14):** purga de dead-paths de evasión (código real
> sin consumidores que sobrevivió a las rondas previas): `reactive_llm`
> (ofuscador polimórfico vía Ollama + mutación de binarios) y `io_uring_ops`
> (E/S encubierta estilo RingReaper). Pipeline ML ahora reproducible
> end-to-end: `training/export_bin.py` convierte el RF de sklearn al formato
> `.bin` de `hive_base::ml` con validación de paridad contra sklearn;
> `train_classifier.py` lo invoca automáticamente. Tests: 275 (226 unit).

> **Nota ronda 8 (2026-09-15):** migración `rlua → mlua 0.10` (lua54 vendored)
> en `beekeeper::scripting` con API pública intacta y 4 tests nuevos; test de
> roundtrip del modelo ML embebido en `worker` (descifrar → parsear →
> clasificar, sin Python) que destapó y corrigió un bug real: `from_binary`
> asignaba memoria según cabeceras no confiables (~512 GB con entrada
> corrupta). CI reparada: triggers malformados (`branches: aster]`), `weaver`
> aún listado (eliminado en ronda 2) y jobs modernizados. Split de
> `hive_base` por features: análisis en ROADMAP → diferido. Tests: 281
> (232 unit).
>
> ⚠️ **Corrección (ronda 9):** el diff de la ronda 8 NO tocó el bloque
> `on:` de ci.yml — el trigger `aster]` sobrevivió a esa ronda y el fix se
> materializó de verdad en la ronda 9.

> **Nota ronda 9 (2026-09-15):** endurecimiento del protocolo de memoria del
> registro del arena (flags 100% atómico: CAS 0→RESERVED(0x80), relleno de
> identidad en reserva, publicación con `fetch_or(ACTIVE, Release)`, DEAD
> pegajoso, `role` atómico — layout intacto); el análisis destapó que la
> identidad se escribía DESPUÉS de publicar el slot y que `mark_agent_dead`
> hacía RMW no atómico contra el CAS. Verificación con loom del archivo REAL
> vía crate `loom-model` (#[path] + `--cfg loom`): 3 modelos exhaustivos
> (claims concurrentes, publicación→lectura Acquire, DEAD vs reserva); lección
> documentada: los atómicos de loom deben CONSTRUIRSE, no proyectarse sobre
> shm ceroada. CI: triggers reales (`branches: [master]`, la ronda 8 no llegó
> a tocarlos), `fmt --check` y `clippy -D warnings` bloqueantes, job loom,
> job ML end-to-end (dataset → train → export → paridad; `train_classifier.py`
> ahora sale 1 si el export falla — el WARNING enmascaraba fallos) y artifacts
> de release subidos. Higiene dual-use: `PrivEscResult` muerto eliminado,
> cabeceras de decisión en `privesc.rs` (solo lectura) y `remote_shell.rs`
> (núcleo C2 tras auditoría del poller). Tests: 286 (237 unit).

> **Nota ronda 10 (2026-09-15):** TUI del operador reparado y completado —
> (1) el buffer de telemetría del TUI se adjunta AHORA al segmento shm real
> (`arena_mgr::connect_to_arena`, misma vía que los agentes; antes montaba
> una arena privada en heap y la pestaña HTL Events jamás mostraba nada);
> (2) nuevo `TelemetryBuffer::read_from(pos, max)` con cursor LOCAL del
> observador: sin robar eventos a los drenadores de los agentes y sin los
> duplicados del `peek` por frame (+4 tests, incluye lap del anillo y
> re-anclaje tras re-init); (3) pestañas Consensus y Log con datos reales
> (estado de directivas observado desde Proposal/Vote/StatusEvent/Belief;
> log de operador con joins/leaves, transiciones de directivas y laps);
> (5) ronda 11: consenso real de punta a punta — worker/drone/honeybee votan
>     propuestas (política deny-list compartida), la Queen procesa votos con
>     reputaciones reales y broadcasts aprobaciones; transporte honesto (el
>     masquerade cloud de smoke_signals eliminado; C2 directo vía HIVE_C2_URL);
>     hive.toml parsea de verdad (test de regresión); royal_jelly eliminado.
> (4) `beekeeper status` ya no anuncia módulos eliminados (Saboteur/Seer) y
> los colores ANSI funcionan de verdad. Despliegue reparado: Dockerfile
> (construía menos binarios de los que COPY; swarm fuera; `--loot-dir`
> fuera del CMD), docker-compose.yml (C2 arrancable, swarm/victim fuera),
> `docker/lab/` y `Dockerfile.lab` eliminados (roto + era ofensiva),
> `lab_setup.sh` reescrito contra el compose lab de la raíz, chart Helm
> parseable y des-escalado (ver `deploy/charts/hive/README.md`).
> Deps Python: torch/onnxruntime/protobuf/skl2onnx fuera de requirements
> (sin importador). Tests: 290 (241 unit).
