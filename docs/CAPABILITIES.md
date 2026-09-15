# Matriz de capacidades (ronda 6)

**Política tras la ronda 6:** los módulos ofensivos y simulados fueron
**ELIMINADOS** del repositorio, no emulados. Histórico: hard-disabled en
ronda 1, emulación en rondas 4–5 (ver `docs/agentes/`), eliminación total en
ronda 6 por decisión del propietario ("hazlo real o elimínalo" — hacerlos
reales no era una opción; se eliminaron).

## Eliminado en ronda 6 (ya no existe en el código)

| Módulo (antes) | Qué hacía al eliminarse |
|----------------|--------------------------|
| `exploits` | Resultados simulados de EternalBlue/BlueKeep/Log4Shell/etc. |
| `exfil` | Exfiltración DNS/HTTP (simulada desde ronda 1) + `ExfilScheduler` |
| `nectar` | Subida troceada de ficheros locales (storm upload) |
| `leech` | Recolección de credenciales (shadow, SSH, cloud tokens, browser) |
| `lateral` (parte) | `harvest_credentials`, `exec_ssh`, `deploy_agent_ssh` |
| `saboteur` | Escaneo de objetivos + órdenes de sabotaje |
| `kerberos` / `smb` | Módulos de ataque a DC/hosts (simulados) |
| `hades_gate` | Resolución de SSNs (Hell's/Halo's Gate) |
| `stack_spoof` | Fábrica de pilas sintéticas (Windows) |
| `anti_analysis` | Anti-debug/anti-VM/anti-sandbox |
| `anti_forensics` | Borrado/timestomping documental |
| `cloud_worker` | Recolección de tokens cloud (AWS/GCP/Azure/K8s) |
| `honeycomb` | Persistencia (crontab/systemd/bashrc/UEFI bootkit) |
| `phoenix` (parte) | `hide`/`hide_fragment`/`install_persistence`/`scan_for_fragments` |
| `privesc` (parte) | `attempt_escalation` + `ExploitTracker` (se conserva el scan) |
| `seer` | Predictor de detección para decisiones de evasión |
| `channel_rotator` | Rotador de canales de exfiltración |
| `death_dance` | A/B de variantes de ataque |
| `syscalls::windows` | Syscalls NT directas + maquinaria de inyección |
| `swarm` (agente) | Worm autónomo (hops/rate/TTL) |
| C2 `POST /collect` | Recepción y almacenamiento de exfiltración (loot) |
| C2 python `/jndi` | Callback Log4Shell |

## Eliminado en ronda 7 (dead-paths de evasión sin consumidores)

Detectados en la auditoría de ronda 7: código de evasión **real** que había
sobrevivido a las rondas previas por no tener ningún consumidor (dead code,
no simulación). Se eliminan bajo el mismo mandato.

| Módulo (antes) | Qué hacía al eliminarse |
|----------------|--------------------------|
| `reactive_llm` | Ofuscador polimórfico vía Ollama: reescritura de código con nombres cambiados, dead-code inyectado y strings ofuscados; `llm_mutate_binary` mutaba bytes de binarios (XOR) para variar el hash (evasión AV) |
| `io_uring_ops` | E/S encubierta vía io_uring (Linux 5.1+) para saltarse hooks de libc y la visibilidad de los EDR (estilo RingReaper), con syscalls reales x86_64 |
| `training/train_model.py` | Script legacy (8 features) que exportaba a ONNX un formato que el runtime no puede parsear; sustituido por el pipeline scout + `export_bin.py` |

## Lo que queda (REAL)

| Componente | Estado | Detalle |
|------------|--------|---------|
| Arena IPC | REAL | Anillo lock-free en memoria compartida, firmas Ed25519 |
| LdC / consenso | REAL | Protocolo de mensajes + votación por reputación |
| C2 server | REAL | `/beacon`, `/task`, `/shell` (WS), `/admin/*`, `/logs`; auth `x-api-key` constante-time, rate-limit, CORS cerrado |
| TaskPoller | REAL | Poll/exec/report; comandos `exfil`/destructivos → `rejected` |
| remote_shell | REAL | Función operadora del C2, cada comando en telemetría |
| system_info / worker scan | REAL (solo lectura) | Perfil del sistema, detección EDR/backup |
| privesc `scan_privilege_escalation` | REAL (solo lectura) | Enumeración SUID/sudo/caps/cron/docker/NFS/kernel |
| lateral `discover_hosts` | REAL (solo lectura) | Ping sweep del segmento de lab |
| phoenix genoma | REAL (en memoria) | Modelado/fragmentación/reensamblado sin escrituras |
| fileless | GATE | `memfd_create` benigno libre; `spawn()` exige `HIVE_LAB_AUTHORIZED=1` |
| whispernet / smoke_signals / c2_channels | REAL (transporte) | Canales internos del enjambre; TLS estricto salvo `HIVE_LAB_MODE` |
| opsec | REAL (timing) | Jitter/ventanas horarias del beacon (sin anti-análisis) |
| stigmergy | REAL | Trails cifrados con `colony_key()` (`HIVE_MASTER_KEY`) |
| chaos / tournament / ML / hivemind | REAL (interno) | Ingeniería interna de la colonia, sin efectos externos |

## Ronda 9 — endurecimiento del arena e higiene dual-use (2026-09-15)

**Protocolo de memoria del registro del arena (`hive_base::shared_arena`):**
el byte `flags` es ahora 100% atómico. Antes: el claim era un CAS (fix TOCTOU
de la ronda 6) pero pass-1/enumerate loían `flags` sin atomicidad,
`mark_agent_dead` hacía `flags |= 2` no atómico (RMW que podía pisar un CAS
concurrente o perderse), y `agent_id`/`verifying_key` se escribían DESPUÉS de
publicar el slot como activo — lectores sincronizados por `flags` no tenían
arista happens-before hacia esas escrituras y podían enumerar identidad
corrupta. Ahora: `CAS FREE→RESERVED(0x80)` → relleno de identidad en reserva →
publicación con `fetch_or(ACTIVE, Release)`; DEAD es pegajoso vía `fetch_or`;
`role` es `AtomicU8`. Layout de memoria intacto (mismos tamaños/offsets).

**Verificación exhaustiva con loom:** nuevo crate `loom-model` que incluye el
`shared_arena.rs` REAL vía `#[path]` y lo compila con `--cfg loom` (shims de
atómicos + MAX_AGENTS/MAX_MESSAGES=2). Tres modelos exhaustivos de
intercalados: (1) claims concurrentes → slots distintos, identidad jamás
basura; (2) publicación de mensaje → lectura Acquire, seqs únicos (incluido
el caso seq==0); (3) `mark_agent_dead` en carrera con reserva → el bit DEAD
no se pierde jamás. Lección documentada en `loom-model/src/lib.rs`: los
atómicos de loom deben CONSTRUIRSE (`AtomicU64::new` + move), no proyectarse
sobre memoria shm ceroada — con memoria ceroada loom produce celdas no
registradas y comportamientos imposibles.

**Higiene dual-use:** `PrivEscResult` (struct de ejecución de escalada de una
era anterior, sin productor desde la ronda 6) eliminado. `privesc.rs`
documentado como enumeración de vectores de SOLO LECTURA (estilo linpeas,
triage defensivo de anfitriones propios; no existe ruta de código que
escale) y `remote_shell.rs` como núcleo C2 legítimo (tasking auditado por
`task_poller` con deny-list destructiva desde la ronda 6) — coherente con
`lateral.rs` y `fileless.rs`, que ya llevaban cabeceras de decisión.

**CI real:** triggers `branches: [master]` (el YAML anterior era sintáctica
pero semánticamente inválido — la CI nunca pudo dispararse), `cargo fmt
--check` y `clippy -D warnings` como gates bloqueantes, job loom dedicado,
job ML end-to-end (dataset → train → export → validación de paridad;
`train_classifier.py` ahora falla de verdad si el export falla) y artifacts
de release subidos.

## Ronda 10 — observador del TUI real + despliegue reparable (2026-09-15)

**TUI del operador:** la pestaña HTL Events estaba muerta por diseño — el
TUI montaba el buffer de telemetría sobre una arena PRIVADA (`alloc_zeroed`)
en vez de adjuntarse al segmento shm de la colonia, y además re-apilaba con
`peek` el mismo lote de eventos en cada frame. Ahora: attach real vía
`arena_mgr::connect_to_arena()` y lectura con cursor LOCAL mediante el nuevo
`TelemetryBuffer::read_from(pos, max)` (no toca el cursor compartido: no roba
eventos a los drenadores de los agentes; clamp anti-lap documentado y
testeado). Las pestañas Consensus y Log — sin productor desde su creación —
muestran ahora estado real: directivas observadas del flujo de mensajes
(Proposal/Vote/StatusEvent/Belief) y log de operador (joins/leaves,
transiciones de directivas, laps del anillo). La pestaña es SOLO OBSERVACIÓN:
el TUI no ejecuta directivas ni tareas.

**Higiene dual-use del despliegue (restos de la era ofensiva):** eliminados
el servicio worm `swarm` y el servicio `victim` (siembra de credenciales
falsas para exfil, sin consumidor desde la ronda 6) de docker-compose.yml;
`--loot-dir` (flag del C2 retirada en la ronda 6) fuera del Dockerfile CMD y
de los compose — el C2 volvía a crashear al arrancar; `docker/lab/` compose
(roto: `networks: ive-net]` en todos los servicios, worm swarm, comentarios
de propagación/exfiltración) y `Dockerfile.lab` eliminados;
`scripts/lab_setup.sh` reescrito sin la ceremonia de claves para "harvesting"
(Leech fue eliminado en ronda 6) y sin exigir el binario swarm.
`deploy/charts/hive`: parseable de nuevo (values.yaml tenía un error YAML),
sin agente swarm, sin volumen loot, securityContext des-escalado (non-root,
sin privilegios, RBAC solo ServiceAccount) y nombre de arena shm válido.

## Ronda 11 — consenso real, transporte honesto y config viva (2026-09-15)

**Consenso HiveMind funcional de punta a punta (antes decorativo):** ningún
binario procesaba `Payload::Vote` y el tally de la reina ponderaba solo con
su propia reputación — ninguna directiva podía aprobarse jamás en runtime.
Ahora worker/drone/honeybee VOTAN cada propuesta una vez (política
compartida `HiveMind::vote_decision_for`: deny-list de la ronda 6 → Reject,
resto → Support), y la Queen procesa los votos con
`ConsensusEngine::reputation_map()` (reputaciones reales de agentes activos)
y broadcast de la aprobación (`hive_directive_approved` + belief
`directive:<id>`). El TUI (pestaña Consensus, ronda 10) ya visualizaba ese
flujo — ahora hay algo real que ver.

**Higiene dual-use del transporte (resto de la era ofensiva):**
`smoke_signals` enviaba beacons POST a proveedores cloud REALES (Windows
Update, Office 365, Google Drive, Apple Push, CloudFront…) con User-Agents
suplantados y payloads disfrazados (`build_smoke_beacon`: SOAP/WS masquerade)
— y además JAMÁS llegaban al C2: en lab mode caían a `/tmp/smoke_beacons/`,
y en producción eran tráfico de basura a terceros. Eliminado el masquerade
(hosts/rutas/UAs, `build_smoke_beacon`, `best_channel_for_org`); el sink
local de lab se conserva y la entrega al C2 es DIRECTA vía `HIVE_C2_URL`
(primaria) con los canales alternativos como respaldo.

**Bug de contrato `HIVE_C2_URL`:** docs/trataban la variable como BASE
(`GET {base}/task/{id}`) pero compose/helm/scripts la configuraban con
sufijo `/beacon` → el TaskPoller consultaba `/beacon/task/…` (404) y jamás
recogió tareas en ningún despliegue documentado. Ahora `normalize_c2_base`
acepta ambas formas y todos los despliegues convergen a la base desnuda.

**Config viva:** el `hive.toml` enviado tenía la sección `[eartbeat]` rota y
exigía campos de agentes eliminados (weaver/worm) — NUNCA parseó y toda la
colonia corría con defaults silenciosamente; `config-check` además lo
reportaba como "no config file found" con exit 0 (y `hive.sh doctor` daba un
✓ falso). Arreglado: parsea de verdad, `config-check` distingue
"inexistente" de "roto" (exit 1) y hay test de regresión del fichero enviado.
Campos muertos `weaver_mutation_interval_secs`/`worm_*` fuera del struct.

**Otros:** `royal_jelly.rs` eliminado (código muerto con directivas
ExfiltrateNow/MaximizeSpread/AvoidEDR/SabotageIntegrity y un TTL que comparaba
bits de UUID con segundos); retención de beacons en el C2 (tope 10k, corte
O(1) por id) y claim de tareas acotado por `agent_id`; fix de deadlock de
pipe en `execute_command_with_timeout` (>64 KB → falso TIMEOUT con pérdida de
salida); lint progresivo `deny(clippy::unwrap_used)` en
comms/shared_arena/telemetry/ldc; `launch_colony.sh` reescrito (era
inejecutable desde la ronda 6) y `PLAYBOOK.md` reescrito de cero.

## Ronda 12 — ejecución de directivas, barrido dual-use completo y limpieza profunda (2026-09-15)

**El consenso gana un ejecutor (y se descubre que seguía roto en el último
eslabón):** un test de regresión nuevo destapó que la ronda 11 dejaba el
ciclo a medias por un bug de identidad: `process_arena_message` registraba
la directiva con un `Uuid::new_v4()` INTERNO mientras los votos viajan con
el `proposal_id` del wire — `cast_vote` nunca encontraba la directiva, el
tally nunca se ejecutaba y la reina jamás aprobaba una propuesta del arena.
FIX: `propose_directive_with_id` registra la directiva con el id del wire.
Además el path de voto devolvía el literal `"approved"` en la posición de la
acción (logs de la reina confusos); ahora devuelve la acción real.

**Ciclo completo propose → vote → approve → EXECUTE:** worker/drone/honeybee
registran `proposal_id → action` al votar; al recibir
`hive_directive_approved` consultan la allow-list de ejecución
(`HiveMind::execution_plan_for`: solo `prop_to_*`, gates `HIVE_LAB_MODE=1` +
`HIVE_LAB_SUBNET` — nunca contra la lista segura de `panal`) y ejecutan un
barrido de alcanzabilidad de solo lectura (`lateral::discover_hosts`,
AHORA PARALELO: pool de 32 sondas, de ~4 min a ~8 s worst-case). El
resultado REAL se publica como belief `hosts:<segmento>` + `StatusEvent
directive_executed` (o `directive_execution_skipped` con el motivo — sin
entorno de lab, acuse honesto, no teatro). El TUI (Consensus tab) marca el
estado `executed`. C2: nuevo `GET /admin/metrics` con contadores REALES de
la BD (agentes registrados, beacons, tasks pending/claimed/completed).

**Barrido dual-use completo (el resto de la era ofensiva que la ronda 11
dejó fuera):**
- `opsec::DecoyProfile::fire_decoys` enviaba peticiones HTTP REALES a
  terceros (crl.microsoft.com, ocsp.digicert.com, cdn.cloudflare.net,
  settings-win/vortex-win.data.microsoft.com, api-global.netflix.com) con
  User-Agents suplantados en cada ciclo de heartbeat (probabilidad 0.3) —
  la misma clase de masquerade eliminada de smoke_signals en la ronda 11.
  ELIMINADO: el OPSEC queda reducido a timing (jitter determinista +
  calendario horario); sin red, sin mimicry de UAs, sin anti-análisis
  (`evasion_check` sandbox/debugger/EDR fuera del gate de `should_act` —
  CAPABILITIES ya declaraba "sin anti-análisis" y ahora el código lo
  cumple; `platform_layer::runtime` queda sin primitivas de evasión).
- `c2_channels` sin familia encubierta: `DomainFront` (fronting por CDN con
  UA suplantado y ruta `/collect`), `DeadDrop` (pastebin/S3/Gist),
  `DnsTunnel` (beacon como labels DNS) e `IcmpTunnel` (payloads en echo
  ICMP) ELIMINADOS junto a sus env vars de registro en comms
  (`HIVE_C2_DNS_DOMAIN`, `HIVE_C2_ICMP_TARGET`, `HIVE_C2_DEAD_DROP_TOKEN` —
  leídas pero documentadas en NINGÚN sitio). El FailoverDirector conserva
  su maquinaria real (prioridad/race/round-robin, cooldown, stats) sobre
  el canal Http (entrega directa al C2 + sink de lab, ronda 11).
- `docker-compose.yml` deja de inyectar `HIVE_C2_DNS_DOMAIN=tunnel.example.com`.

**Limpieza profunda (13 módulos muertos, ~2.460 líneas):** `did.rs`,
`federation.rs` (covert channels + "bypass techniques" inter-hive),
`hive_scale.rs`, `homomorphic.rs`, `marl_online.rs` ("rewards from real
attacks"), `c2_bridge.rs` (traductores LdC → Sliver gRPC / Cobalt Strike
Beacon SMB), `pheromone.rs`, `waggle_dance.rs`, `obfstr.rs` (macro XOR de
ofuscación), `syscalls.rs` (syscalls directas anti-hook), `guardian.rs`
(honeypot detection "before attacking"), `hibernation.rs` (supervivencia
ante IR vía "honeycomb persistence" — eliminada en ronda 6) y
`swarming.rs` — TODOS sin un solo consumidor en el workspace (verificado
con rg sobre use/llamadas/re-exports/tests). Varios además traían carga
ofensiva. El macro `obf!` solo tenía un test como consumidor: test fuera
con el módulo. Lint progresivo escalón 2: `deny(clippy::unwrap_used)`
(_fuera_ de tests) extendido a crypto/consensus/config/hivemind/opsec/
identity/task_poller/panal/lateral; único `unwrap` de producción del grupo
(crypto `try_into` tras length-check) convertido en `.ok()?` infalible.
