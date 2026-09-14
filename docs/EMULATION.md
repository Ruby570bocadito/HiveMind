# Política de Emulación — Hive Colony (ronda 4)

Hive Colony es un **enjambre de agentes red team para laboratorios
autorizados**. Desde la ronda 4, el repo distingue entre lo que puede
ejecutarse de verdad (enumeración de solo lectura, transporte, consenso,
ejecución de comandos con auditoría) y lo que siempre se **simula** (todo
efecto destructivo, exfiltración, persistencia y evasión). Esta página es la
referencia autorizada de qué es qué.

## Principios

1. **Lab-only.** El target válido es infraestructura propia o autorizada
   (`docker-compose.lab.yml`, `scripts/lab_setup.sh`).
2. **Enumeración real, ejecución simulada.** Encontrar vectores enseña;
   ejecutarlos contra producción no tiene lugar en este repo.
3. **Telemetría etiquetada.** Cada paso simulado emite eventos con la marca
   `(simulated)` / `(emulation mode, ronda 4)` para que el equipo azul pueda
   distinguirlos y correlacionarlos en su SIEM.
4. **Sin código armable.** No se versionan exploits funcionales, ofuscadores,
   stagers ni mecanismos de persistencia real. El histórico de purgas está en
   `docs/agentes/`.

## Matriz por módulo (`hive_base`)

| Módulo | Estado | Detalle |
|--------|--------|---------|
| `privesc` | HÍBRIDO | `scan_privilege_escalation()` real (solo lectura: SUID, sudo, capabilities, cron, docker, NFS, kernel, contenedores). `attempt_escalation()` simulado. |
| `leech` | SIMULADO | `harvest_*()` devuelven lotes enmascarados `SIMULATED::********`. `discover_credential_sources()` solo comprueba existencia de rutas. |
| `exploits` | SIMULADO | EternalBlue/BlueKeep/Log4Shell/DCSync/AACL/RedQueen devuelven `ExploitResult { simulated: true }`; el código de explotación real fue eliminado. |
| `exfil` | SIMULADO | `dns_exfiltrate`/`http_exfiltrate` no emiten tráfico. `dns_encode` y `ExfilScheduler` (planificación pura) se conservan para detección. |
| `lateral` | HÍBRIDO | `discover_hosts()` real (ping sweep de lab). `harvest_credentials`/`exec_ssh`/`deploy_agent_ssh` simulados, con gate BRAIN intacto. |
| `phoenix` | HÍBRIDO | Genoma y fragmentación **en memoria** reales; `hide`/`hide_fragment`/`install_persistence`/`rebuild_from_genome` simulados (cero escrituras). |
| `honeycomb` | EMULADO (ronda 5) | `install_persistence`/`install_uefi_bootkit`/`generate_bootkit_stub` simulados (cero escrituras en crontab/systemd/bashrc/EFI). `uefi_bootkit_feasible`/`bootkit_installed` reales (solo lectura). `uninstall_persistence`/`remove_uefi_bootkit` reales pero estrictamente de remediación (solo borran artefactos con marker `HIVE_PERSISTENCE_MARKER`/`.hive_bak`; sin `crontab -r`) y respetan `HIVE_PERSISTENCE_DRY_RUN=1`. |
| `saboteur` | HÍBRIDO | `scan_for_targets()` real (solo lectura). `execute_order()` simulado. |
| `anti_forensics` | SIMULADO | Sin borrado ni timestomping; documenta rutas consideradas. |
| `kerberos` / `smb` | SIMULADO | Sin tráfico hacia DC/hosts; resultados estructurados simulados. |
| `hades_gate` | SIMULADO | Resolución de SSNs eliminada; firmas devuelven `None` con telemetría. |
| `stack_spoof` | SIMULADO | No fabrica pilas sintéticas; conserva introspección de solo lectura y el detector defensivo `detect_stack_spoofing()`. |
| `anti_analysis` | SIMULADO | Sin anti-debug/anti-VM real; estado neutro etiquetado. |
| `fileless` | GATE | `new()` (memfd benigno) libre; `spawn()` exige `HIVE_LAB_AUTHORIZED=1`. Maquinaria Windows (NtCreateSection) eliminada. |
| `remote_shell` | REAL (auditado) | Función operadora del C2; cada comando queda en telemetría. |
| `chaos` | REAL | Ingeniería del caos sobre la propia colonia (recetas con replay). |
| `whispernet` / `smoke_signals` | REAL (transporte) | Canal interno del enjambre; exfiltración hacia fuera purgada (ronda 1). |
| `death_dance` | REAL (estadística) | Gestor A/B de variantes sobre resultados simulados. |
| `obfstr` | REAL | XOR de literales en compile-time; utilidad educativa. |

## TaskPoller y comandos del C2

El TaskPoller (ronda 4) consume `GET /task/:agent_id` y responde por
`POST /beacon` con el frame `{"session": <task_id>, "output": ...}` que el
shell del operador retransmite. Política de comandos:

| Comando | Comportamiento |
|---------|----------------|
| `shell` / `shell_exec` | Ejecución real con auditoría (`remote_shell::execute_command`); el resultado se enruta a la sesión del shell del operador |
| `exfil` | SIEMPRE simulado |
| `encrypt` / `wipe` / `destroy` / `sabotage` | SIEMPRE simulado |
| Otros | `unsupported` (con etiqueta de emulación) |

Tests que fijan esta política: `hive_base/src/task_poller.rs::tests`
(destructivos simulados, exfil simulado, desconocidos soportados como
`unsupported`).

## Qué NO volverá a este repo

- Exploits funcionales o payloads cifrados/embebidos.
- Ofuscadores polimórficos, stagers, inyección de certificados.
- Persistencia real (systemd/cron/UEFI/MBR) o propagación (SSH/worm).
- Exfiltración por canales reales (DNS/HTTP/ICMP hacia dominios externos).
- Anti-forense real (borrado de logs, timestomping) o evasión de EDR
  (resolución de syscalls, spoofing de pila).

Si necesitas alguna de estas capacidades para un ejercicio autorizado, usa
marcos mantenidos por la comunidad de seguridad con los controles adecuados
(p. ej. Atomic Red Team o Caldera en un lab aislado) — Hive Colony se queda
con la parte que le es propia: el enjambre, el consenso y la telemetría.
