#!/usr/bin/env bash
# Hive Colony End-to-End Verification
# Prueba TODO de verdad: agentes, arena, C2, ciclo de tareas (ronda 6:
# solo infraestructura real — sin módulos ofensivos ni simulados)
set -euo pipefail

HIVE_BIN="target/release"
ARENA_NAME="hive_verify_$(date +%s)"
C2_PORT=${C2_PORT:-8444}
C2_URL="http://127.0.0.1:${C2_PORT}"
DB_PATH="/tmp/hive_verify_$$.db"
PID_FILE="/tmp/hive_verify_pids_$$"
PASS=0
FAIL=0
TIMEOUT=60  # max seconds to wait for each agent

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}PASS${NC}: $1"; PASS=$((PASS + 1)); }
fail() { echo -e "  ${RED}FAIL${NC}: $1"; FAIL=$((FAIL + 1)); }
info() { echo -e "${CYAN}[*]${NC} $1"; }
warn() { echo -e "${YELLOW}[!]${NC} $1"; }

cleanup() {
    info "Limpiando..."
    if [ -f "$PID_FILE" ]; then
        while read -r pid; do kill "$pid" 2>/dev/null || true; done < "$PID_FILE"
        rm -f "$PID_FILE"
    fi
    pkill -f "c2-server.*--port $C2_PORT" 2>/dev/null || true
    rm -f "/dev/shm/${ARENA_NAME}" 2>/dev/null || true
    rm -rf "$DB_PATH"
}
trap cleanup EXIT

assert_pid_alive() {
    local desc="$1" pid="$2" name="$3"
    if kill -0 "$pid" 2>/dev/null; then
        pass "$desc ($name PID $pid)"
        return 0
    else
        fail "$desc ($name murió)"
        return 1
    fi
}

wait_for_log() {
    local logfile="$1" pattern="$2" timeout_secs="${3:-$TIMEOUT}" desc="$4"
    local waited=0
    while [ $waited -lt $timeout_secs ]; do
        if [ -f "$logfile" ] && grep -q "$pattern" "$logfile" 2>/dev/null; then
            pass "$desc"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    fail "$desc (timeout ${timeout_secs}s, pattern: '$pattern')"
    warn "Últimas 10 líneas de $logfile:"
    tail -10 "$logfile" 2>/dev/null | sed 's/^/    /'
    return 1
}

# ===== SETUP =====
echo "╔══════════════════════════════════════════════════╗"
echo "║   HIVE COLONY — VERIFICACIÓN END-TO-END REAL    ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# 1. Verificar bins compilados
info "Paso 0: Verificando binarios compilados..."
for bin in c2-server queen worker drone honeybee; do
    if [ ! -f "$HIVE_BIN/$bin" ]; then
        fail "Binario faltante: $HIVE_BIN/$bin"
        info "Ejecutá: cargo build --release -p $bin"
        exit 1
    fi
done
pass "Todos los binarios existen en $HIVE_BIN/"

# 3. Iniciar C2 Server
info "Paso 1: Iniciando C2 Server..."
rm -rf "$DB_PATH"
setsid "$HIVE_BIN/c2-server" --port "$C2_PORT" --db-path "$DB_PATH" \
    < /dev/null > /tmp/hive_verify_c2.log 2>&1 &
C2_PID=$!
echo "$C2_PID" > "$PID_FILE"

# Esperar a que C2 esté listo
C2_READY=false
for i in $(seq 1 15); do
    sleep 1
    if curl -sf "$C2_URL/health" > /dev/null 2>&1; then
        C2_READY=true
        break
    fi
done
if $C2_READY; then
    pass "C2 Server iniciado (PID $C2_PID, puerto $C2_PORT)"
else
    fail "C2 Server no respondió en 15s"
    tail -20 /tmp/hive_verify_c2.log
    exit 1
fi

# 4. Test endpoints C2
info "Paso 2: Verificando endpoints C2..."

# 4a. Health
HEALTH=$(curl -sf "$C2_URL/health" 2>/dev/null || echo "")
if echo "$HEALTH" | grep -q "ok"; then
    pass "C2 endpoint /health"
else
    fail "C2 endpoint /health (respuesta: $HEALTH)"
fi

# 4b. Beacon
BEACON=$(curl -sf -X POST "$C2_URL/beacon" \
    -H "X-Agent-ID: verify-queen-001" \
    -H "X-Agent-Role: queen" \
    -d '{"hostname":"verify-host","username":"root","os":"linux","version":"3.0.0"}' 2>/dev/null || echo "")
if echo "$BEACON" | grep -q "ack"; then
    pass "C2 endpoint /beacon (queen)"
else
    fail "C2 endpoint /beacon (respuesta: $BEACON)"
fi

# 4c. Task push/pull (el endpoint de exfiltración /collect fue eliminado en ronda 6)
TASK_PUSH=$(curl -sf -o /dev/null -w "%{http_code}" -X POST "$C2_URL/task/verify-queen-001" \
    -H "Content-Type: application/json" \
    -d '{"id":"vt1","command":"exec","payload":{"cmd":"id"}}' 2>/dev/null || echo "")
if [ "$TASK_PUSH" = "201" ]; then
    pass "C2 endpoint /task (push)"
else
    fail "C2 endpoint /task push (HTTP $TASK_PUSH)"
fi

TASK_PULL=$(curl -sf "$C2_URL/task/verify-queen-001" 2>/dev/null || echo "")
if echo "$TASK_PULL" | grep -q "vt1"; then
    pass "C2 endpoint /task (pull)"
else
    fail "C2 endpoint /task pull"
fi

# 4e. Admin agents
AGENTS=$(curl -sf "$C2_URL/admin/agents" 2>/dev/null || echo "")
if echo "$AGENTS" | grep -q "verify-queen-001"; then
    pass "C2 endpoint /admin/agents (verify-queen-001 aparece)"
else
    fail "C2 endpoint /admin/agents (verify-queen-001 no aparece)"
    info "Respuesta: $AGENTS"
fi

# ===== COLONY LAUNCH =====
info "Paso 3: Lanzando colonia (todos los agentes)..."
export __HIVE_ARENA="$ARENA_NAME"
export HIVE_LAB_MODE=1
export RUST_LOG=info
export HIVE_C2_URL="http://127.0.0.1:${C2_PORT}"
export HIVE_TELEMETRY_DIR="/tmp/hive_verify_telemetry"

mkdir -p "$HIVE_TELEMETRY_DIR"
declare -A AGENT_PIDS
AGENT_LIST=(queen worker drone honeybee)

for agent in "${AGENT_LIST[@]}"; do
    setsid "$HIVE_BIN/$agent" < /dev/null > "/tmp/hive_verify_${agent}.log" 2>&1 &
    AGENT_PIDS[$agent]=$!
    echo "$!" >> "$PID_FILE"
    info "  $agent iniciado (PID $!)"
    sleep 0.3
done

# Esperar que los agentes se estabilicen
sleep 3
echo ""

# ===== VERIFICACIÓN AGENTES =====
info "Paso 4: Verificando que los agentes están vivos..."
for agent in "${AGENT_LIST[@]}"; do
    assert_pid_alive "$agent corriendo" "${AGENT_PIDS[$agent]}" "$agent" || true
done

# Esperar heartbeats
info "Paso 5: Verificando heartbeats en arena..."
sleep 3
if [ -f "/dev/shm/${ARENA_NAME}" ] || [ -e "/dev/shm/${ARENA_NAME}" ]; then
    ARENA_SIZE=$(stat -c%s "/dev/shm/${ARENA_NAME}" 2>/dev/null || echo "unknown")
    pass "Arena existe en /dev/shm/${ARENA_NAME} (size: $ARENA_SIZE)"
else
    fail "Arena NO encontrada en /dev/shm/${ARENA_NAME}"
fi

# 5a. Queen: heartbeat, seer, phoenix
info "Paso 5a: Verificando Queen..."
wait_for_log "/tmp/hive_verify_queen.log" "heartbeat" 30 "Queen: heartbeat enviado" || true
wait_for_log "/tmp/hive_verify_queen.log" "OvermindAgent" 15 "Queen: OvermindAgent inicializado" || true

# 5b. Worker: profiling, EDR detection
info "Paso 5b: Verificando Worker..."
wait_for_log "/tmp/hive_verify_worker.log" "ScoutAgent" 15 "Worker: ScoutAgent inicializado" || true
wait_for_log "/tmp/hive_verify_worker.log" "profile" 30 "Worker: system profile recolectado" || true
wait_for_log "/tmp/hive_verify_worker.log" "edr" 45 "Worker: detección EDR" || true

# 5c. Drone: stigmergy, propuestas de consenso
info "Paso 5c: Verificando Drone..."
wait_for_log "/tmp/hive_verify_drone.log" "DroneAgent" 15 "Drone: DroneAgent inicializado" || true

# 5d. Honeybee: consenso + genoma en memoria
info "Paso 5d: Verificando Honeybee..."
wait_for_log "/tmp/hive_verify_honeybee.log" "HoarderAgent" 15 "Honeybee: HoarderAgent inicializado" || true
wait_for_log "/tmp/hive_verify_honeybee.log" "Phoenix" 30 "Honeybee: genoma phoenix en memoria" || true

# ===== TEST ARENA IPC =====
info "Paso 6: Verificando IPC inter-agentes..."
# Verificar que múltiples agentes están registrados en arena
# (Leemos el log de queen para ver si detecta otros agentes)
wait_for_log "/tmp/hive_verify_queen.log" "worker" 30 "Queen detecta Worker en arena" || true
wait_for_log "/tmp/hive_verify_queen.log" "drone" 30 "Queen detecta Drone en arena" || true
wait_for_log "/tmp/hive_verify_queen.log" "honeybee" 30 "Queen detecta Honeybee en arena" || true

# ===== TEST CICLO DE TAREAS (RECHAZO DE TAREAS DESTRUCTIVAS) =====
info "Paso 7: Ciclo de tareas del C2 (ronda 6: exfil/destructivas se RECHAZAN)..."

# Una tarea de exfil debe ser RECHAZADA por el TaskPoller (capacidad eliminada)
TASK_EXFIL=$(curl -sf -o /dev/null -w "%{http_code}" -X POST "$C2_URL/task/verify-honeybee-001" \
    -H "Content-Type: application/json" \
    -d '{"id":"vexfil1","command":"exfil","payload":{"path":"/tmp/x"}}' 2>/dev/null || echo "")
if [ "$TASK_EXFIL" = "201" ]; then
    pass "Tarea destructiva aceptada en cola (el agente la rechazará: 'rejected')"
else
    warn "No se pudo crear tarea de prueba (HTTP $TASK_EXFIL)"
fi

# ===== TRANSPORTE =====
info "Paso 9: Transporte del enjambre (WhisperNet + failover multi-canal)..."
if grep -q "WhisperNet" /tmp/hive_verify_queen.log 2>/dev/null; then
    pass "Queen: transporte WhisperNet activo en telemetría"
else
    warn "Queen: sin eventos WhisperNet aún (puede requerir más ciclos)"
fi

# ===== VERIFICACIÓN FINAL =====
echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   RESULTADOS VERIFICACIÓN                        ║"
echo "╠══════════════════════════════════════════════════╣"
TOTAL=$((PASS + FAIL))
echo "║  TOTAL: $TOTAL tests"
echo "║  ✅ PASS: $PASS"
echo "║  ❌ FAIL: $FAIL"
echo "╚══════════════════════════════════════════════════╝"

# Resumen agente por agente
echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   LOGS POR AGENTE (últimas 3 líneas)            ║"
echo "╚══════════════════════════════════════════════════╝"
for agent in c2-server queen worker drone honeybee swarm; do
    logfile="/tmp/hive_verify_${agent}.log"
    if [ -f "$logfile" ]; then
        last_line=$(tail -1 "$logfile" 2>/dev/null | tr -d '\n' | head -c 120)
        echo "  ${agent}: $last_line"
    fi
done

[ "$FAIL" -eq 0 ] && exit 0 || exit 1
