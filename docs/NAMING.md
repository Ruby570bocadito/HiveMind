# Naming Convention Map: Documentation ↔ Code

Los documentos de diseño usan términos genéricos de "enjambre" (Swarm).
El código usa la metáfora biológica de "colmena" (Hive) por ser más rica en roles y comportamientos.

## Mapeo principal

| Concepto (docs) | Nombre en código | Rol biológico | Función técnica |
|---|---|---|---|
| **Swarm** | **Hive / Colmena** | Colonia completa | Proyecto entero |
| **Scout** | **Worker** | Abeja obrera exploradora | Percepción: escanea sistema, detecta EDR/backup, publica beliefs |
| **Shaper** | **Drone** | Zángano (macho reproductor) | Decisiones: movimiento lateral, persistencia, regeneración de agentes |
| **Hoarder** | **Honeybee** | Abeja recolectora de néctar | Ejecutor interno: comparte conocimiento (stigmergy), shell del C2, peer P2P whispernet |
| **Overmind** | **Queen** | Abeja reina | Oráculo: responde dilemas estratégicos vía LLM y tally del consenso |
| ~~**Worm**~~ | ~~**Swarm**~~ | ~~Enjambre autónomo~~ | ELIMINADO (ronda 6): agente worm autónomo |

## Módulos de infraestructura

| Concepto (docs) | Nombre en código | Función |
|---|---|---|
| Lenguaje de Colonia (LdC) | `ldc.rs` | Mensajes estructurados: Belief, Proposal, Vote, Query, StatusEvent |
| Memoria Compartida Efímera | `shared_arena.rs` + `arena_mgr.rs` | `shm_open`/`mmap` arena lock-free entre procesos |
| Comunicación Inter-Agente | `comms.rs` (HiveChamber) | Cliente de la arena compartida |
| Consenso por Reputación | `consensus.rs` | Votación ponderada con umbral 66% |
| Dropper | `stinger/` | Payload inicial que empaqueta y despliega agentes |
| CLI de Control | `beekeeper/` | Consola del operador |
| CLI de Despliegue | `buzz/` | Despliegue rápido |
| Panal (Safe Target Check) | `panal.rs` | Lista de hosts seguros del lab (consumida por la config; los checks ofensivos vivieron solo en `swarming.rs`, eliminado en ronda 12) |
| C2 Bridge | ~~`c2_bridge.rs`~~ | ELIMINADO (ronda 12): muerto, traductores a Sliver/Cobalt Strike de la era ofensiva |
| Jalea Real (Payload Mut) | ~~`royal_jelly.rs`~~ | ELIMINADO (ronda 11): código muerto con directivas de la era ofensiva |
| Cera (Seal/Unseal) | ~~`wax.rs`~~ | ELIMINADO (rondas previas) |
| Néctar (Exfil) | ~~`nectar.rs`~~ | ELIMINADO (ronda 6) |
| Danza del Abejorro | ~~`waggle_dance.rs`~~ | ELIMINADO (ronda 12): sin consumidores |
| Feromona (Stigmergy) | `stigmergy.rs` | Comunicación indirecta vía entorno |
| Larva (Regeneración) | ~~`larva.rs`~~ | ELIMINADO (la regeneración vive en `drone` + `phoenix`) |
| Guardián | ~~`guardian.rs`~~ | ELIMINADO (ronda 12): muerto, anti-análisis de la era ofensiva |
| DID / Federación / Homomórfico / MARL online / HiveScale / Pheromona / Swarming / Syscalls / Hibernación | ~~`did.rs` `federation.rs` `homomorphic.rs` `marl_online.rs` `hive_scale.rs` `pheromone.rs` `swarming.rs` `syscalls.rs` `hibernation.rs`~~ | ELIMINADOS (ronda 12): sin ningún consumidor en el workspace (verificación con rg), varios con carga de la era ofensiva |

## Archivos de configuración

| Archivo | Propósito |
|---|---|
| `hive.toml` | Configuración principal (producción) |
| `colmena.toml` → `hive.toml` | Alias (symlink, cargado primero por el config loader) |

## Por qué "Hive" en vez de "Swarm"

1. **Riqueza semántica**: Una colmena tiene roles mucho más específicos (obrera, zángano, reina, tejedora) que un enjambre genérico
2. **OPSEC**: "Hive" no aparece en listas negras de términos de malware
3. **Productos biológicos**: Cera, miel, jalea real, néctar, feromonas → metáforas para cifrado, datos, mutación, exfiltración, estigmergia
4. **Memorabilidad**: Los nombres de abejas son más fáciles de recordar que "Scout Agent v2"
