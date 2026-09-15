#!/usr/bin/env bash
# Hive Colony v3.0 — Laboratorio de verificación
# Prepara el entorno de laboratorio: imagen base, binarios reales y targets
# SSH benignos (docker-compose.lab.yml en la raíz).
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()  { echo -e "${CYAN}[*]${NC} $*"; }
ok()    { echo -e "  ${GREEN}✓${NC} $*"; }
warn()  { echo -e "  ${YELLOW}⚠${NC} $*"; }
fail()  { echo -e "  ${RED}✗${NC} $*"; }

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
HIVE_BIN="${ROOT_DIR}/target/release"
COMPOSE="docker compose -f ${ROOT_DIR}/docker-compose.lab.yml"

# Verificar Docker
if ! command -v docker &>/dev/null; then
    fail "Docker no instalado"
    exit 1
fi
ok "Docker disponible"

# 1. Construir la imagen base (agents + c2 + tui) si no existe
info "Paso 1: Construyendo imagen base hive-colony..."
if ! docker image inspect hive-colony:latest &>/dev/null; then
    docker build -t hive-colony:latest -f "${ROOT_DIR}/Dockerfile" "${ROOT_DIR}"
    ok "Imagen hive-colony construida"
else
    ok "Imagen hive-colony ya existe"
fi

# 2. Verificar binarios compilados (los que existen de verdad desde ronda 6)
info "Paso 2: Verificando binarios..."
for bin in c2-server queen worker drone honeybee beekeeper; do
    if [ ! -f "${HIVE_BIN}/${bin}" ]; then
        fail "Binario faltante: ${HIVE_BIN}/${bin}"
        info "Ejecutá: cargo build --release -p c2-server -p beekeeper -p queen -p worker -p drone -p honeybee"
        exit 1
    fi
done
ok "Todos los binarios listos (c2-server, queen, worker, drone, honeybee, beekeeper)"

# 3. Iniciar los targets SSH del laboratorio (compose de la raíz)
info "Paso 3: Iniciando targets SSH del laboratorio..."
${COMPOSE} up -d 2>&1 | tail -3
ok "Servicios de laboratorio iniciados"

# 4. Esperar a que los targets SSH estén listos
info "Paso 4: Esperando targets SSH..."
for target in hive-victim1 hive-victim2; do
    for i in $(seq 1 15); do
        if docker exec "$target" nc -z 127.0.0.1 22 2>/dev/null; then
            ok "${target} SSH listo"
            break
        fi
        if [ "$i" -eq 15 ]; then
            fail "${target} no responde"
        fi
        sleep 1
    done
done

# 5. Verificar conectividad SSH desde el host (credenciales de laboratorio:
#    victim/victim123, definidas en docker-compose.lab.yml)
info "Paso 5: Verificando SSH desde el host..."
for target in hive-victim1 hive-victim2; do
    IP=$(docker inspect -f '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$target" 2>/dev/null)
    if command -v sshpass &>/dev/null; then
        if sshpass -p victim123 ssh -o StrictHostKeyChecking=no -o ConnectTimeout=3 \
                   -o PreferredAuthentications=password -o PubkeyAuthentication=no \
                   "victim@${IP}" "hostname" 2>/dev/null; then
            ok "SSH a ${target} (${IP}) funciona"
        else
            warn "SSH a ${target} (${IP}) falló — revisá las credenciales del lab"
        fi
    else
        warn "sshpass no instalado — salteando verificación SSH automática (${IP})"
    fi
done

# 6. Resumen
echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   LABORATORIO LISTO                          ║"
echo "╠══════════════════════════════════════════════╣"
echo "║  Targets SSH (credenciales de lab):          ║"
for target in hive-victim1 hive-victim2; do
    IP=$(docker inspect -f '{{range.NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$target" 2>/dev/null)
    echo "║    ${target}: ${IP}:22 (victim/victim123)"
done
echo "║                                              ║"
echo "║  C2:       ./hive.sh c2   (http://localhost:8444)"
echo "║  TUI:      ./hive.sh tui"
echo "║  Colonia:  ./hive.sh colony"
echo "║                                              ║"
echo "║  Políticas y capacidades: docs/CAPABILITIES.md"
echo "╚══════════════════════════════════════════════╝"
