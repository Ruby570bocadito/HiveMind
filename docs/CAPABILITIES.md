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
