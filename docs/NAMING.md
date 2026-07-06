# Naming Convention Map: Documentation ↔ Code

Los documentos de diseño usan términos genéricos de "enjambre" (Swarm).
El código usa la metáfora biológica de "colmena" (Hive) por ser más rica en roles y comportamientos.

## Mapeo principal

| Concepto (docs) | Nombre en código | Rol biológico | Función técnica |
|---|---|---|---|
| **Swarm** | **Hive / Colmena** | Colonia completa | Proyecto entero |
| **Scout** | **Worker** | Abeja obrera exploradora | Percepción: escanea sistema, detecta EDR/backup, publica beliefs |
| **Shaper** | **Drone** | Zángano (macho reproductor) | Decisiones: movimiento lateral, persistencia, regeneración de agentes |
| **Hoarder** | **Honeybee** | Abeja recolectora de néctar | Ejecutor: cifrado, exfiltración, destrucción cuando hay consenso |
| **Weaver** | **Weaver** | Abeja tejedora de cera | Camuflaje: mutación polimórfica, generación de variantes |
| **Overmind** | **Queen** | Abeja reina | Oráculo: responde dilemas estratégicos vía LLM |
| **Worm** | **Swarm** | Enjambre autónomo | Propagación autónoma entre hosts |

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
| C2 Bridge | `c2_bridge.rs` | Comunicación externa opcional |
| Panal (Safe Target Check) | `panal.rs` | Verifica que un host no esté en lista segura antes de atacar |
| Jalea Real (Payload Mut) | `royal_jelly.rs` | Mutación avanzada de payloads |
| Cera (Seal/Unseal) | `wax.rs` | Cifrado/descifrado de datos |
| Néctar (Exfil) | `nectar.rs` | Exfiltración de datos |
| Danza del Abejorro | `waggle_dance.rs` | Descubrimiento de rutas de red |
| Feromona (Stigmergy) | `stigmergy.rs` | Comunicación indirecta vía entorno |
| Larva (Regeneración) | `larva.rs` | Regeneración de agentes caídos |
| Guardián | `guardian.rs` | Protección anti-análisis |

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
