#!/usr/bin/env bash
# Hive Colony Launcher
# Starts C2 server + all 6 agents with shared arena
# Usage: ./scripts/launch_colony.sh [--release|--debug] [--port PORT]

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "  ${GREEN}✓${NC} $*"; }
fail() { echo -e "  ${RED}✗${NC} $*"; }
warn() { echo -e "  ${YELLOW}⚠${NC} $*"; }

BUILD_MODE="--release"
CARGO_TARGET="release"
PORT=8444
ARENA_NAME="hive_colony"
LOOT_DIR="/tmp/hive_loot"
DB_PATH="/tmp/hive_c2.db"
PID_FILE="/tmp/hive_colony_pids.txt"

while [[ $# -gt 0 ]]; do case "$1" in
    --debug)   BUILD_MODE=""; CARGO_TARGET="debug"; shift ;;
    --port)    PORT="$2"; shift 2 ;;
    *) echo "Usage: $0 [--debug] [--port PORT]"; exit 1 ;;
esac; done

C2_BIN="target/${CARGO_TARGET}/c2-server"
AGENTS=(queen worker drone honeybee weaver swarm)

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
if [ ! -f "$C2_BIN" ]; then
    cargo build ${BUILD_MODE} -p c2-server 2>&1 | tail -1
fi
for agent in "${AGENTS[@]}"; do
    if [ ! -f "target/${CARGO_TARGET}/${agent}" ]; then
        cargo build ${BUILD_MODE} -p "$agent" 2>&1 | tail -1
    fi
done

log "Starting C2 server on port ${PORT}..."
rm -rf "$LOOT_DIR" "$DB_PATH"
mkdir -p "$LOOT_DIR"
export __HIVE_ARENA="${ARENA_NAME}"
export HIVE_LAB_MODE=1
export RUST_LOG=info
export HIVE_C2_URL="http://127.0.0.1:${PORT}/collect"
export HIVE_C2_DNS_DOMAIN="tunnel.example.com"
export HIVE_C2_ICMP_TARGET="127.0.0.1"

setsid "$C2_BIN" --port "$PORT" --loot-dir "$LOOT_DIR" --db-path "$DB_PATH" < /dev/null > /tmp/hive_c2.log 2>&1 &
C2_PID=$!
echo "$C2_PID" > "$PID_FILE"

for i in $(seq 1 10); do
    sleep 1
    if curl -sf "http://127.0.0.1:${PORT}/health" > /dev/null 2>&1; then
        ok "C2 server ready (PID ${C2_PID})"
        break
    fi
    if [ "$i" -eq 10 ]; then
        fail "C2 server failed to start"
        exit 1
    fi
done

log "Starting Hive agents..."
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
echo "║  C2:   http://127.0.0.1:${PORT}             ║"
echo "║  Arena: /dev/shm/${ARENA_NAME}             ║"
echo "║  Loot:  ${LOOT_DIR}                        ║"
echo "╠════════════════════════════════════════════╣"

ALL_OK=true
for agent in "${AGENTS[@]}" c2-server; do
    pid=$(pgrep -f "target/${CARGO_TARGET}/${agent}$" 2>/dev/null || true)
    if [ -n "$pid" ]; then
        ok "${agent} running (PID ${pid})"
    else
        fail "${agent} NOT running"
        ALL_OK=false
    fi
done

ROLES=$(tr '\n' ' ' < /tmp/hive_queen.log 2>/dev/null | grep -oP 'slot \d+, role: \w+' | tail -1 || echo "")
if [ -n "$ROLES" ]; then
    ok "Arena: ${ROLES}"
fi

echo "╚════════════════════════════════════════════╝"
echo "Logs: /tmp/hive_{agent}.log"
echo "Press Ctrl+C to stop the colony."

if [ "$ALL_OK" = true ]; then
    wait
else
    exit 1
fi
