#!/bin/bash
# EDR Gauntlet - Automated stealth validation pipeline.
# Runs a battery of tests to verify swarm evasion properties.
# Designed for CI/CD integration and manual lab testing.
#
# Usage: bash edr_gauntlet.sh [--watch] [--report report.json]
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - OPENSSL_DIR, OPENSSL_LIB_DIR, OPENSSL_INCLUDE_DIR env vars
#   - Python 3 (for detection monitor)

set +e
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

PASS=0
FAIL=0
WARN=0
REPORT_FILE=""
WATCH_MODE=false
TIMESTAMP=$(date -Iseconds)

# ── Helpers ───────────────────────────────────────────────────────────────────

log_section() { echo -e "\n${BOLD}${CYAN}=== $1 ===${NC}\n"; }
log_pass()   { echo -e "  ${GREEN}[PASS]${NC} $1"; ((PASS++)); }
log_fail()   { echo -e "  ${RED}[FAIL]${NC} $1"; ((FAIL++)); }
log_warn()   { echo -e "  ${YELLOW}[WARN]${NC} $1"; ((WARN++)); }
log_info()   { echo -e "  ${CYAN}[INFO]${NC} $1"; }

# ── Parse args ────────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --report) REPORT_FILE="$2"; shift 2 ;;
        --watch)  WATCH_MODE=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── Env setup ─────────────────────────────────────────────────────────────────

export OPENSSL_DIR="${OPENSSL_DIR:-/usr}"
export OPENSSL_LIB_DIR="${OPENSSL_LIB_DIR:-/usr/lib/x86_64-linux-gnu}"
export OPENSSL_INCLUDE_DIR="${OPENSSL_INCLUDE_DIR:-/usr/include}"

REPORT_JSON='{"timestamp":"'"$TIMESTAMP"'","results":{}}'

add_result() {
    local test="$1" result="$2" detail="$3"
    REPORT_JSON=$(echo "$REPORT_JSON" | jq --arg t "$test" --arg r "$result" --arg d "$detail" \
        '.results[$t] = {"result":$r,"detail":$d}')
}

# ── Test Suite ────────────────────────────────────────────────────────────────

log_section "1. BUILD INTEGRITY"

log_info "Building workspace..."
cargo build --workspace 2>&1 > /tmp/gauntlet_build.log
if tail -1 /tmp/gauntlet_build.log | grep -q "Finished"; then
    log_pass "Workspace compiles cleanly"
    add_result "build" "pass" "workspace compiles"
else
    log_fail "Workspace build failed"
    add_result "build" "fail" "compilation errors"
fi

log_info "Running unit tests..."
cargo test --lib -p hive_base 2>&1 > /tmp/gauntlet_test.log
if grep -q "test result: ok" /tmp/gauntlet_test.log; then
    log_pass "Unit tests pass"
    add_result "unit_tests" "pass" "all unit tests green"
else
    log_fail "Unit tests failed"
    add_result "unit_tests" "fail" "test failures"
fi

log_section "2. TCP PORT AUDIT"

log_info "Scanning for open TCP ports on loopback..."
TCP_PORTS=(4242 8080 9000 5555 1337 31337 4444 8888)
for port in "${TCP_PORTS[@]}"; do
    if timeout 0.5 bash -c "echo >/dev/tcp/127.0.0.1/$port" 2>/dev/null; then
        log_fail "Port $port is OPEN (bus leak!)"
        add_result "tcp_$port" "fail" "port open"
    else
        log_pass "Port $port closed"
        add_result "tcp_$port" "pass" "port closed"
    fi
done

log_section "3. BINARY FORENSICS"

SWARMCTL_BIN="target/release/beekeeper"
if [[ -f "$SWARMCTL_BIN" ]]; then
    log_info "Analyzing $SWARMCTL_BIN for indicators..."

    # Check for TCP bus address
    if strings "$SWARMCTL_BIN" | grep -q "127.0.0.1:4242"; then
        log_warn "Bus address string found in binary (legacy constant)"
        add_result "binary_bus_str" "warn" "legacy string found"
    else
        log_pass "No bus address in binary"
        add_result "binary_bus_str" "pass" "clean"
    fi

    # Check for ONNX magic
    if strings "$SWARMCTL_BIN" | grep -q "ONNX"; then
        log_info "ONNX strings found (expected in model-using bins)"
        add_result "binary_onnx" "info" "ONNX strings present"
    else
        log_pass "No raw ONNX signatures"
        add_result "binary_onnx" "pass" "clean"
    fi

    # Check entropy (high entropy = encrypted/packed = good)
    ENTROPY=$(ent "$SWARMCTL_BIN" 2>/dev/null | grep "Entropy" | awk '{print $3}' || echo "0")
    if [[ -n "$ENTROPY" ]]; then
        log_info "Binary entropy: $ENTROPY bits/byte"
        add_result "binary_entropy" "info" "$ENTROPY"
    fi
else
    log_warn "swarmctl binary not found (build first)"
    add_result "binary_analysis" "skip" "binary not found"
fi

log_section "4. SHARED MEMORY AUDIT"

if [[ -d "/dev/shm" ]]; then
    SWARM_FILES=$(find /dev/shm -name "swarm_*" 2>/dev/null | wc -l)
    if [[ $SWARM_FILES -gt 0 ]]; then
        log_info "$SWARM_FILES swarm_ files in /dev/shm (arena active)"
        add_result "shm_arena" "info" "$SWARM_FILES files found"
    else
        log_pass "No persistent shared memory artifacts"
        add_result "shm_arena" "pass" "no artifacts"
    fi
else
    log_info "/dev/shm not available"
    add_result "shm_arena" "skip" "platform not supported"
fi

log_section "5. PROCESS INSPECTION"

# Check for running swarm agents
if pgrep -f "worker|drone|honeybee|weaver|queen" > /dev/null 2>&1; then
    AGENT_COUNT=$(pgrep -fc "worker|drone|honeybee|weaver|queen" || echo 0)
    log_info "$AGENT_COUNT swarm agent(s) running"
    add_result "agents_running" "info" "$AGENT_COUNT agents"
else
    log_info "No swarm agents running (idle test)"
    add_result "agents_running" "info" "idle"
fi

# Check for memfd in agent processes
if pgrep -f "worker|drone" > /dev/null 2>&1; then
    for pid in $(pgrep -f "worker|drone|honeybee|weaver|queen"); do
        MEMFD_COUNT=$(ls -la /proc/$pid/fd 2>/dev/null | grep -c "memfd:" || echo 0)
        if [[ $MEMFD_COUNT -gt 0 ]]; then
            log_pass "PID $pid: $MEMFD_COUNT memfd(s) - fileless confirmed"
            add_result "memfd_$pid" "pass" "$MEMFD_COUNT memfds"
        fi
    done
fi

log_section "6. ANTI-ANALYSIS CHECKS"

# Check if being traced
if [[ -f "/proc/self/status" ]]; then
    TRACER=$(grep "TracerPid:" /proc/self/status | awk '{print $2}')
    if [[ "$TRACER" != "0" ]]; then
        log_fail "Process is being traced (PID: $TRACER)!"
        add_result "traced" "fail" "tracer PID $TRACER"
    else
        log_pass "Not being traced"
        add_result "traced" "pass" "no tracer"
    fi
fi

# Check VM indicators
if [[ -f "/sys/class/dmi/id/product_name" ]]; then
    PRODUCT=$(cat /sys/class/dmi/id/product_name 2>/dev/null || echo "unknown")
    if echo "$PRODUCT" | grep -qi "virtualbox\|vmware\|qemu\|kvm"; then
        log_info "Running in VM: $PRODUCT (expected for lab)"
        add_result "vm_detected" "info" "$PRODUCT"
    else
        log_info "Bare metal or unknown: $PRODUCT"
        add_result "vm_detected" "info" "$PRODUCT"
    fi
fi

log_section "7. CODE QUALITY"

log_info "Running clippy..."
cargo clippy --workspace -- -D warnings 2>&1 > /tmp/gauntlet_clippy.log
if tail -1 /tmp/gauntlet_clippy.log | grep -q "0 errors"; then
    log_pass "Clippy: no warnings"
    add_result "clippy" "pass" "clean"
else
    log_warn "Clippy: warnings present"
    add_result "clippy" "warn" "warnings found"
fi

log_info "Checking for unsafe code..."
UNSAFE_COUNT=$(grep -r "unsafe" hive_base/src/ --include="*.rs" | wc -l)
if [[ $UNSAFE_COUNT -gt 0 ]]; then
    log_info "$UNSAFE_COUNT unsafe blocks (expected for syscalls/memfd)"
    add_result "unsafe_blocks" "info" "$UNSAFE_COUNT blocks"
else
    log_pass "No unsafe code"
    add_result "unsafe_blocks" "pass" "clean"
fi

# ── Summary ───────────────────────────────────────────────────────────────────

log_section "RESULTS SUMMARY"

TOTAL=$((PASS + FAIL + WARN))
echo -e "  Tests:  ${BOLD}$TOTAL${NC} total"
echo -e "  Passed: ${GREEN}$PASS${NC}"
echo -e "  Failed: ${RED}$FAIL${NC}"
echo -e "  Warnings/Info: ${YELLOW}$WARN${NC}"

if [[ $FAIL -gt 0 ]]; then
    echo -e "\n  ${RED}${BOLD}GAUNTLET FAILED${NC} - $FAIL detection surface(s) exposed!"
    EXIT_CODE=1
else
    echo -e "\n  ${GREEN}${BOLD}GAUNTLET PASSED${NC} - Stealth properties verified"
    EXIT_CODE=0
fi

# ── Report ────────────────────────────────────────────────────────────────────

if [[ -n "$REPORT_FILE" ]]; then
    echo "$REPORT_JSON" | jq '.' > "$REPORT_FILE"
    echo -e "\nReport saved to: $REPORT_FILE"
fi

# ── Watch mode ────────────────────────────────────────────────────────────────

if $WATCH_MODE; then
    echo -e "\n${CYAN}Entering watch mode (Ctrl+C to stop)...${NC}"
    echo "Monitoring for EDR detections every 10s..."
    while true; do
        sleep 10
        ALERTS=0
        # Re-check TCP ports
        for port in "${TCP_PORTS[@]}"; do
            if timeout 0.3 bash -c "echo >/dev/tcp/127.0.0.1/$port" 2>/dev/null; then
                echo -e "  ${RED}[$(date +%H:%M:%S)] ALERT: Port $port opened!${NC}"
                ((ALERTS++))
            fi
        done
        if [[ $ALERTS -eq 0 ]]; then
            echo -e "  ${GREEN}[$(date +%H:%M:%S)] Clean scan${NC}"
        fi
    done
fi

exit $EXIT_CODE
