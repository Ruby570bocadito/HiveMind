#!/bin/bash
# Hive Colony Test Harness — automated deploy + bug detection
# Usage: bash hive_test.sh

set -euo pipefail
cd "$(dirname "$0")"

GREEN='\033[0;32m' RED='\033[0;31m' CYAN='\033[0;36m' YELLOW='\033[1;33m' NC='\033[0m'
BUGS=0 FIXES=0

# ── Cleanup ───────────────────────────────────────────────────────────
cleanup() {
    echo -e "\n${CYAN}[cleanup] Stopping hive...${NC}"
    kill $WORKER_PID $DRONE_PID $HONEYBEE_PID $WEAVER_PID $C2_PID $DASH_PID 2>/dev/null
    fuser -k 8080/tcp 2>/dev/null || true; fuser -k 8445/tcp 2>/dev/null || true
    echo -e "${GREEN}Bugs found: $BUGS | Fixed: $FIXES${NC}"
    exit 0
}
trap cleanup INT TERM

# ── Clean ports ────────────────────────────────────────────────────────
fuser -k 8080/tcp 2>/dev/null || true; fuser -k 8445/tcp 2>/dev/null || true; sleep 1

# ── Generate shared arena ──────────────────────────────────────────────
ARENA_NAME="/hive_$(date +%s)_$(shuf -i 1000-9999 -n 1)"
export __HIVE_ARENA="$ARENA_NAME"

echo -e "${CYAN}╔══════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║   HIVE COLONY TEST HARNESS v1.0         ║${NC}"
echo -e "${CYAN}║   Arena: $ARENA_NAME${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════╝${NC}"
echo ""

# ── Phase 1: Launch infrastructure ─────────────────────────────────────
echo -e "${GREEN}[Phase 1] Infrastructure${NC}"

echo -n "  C2 Server :8445... "
python3 tests/c2_server.py --port 8445 --no-tls > /dev/null 2>&1 &
C2_PID=$!
sleep 2
if curl -s http://127.0.0.1:8445/health > /dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${RED}FAIL${NC}"; ((BUGS++))
fi

echo -n "  Dashboard :8080... "
python3 tests/dashboard.py --port 8080 > /dev/null 2>&1 &
DASH_PID=$!
sleep 2
if curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8080/ | grep -q 200; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${RED}FAIL${NC}"; ((BUGS++))
fi

# ── Phase 2: Launch colony agents ──────────────────────────────────────
echo -e "\n${GREEN}[Phase 2] Colony Agents${NC}"

echo -n "  Worker (arena creator)... "
target/debug/worker > /tmp/hive_worker.log 2>&1 &
WORKER_PID=$!
sleep 3
if kill -0 $WORKER_PID 2>/dev/null; then
    echo -e "${GREEN}OK (PID: $WORKER_PID)${NC}"
else
    echo -e "${RED}CRASHED${NC}"; ((BUGS++))
    tail -5 /tmp/hive_worker.log
fi

echo -n "  Drone (decision maker)... "
target/debug/drone > /tmp/hive_drone.log 2>&1 &
DRONE_PID=$!
sleep 2
if kill -0 $DRONE_PID 2>/dev/null; then
    echo -e "${GREEN}OK (PID: $DRONE_PID)${NC}"
else
    echo -e "${RED}CRASHED${NC}"; ((BUGS++))
    tail -5 /tmp/hive_drone.log
fi

echo -n "  Honeybee (executor)... "
target/debug/honeybee > /tmp/hive_honeybee.log 2>&1 &
HONEYBEE_PID=$!
sleep 2
if kill -0 $HONEYBEE_PID 2>/dev/null; then
    echo -e "${GREEN}OK (PID: $HONEYBEE_PID)${NC}"
else
    echo -e "${RED}CRASHED${NC}"; ((BUGS++))
    tail -5 /tmp/hive_honeybee.log
fi

echo -n "  Weaver (mutator)... "
target/debug/weaver > /tmp/hive_weaver.log 2>&1 &
WEAVER_PID=$!
sleep 2
if kill -0 $WEAVER_PID 2>/dev/null; then
    echo -e "${GREEN}OK (PID: $WEAVER_PID)${NC}"
else
    echo -e "${RED}CRASHED${NC}"; ((BUGS++))
    tail -5 /tmp/hive_weaver.log
fi

# ── Phase 3: Verify shared arena ───────────────────────────────────────
echo -e "\n${GREEN}[Phase 3] Arena Verification${NC}"

echo -n "  Checking arena slots... "
ARENA_SLOTS=$(grep -c "Connected to swarm arena" /tmp/hive_*.log 2>/dev/null || echo 0)
echo -e "${GREEN}$ARENA_SLOTS agents connected${NC}"

echo -n "  Same arena name? "
UNIQUE_ARENAS=$(grep "arena (shared memory)" /tmp/hive_*.log 2>/dev/null | wc -l)
if [ "$UNIQUE_ARENAS" -le 1 ]; then
    echo -e "${GREEN}YES (1 arena, $ARENA_SLOTS agents sharing)${NC}"
else
    echo -e "${RED}BUG: $UNIQUE_ARENAS separate arenas created (agents not sharing!)${NC}"
    ((BUGS++))
    echo -e "${YELLOW}  Fix: __HIVE_ARENA=$ARENA_NAME passed to all agents${NC}"
    echo -e "${YELLOW}  Check connect_to_arena() reads __HIVE_ARENA first${NC}"
fi

# ── Phase 4: Verify communication ──────────────────────────────────────
echo -e "\n${GREEN}[Phase 4] Colony Communication${NC}"

echo -n "  Worker published beliefs... "
grep -c "Published belief\|Belief:" /tmp/hive_worker.log 2>/dev/null || echo "0"
BELIEFS=$(grep -c "Published belief\|Belief:" /tmp/hive_worker.log 2>/dev/null || echo 0)
echo -e "${GREEN}${BELIEFS} beliefs${NC}"

echo -n "  Drone received beliefs... "
DRONE_BELIEFS=$(grep -c "Belief from\|belief" /tmp/hive_drone.log 2>/dev/null || echo 0)
echo -e "${GREEN}${DRONE_BELIEFS} received${NC}"
if [ "$DRONE_BELIEFS" -eq 0 ] && [ "$BELIEFS" -gt 0 ]; then
    echo -e "${RED}  BUG: Drone not receiving Worker beliefs (arena not shared?)${NC}"
    ((BUGS++))
fi

echo -n "  Drone regeneration activity... "
REGENS=$(grep -c "regenerat\|Fileless spawn" /tmp/hive_drone.log 2>/dev/null || echo 0)
echo -e "${GREEN}${REGENS} regeneration attempts${NC}"

echo -n "  Weaver mutations generated... "
MUTATIONS=$(grep -c "Variant:" /tmp/hive_weaver.log 2>/dev/null || echo 0)
echo -e "${GREEN}${MUTATIONS} variants${NC}"

# ── Phase 5: Dashboard & C2 Check ──────────────────────────────────────
echo -e "\n${GREEN}[Phase 5] External Interfaces${NC}"

echo -n "  Agent count in dashboard... "
AGENTS=$(curl -s http://127.0.0.1:8080/api/state 2>/dev/null | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('agents',[])))" 2>/dev/null || echo "0")
echo -e "${GREEN}${AGENTS} agents visible${NC}"

echo -n "  C2 beacons received... "
BEACONS=$(curl -s http://127.0.0.1:8445/logs 2>/dev/null | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('beacons',[])))" 2>/dev/null || echo "0")
echo -e "${GREEN}${BEACONS} beacons${NC}"

# ── Phase 6: Bug Summary ───────────────────────────────────────────────
echo -e "\n${CYAN}══════════════════════════════════════════${NC}"
echo -e "${CYAN}   BUG REPORT${NC}"
echo -e "${CYAN}══════════════════════════════════════════${NC}"

# Check for crashes
for agent in worker drone honeybee weaver; do
    log="/tmp/hive_${agent}.log"
    if grep -q "panicked\|SIGSEGV\|SIGABRT\|stack overflow" "$log" 2>/dev/null; then
        echo -e "${RED}  CRASH: $agent — $(grep 'panicked\|Error' $log | tail -1)${NC}"
        ((BUGS++))
    fi
done

# Check arena sharing
if grep -q "Connected to swarm arena (slot 1" /tmp/hive_drone.log 2>/dev/null; then
    echo -e "${GREEN}  PASS: Agents sharing arena (multi-slot)${NC}"
elif grep -q "Connected to swarm arena (slot 0" /tmp/hive_*.log 2>/dev/null | wc -l | grep -q "$ARENA_SLOTS"; then
    echo -e "${YELLOW}  WARN: All agents on slot 0 (separate arenas)${NC}"
    ((BUGS++))
fi

# Check fileless execution
if grep -q "Fileless spawn" /tmp/hive_drone.log 2>/dev/null; then
    echo -e "${GREEN}  PASS: Fileless regeneration working${NC}"
else
    echo -e "${YELLOW}  WARN: No regeneration attempts${NC}"
fi

# Final status
echo -e "\n${GREEN}Bugs: $BUGS | Agents alive: $(jobs -p | wc -l)${NC}"
echo -e "${CYAN}══════════════════════════════════════════${NC}"
echo ""
echo "Logs: /tmp/hive_*.log"
echo "Dashboard: http://localhost:8080"
echo "C2: http://localhost:8445/health"
echo "Press Ctrl+C to stop"

# Keep running + periodic checks
while true; do
    sleep 10
    ALIVE=$(jobs -p | wc -l)
    REGENS=$(grep -c "Fileless spawn" /tmp/hive_drone.log 2>/dev/null || echo 0)
    echo -e "[$(date +%H:%M:%S)] Agents: $ALIVE | Regenerations: $REGENS | Arena: $ARENA_NAME"
done
