#!/bin/bash
# Hive Colony v3.0 — Colony Lifecycle Orchestrator (ronda 6)
# Ejecuta el ciclo de vida completo de la colonia en un laboratorio propio:
# lanzamiento, reconocimiento, consenso y C2. Sin módulos ofensivos ni
# simulados (eliminados en ronda 6).
#
# Uso:
#   ./scripts/scenario.sh [--target <ip>] [--quick] [--cleanup] [--report]
#
# Flags:
#   --target   IP del lab (default: 127.0.0.1)
#   --quick    Ejecuta todas las fases sin pausas
#   --cleanup  Limpia rastros del lab
#   --report   Muestra el reporte Markdown al final

set -euo pipefail
trap 'echo "[!] Escenario interrumpido en línea $LINENO"; exit 1' ERR

TARGET="${2:-127.0.0.1}"
QUICK=false
CLEANUP=false
REPORT=false
ARENA="hive_campaign_$(date +%s)"
OUT_DIR="./loot/campaign_$(date +%Y%m%d_%H%M%S)"
HIVE_BIN="./target/release"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; NC='\033[0m'

info()  { echo -e "${CYAN}[*]${NC} $1"; }
ok()    { echo -e "${GREEN}[+]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
err()   { echo -e "${RED}[x]${NC} $1"; }

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --target) TARGET="$2"; shift 2 ;;
            --quick)  QUICK=true; shift ;;
            --cleanup) CLEANUP=true; shift ;;
            --report) REPORT=true; shift ;;
            *) err "Argumento desconocido: $1"; exit 1 ;;
        esac
    done
}

phase_sleep() {
    $QUICK && return
    local secs="${1:-5}"
    info "Esperando ${secs}s..."
    sleep "$secs"
}

phase_prepare() {
    info "=== FASE 0: Preparación del entorno ==="
    for bin in worker drone honeybee queen beekeeper; do
        if [[ ! -f "$HIVE_BIN/$bin" ]]; then
            err "Binario no encontrado: $HIVE_BIN/$bin"
            info "Ejecuta: cargo build --release -p $bin"
            exit 1
        fi
    done
    ok "Todos los binarios presentes"
    mkdir -p "$OUT_DIR"
    export __HIVE_ARENA="$ARENA"
    export HIVE_C2_URL="http://localhost:8443/beacon"

    if ! pgrep -f "c2_server.py" >/dev/null 2>&1; then
        python3 tests/c2_server.py --port 8443 &
        sleep 2
        ok "C2 de referencia iniciado en puerto 8443"
    fi

    ok "Entorno preparado — Arena: $ARENA | Lab: $TARGET"
}

phase_launch() {
    info "=== FASE 1: Lanzamiento de la colonia ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/worker" &
    phase_sleep 2
    "$HIVE_BIN/drone" &
    phase_sleep 2
    echo "[FASE1] worker+drone lanzados" >> "$OUT_DIR/campaign.log"
    ok "Worker y Drone activos sobre la arena IPC"
}

phase_recon() {
    info "=== FASE 2: Reconocimiento (solo lectura) ==="
    export __HIVE_ARENA="$ARENA"
    phase_sleep 5
    echo "[FASE2] system profile + propuestas" >> "$OUT_DIR/campaign.log"
    ok "Worker publica perfil del sistema; Drone propone acciones internas"
}

phase_consensus() {
    info "=== FASE 3: Consenso + C2 ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/honeybee" &
    phase_sleep 3
    "$HIVE_BIN/queen" &
    phase_sleep 5
    echo "[FASE3] honeybee+queen activas" >> "$OUT_DIR/campaign.log"
    ok "Queen activa — HiveMind consenso + TaskPoller + shell del operador"
}

phase_report() {
    info "=== FASE 4: Reporte ==="

    cat > "$OUT_DIR/reporte_campana.md" << REOF
# Reporte Colonia Hive Colony v3.0 (ronda 6)

**Fecha:** $(date)
**Lab:** $TARGET
**Arena:** $ARENA

## Fases
| Fase | Componente | Estado |
|------|------------|--------|
| 1. Lanzamiento | worker + drone (arena IPC) | ✅ |
| 2. Reconocimiento | system profile (solo lectura) | ✅ |
| 3. Consenso | HiveMind + Tournament | ✅ |
| 3. C2 | TaskPoller + beacons + shell operador | ✅ |
| 4. Transporte | WhisperNet P2P + failover multi-canal | ✅ |

## Nota
El build no contiene módulos ofensivos ni simulados (ronda 6):
exploits, exfiltración, sabotaje, recolección de credenciales,
persistencia y evasión fueron eliminados del repositorio.
REOF
    echo "[FASE4] Reporte generado" >> "$OUT_DIR/campaign.log"
    ok "Reporte: $OUT_DIR/reporte_campana.md"
}

cleanup() {
    info "=== CLEANUP ==="
    for proc in worker drone honeybee queen beekeeper; do
        pkill -f "$HIVE_BIN/$proc" 2>/dev/null || true
    done
    pkill -f "c2_server.py" 2>/dev/null || true
    rm -rf /dev/shm/colmena_* /dev/shm/hive_* /dev/shm/.hive_* 2>/dev/null || true
    ok "Cleanup completado"
}

main() {
    parse_args "$@"
    echo -e "${CYAN}╔════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║  Hive Colony v3.0 — Colony Lab     ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════╝${NC}"

    if $CLEANUP; then cleanup; exit 0; fi

    phase_prepare;  phase_sleep 2
    phase_launch; phase_sleep 3
    phase_recon;    phase_sleep 3
    phase_consensus; phase_sleep 3
    phase_report

    echo -e "${GREEN}CICLO COMPLETADO — Reporte: $OUT_DIR/reporte_campana.md${NC}"
    $REPORT && cat "$OUT_DIR/reporte_campana.md"
}

main "$@"
