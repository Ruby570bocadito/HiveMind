# Deployment Guide 🐝

> **Edición laboratorio (ronda 4).** Hive Colony se despliega en entornos
> que controlas y estás autorizado a operar. Las herramientas de armado de
> payloads (`deploy.sh`, `build_payload.sh`, `obfuscate_pe.py`) fueron
> retiradas del repositorio: la política del proyecto es **emulación**
> (tácticas simuladas con telemetría), no entrega de artefactos armados.

## Índice

1. [Build](#1-build)
2. [Despliegue en laboratorio](#2-despliegue-en-laboratorio)
3. [Variables de entorno](#3-variables-de-entorno)
4. [Verificación](#4-verificación)

## 1. Build

### Linux nativo

```bash
# Build completo (todos los agentes + C2)
./hive.sh build

# Solo un agente (más rápido para desarrollo)
cargo build --release -p honeybee
```

### Windows cross-compile

```bash
# 1. Instalar toolchain (una vez)
rustup target add x86_64-pc-windows-gnu
./setup_cross.sh

# 2. Compilar
cargo build --release --target x86_64-pc-windows-gnu -p queen

# Output: target/x86_64-pc-windows-gnu/release/queen.exe
```

## 2. Despliegue en laboratorio

La vía canónica para montar un ejercicio completo (C2 + agentes + objetivos
SSH + dashboard) es Docker Compose:

```bash
./hive.sh colony         # stack principal (C2 + agentes)
./hive.sh lab            # objetivos SSH del laboratorio (scripts/lab_setup.sh)
```

Flujo operativo del ejercicio:

1. Arranca el C2 (`./hive.sh c2`) y el stack de colonia (`./hive.sh colony`).
2. Conecta el TUI del operador (`./hive.sh tui`) o el shell interactivo del
   C2 (`GET /shell/:session_id` por WebSocket).
3. Crea tareas con `POST /task/:agent_id`; los agentes las recogen con el
   **TaskPoller** (ronda 4) y devuelven resultados vía `POST /beacon`.
4. Las tácticas se ejecutan en **modo emulación**: la enumeración real es de
   solo lectura y los efectos (exfil, sabotaje, persistencia, escalada)
   se simulan con telemetría etiquetada — ver `docs/EMULATION.md`.

## 3. Variables de entorno

| Variable | Descripción | Por defecto |
|----------|-------------|-------------|
| `HIVE_C2_URL` | Base del C2 para el TaskPoller (`http://127.0.0.1:8444`) | *(sin C2: poller inactivo)* |
| `HIVE_C2_API_KEY` | Clave si el C2 exige `x-api-key` | *(sin auth)* |
| `HIVE_POLL_SECS` | Intervalo de sondeo de tareas (mín. 2 s) | `10` |
| `HIVE_LAB_AUTHORIZED` | `1` habilita la ejecución fileless en lab autorizado | *(bloqueado)* |

## 4. Verificación

```bash
./hive.sh test           # suite de hive_base
cargo test -p c2-server  # auth + rate-limit + CORS del C2
./tests/hive_colony_verify.sh   # verificación end-to-end de la colonia (lab)
```

La verificación de colonia valida el ciclo completo: agentes vivos en la
arena, consenso, telemetría y el ciclo de tareas del C2 con resultados
simulados etiquetados.
