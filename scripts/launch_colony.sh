#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Hive Colony — local launcher (bare metal, sin Docker)
#
# Ronda 11: reescrito. El anterior era inejecutable desde la ronda 6:
# compilaba el agente `swarm` (eliminado), arrancaba el C2 con `--loot-dir`
# (flag retirada → crash) y exportaba HIVE_C2_ICMP_TARGET (env var sin
# lector) junto a un LOOT_DIR sin consumidor.
#
# Usage:
#   ./scripts/launch_colony.sh [--release|--debug] [--port PORT]
#
# Para el stack completo en Docker usa ./hive.sh colony (docker-compose.yml).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

RED='\033[91m'; GREEN='\033[92m'; YELLOW='\033[93m'; CYAN='\033[96m'; NC='\033[0m'
log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "  ${GREEN}✓${NC} $*"; }
fail() { echo -e "  ${RED}✗${NC} $*"; }
warn() { echo -e "  ${YELLOW}⚠${NC} $*"; }

BUILD_MODE="--release"
CARGO_TARGET="release"
PORT=8444
ARENA_NAME="hive_colony"
DB_PATH="/tmp/hive_c2.db"
PID_FILE="/tmp/hive_colony_pids.txt"

# Ronda 11: los 4 agentes reales (swarm fue eliminado en la ronda 6).
AGENTS=(queen worker drone honeybee)

while [[ $# -gt 0 ]]; do case "$1" in
    --release) BUILD_MODE="--release"; CARGO_TARGET="release"; shift ;;
    --debug)   BUILD_MODE=""; CARGO_TARGET="debug"; shift ;;
    --port)    PORT="$2"; shift 2 ;;
    *) echo "Usage: $0 [--release|--debug] [--port PORT]"; exit 1 ;;
esac; done

C2_BIN="target/${CARGO_TARGET}/c2-server"

cleanup() {
    echo
    log "Shutting down Hive Colony..."
    if [ -f "$PID_FILE" ]; then
        while read -r pid; do kill "$pid" 2>/dev/null || true; done < "$PID_FILE"
        rm -f "$PID_FILE"
    fi
    pkill -f "c2-server" 2>/dev/null || true
    rm -f "/dev/shm/${ARENA_NAME}"
    log "All agents stopped."
    exit 0
}
trap cleanup SIGINT SIGTERM EXIT

log "Building Hive Colony (${CARGO_TARGET})..."
cargo build ${BUILD_MODE} -p c2-server -p queen -p worker -p drone -p honeybee 2>&1 | tail -1

log "Starting C2 server on port ${PORT}..."
rm -f "$DB_PATH"
export __HIVE_ARENA="${ARENA_NAME}"
export HIVE_LAB_MODE=1
export RUST_LOG=info
# Ronda 11: BASE del C2 sin sufijo (el TaskPoller añade /task/{id} y /beacon;
# con el histórico …/beacon el poller consultaba /beacon/task/… → 404).
export HIVE_C2_URL="http://127.0.0.1:${PORT}"

setsid "$C2_BIN" --port "$PORT" --db-path "$DB_PATH" < /dev/null > /tmp/hive_c2.log 2>&1 &
C2_PID=$!
echo "$C2_PID" > "$PID_FILE"

for i in $(seq 1 10); do
    sleep 1
    if curl -sf "http://127.0.0.1:${PORT}/health" > /dev/null 2>&1; then
        ok "C2 server ready (PID ${C2_PID})"
        break
    fi
    if [ "$i" -eq 10 ]; then
        fail "C2 server failed to start (see /tmp/hive_c2.log)"
        exit 1
    fi
done

log "Starting agents (${AGENTS[*]})..."
for agent in "${AGENTS[@]}"; do
    setsid "target/${CARGO_TARGET}/${agent}" < /dev/null > "/tmp/hive_${agent}.log" 2>&1 &
    AGENT_PID=$!
    echo "$AGENT_PID" >> "$PID_FILE"
    sleep 0.5
done

sleep 2

echo "╔════════════════════════════════════════════╗"
echo "║         HIVE COLONY STATUS                 ║"
echo "╠════════════════════════════════════════════╣"
echo "║  C2:    http://127.0.0.1:${PORT}            ║"
echo "║  Arena: /dev/shm/${ARENA_NAME}             ║"
echo "║  TUI:   ./hive.sh tui (otra terminal)      ║"
echo "╠════════════════════════════════════════════╣"

ALL_OK=true
for proc in "${AGENTS[@]}" c2-server; do
    pid=$(pgrep -f "target/${CARGO_TARGET}/${proc}$" 2>/dev/null || true)
    if [ -n "$pid" ]; then
        ok "${proc} running (PID ${pid})"
    else
        fail "${proc} NOT running"
        ALL_OK=false
    fi
done

ROLES=$(tr '\n' ' ' < /tmp/hive_queen.log 2>/dev/null | grep -oP 'slot \d+, role: \w+' | tail -1 || echo "")
if [ -n "$ROLES" ]; then
    ok "Arena: ${ROLES}"
fi

echo "╚════════════════════════════════════════════╝"
echo "Logs: /tmp/hive_{queen,worker,drone,honeybee,c2}.log"
echo "Press Ctrl+C to stop the colony."

if [ "$ALL_OK" = true ]; then
    wait
fi
