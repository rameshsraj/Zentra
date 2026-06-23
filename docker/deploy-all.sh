#!/usr/bin/env bash
# =============================================================================
# Zentra - Full Deployment Script (Linux)
# Runs both the Zentra application installer and the datacenter simulation
# setup in sequence, then displays a combined summary.
#
# Usage:
#   chmod +x deploy-all.sh
#   sudo ./deploy-all.sh
#
# What this does:
#   1. Installs/updates Zentra (Docker container with Oracle client)
#   2. Deploys the datacenter simulation (12 simulated nodes)
#   3. Displays a combined summary of all services
#
# Prerequisites: Docker must be installed and running.
# =============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Get the first valid IP for a container (handles multiple networks safely)
get_sim_ip() {
    docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' "$1" 2>/dev/null | awk '{print $1}'
}

START_TIME=$(date +%s)

echo ""
echo "======================================================"
echo "  Zentra - Full Deployment (App + Simulation)"
echo "======================================================"
echo ""

# ---------------------------------------------------------------------------
# Step 1: Check Docker
# ---------------------------------------------------------------------------
info "Verifying Docker is available..."

if ! command -v docker &>/dev/null; then
    error "Docker is not installed."
    echo "  Install from: https://docs.docker.com/engine/install/"
    exit 1
fi

if ! docker info &>/dev/null; then
    error "Docker daemon is not running. Start it with: sudo systemctl start docker"
    exit 1
fi

ok "Docker is ready"

# ---------------------------------------------------------------------------
# Step 2: Clean up existing containers
# ---------------------------------------------------------------------------
echo ""
info "Stopping and removing existing containers..."

ALL_CONTAINERS="zentra zentra-agent srv-linux-01 srv-linux-02 srv-win-01 srv-win-02 sw-core-01 sw-access-01 fw-01 stor-01 mw-nginx-01 mw-apache-01 mw-tomcat-01 db-oracle-01"

REMOVED_COUNT=0
for ctr in ${ALL_CONTAINERS}; do
    if docker ps -aq -f "name=^${ctr}$" 2>/dev/null | grep -q .; then
        docker stop "${ctr}" >/dev/null 2>&1 || true
        docker rm -f "${ctr}" >/dev/null 2>&1 || true
        REMOVED_COUNT=$((REMOVED_COUNT + 1))
    fi
done

if [ "${REMOVED_COUNT}" -gt 0 ]; then
    ok "Removed ${REMOVED_COUNT} existing container(s)"
else
    ok "No existing containers to clean up"
fi

# ---------------------------------------------------------------------------
# Step 3: Run Zentra Application Installer
# ---------------------------------------------------------------------------
echo ""
echo "======================================================"
echo "  Phase 1/2: Zentra Application"
echo "======================================================"
echo ""

INSTALL_SCRIPT="${SCRIPT_DIR}/install.sh"
if [ ! -f "${INSTALL_SCRIPT}" ]; then
    error "install.sh not found at: ${INSTALL_SCRIPT}"
    exit 1
fi

INSTALL_SUCCESS=true
bash "${INSTALL_SCRIPT}" || INSTALL_SUCCESS=false

if [ "${INSTALL_SUCCESS}" = true ]; then
    ok "Zentra application deployment complete"
else
    warn "Zentra application deployment had issues - continuing with simulation..."
fi

sleep 3

# ---------------------------------------------------------------------------
# Step 4: Run Datacenter Simulation Setup
# ---------------------------------------------------------------------------
echo ""
echo "======================================================"
echo "  Phase 2/2: Datacenter Simulation"
echo "======================================================"
echo ""

SIM_SCRIPT="${SCRIPT_DIR}/simulation/setup.sh"
if [ ! -f "${SIM_SCRIPT}" ]; then
    error "simulation/setup.sh not found at: ${SIM_SCRIPT}"
    exit 1
fi

SIM_SUCCESS=true
bash "${SIM_SCRIPT}" || SIM_SUCCESS=false

if [ "${SIM_SUCCESS}" = true ]; then
    ok "Datacenter simulation deployment complete"
else
    warn "Simulation deployment had issues"
fi

# ---------------------------------------------------------------------------
# Step 5: Wait for zentra-agent to be healthy
# ---------------------------------------------------------------------------
echo ""
info "Waiting for zentra-agent to start and distribute SSH keys..."

AGENT_READY=false
for wait_i in $(seq 1 30); do
    if docker ps --filter "name=zentra-agent" --filter "status=running" --format '{{.Names}}' 2>/dev/null | grep -q "zentra-agent"; then
        if docker exec zentra-agent pgrep -f zentra-agent >/dev/null 2>&1; then
            AGENT_READY=true
            break
        fi
    fi
    printf "."
    sleep 5
done
echo ""

if [ "${AGENT_READY}" = true ]; then
    ok "zentra-agent is running and collecting metrics"
else
    warn "zentra-agent may not be fully ready yet — check logs: docker logs zentra-agent"
fi

# ---------------------------------------------------------------------------
# Step 6: Combined Summary
# ---------------------------------------------------------------------------
END_TIME=$(date +%s)
ELAPSED=$(( END_TIME - START_TIME ))
ELAPSED_MIN=$(( ELAPSED / 60 ))
ELAPSED_SEC=$(( ELAPSED % 60 ))
ELAPSED_STR=$(printf "%02d:%02d" ${ELAPSED_MIN} ${ELAPSED_SEC})

# Exclude Docker/virtual adapter IPs (172.16-31.x.x, 10.x.x.x, 169.254.x.x)
SERVER_IP=$(hostname -I 2>/dev/null | tr ' ' '\n' | grep -vE '^(172\.(1[6-9]|2[0-9]|3[01])\.|10\.|169\.254\.|127\.)' | head -1)
[ -z "${SERVER_IP}" ] && SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
[ -z "${SERVER_IP}" ] && SERVER_IP="localhost"


# Get container status
get_status() {
    local running
    running=$(docker inspect -f '{{.State.Running}}' "$1" 2>/dev/null || echo "false")
    if [ "${running}" = "true" ]; then
        echo "UP"
    else
        echo "DOWN"
    fi
}

echo ""
echo ""
echo "======================================================================"
echo "  Zentra Full Deployment - Summary                   [${ELAPSED_STR}]"
echo "======================================================================"
echo ""

if [ "${INSTALL_SUCCESS}" = true ]; then
    ZENTRA_STATUS="Running"
else
    ZENTRA_STATUS="Check Needed"
fi

echo "  ZENTRA APPLICATION"
echo "  ------------------"
echo "  Status:        ${ZENTRA_STATUS}"
echo "  Web UI:        http://${SERVER_IP}:5010"
echo "  Login:         admin@Zentra.local / changeme"
echo ""

# Simulation section
echo "  DATACENTER SIMULATION (12 Nodes)"
echo "  ---------------------------------"
echo ""
printf "  %-18s %-16s %-10s %-8s %s\n" "COMPONENT" "IP ADDRESS" "PORT(S)" "STATUS" "DESCRIPTION"
printf "  %-18s %-16s %-10s %-8s %s\n" "-----------------" "----------------" "----------" "--------" "----------------------------"

SIM_NODES=(
    "srv-linux-01|22,161|Rocky Linux 8 (Web Server)"
    "srv-linux-02|22,161|Ubuntu 22.04 (App Server)"
    "srv-win-01|22,161|Windows Server 2022 (AD/DNS)"
    "srv-win-02|22,161|Windows Server 2019 (Files)"
    "sw-core-01|22,161|Cisco Catalyst 9300 (Core)"
    "sw-access-01|22,161|Arista 7050X (Access)"
    "fw-01|22,161|Palo Alto PA-3260 (Firewall)"
    "stor-01|22,161|Dell PowerStore 500T (SAN)"
    "mw-nginx-01|22,80,8080|Nginx (Reverse Proxy)"
    "mw-apache-01|22,80,8080|Apache HTTP Server"
    "mw-tomcat-01|22,8080|Apache Tomcat 9"
    "db-oracle-01|22,1521|Oracle 21c XE (ZENTRADB)"
)

RUNNING_COUNT=0
for entry in "${SIM_NODES[@]}"; do
    IFS='|' read -r name ports desc <<< "${entry}"
    ip=$(get_sim_ip "${name}")
    status=$(get_status "${name}")

    if [ "${status}" = "UP" ]; then
        RUNNING_COUNT=$((RUNNING_COUNT + 1))
        printf "  ${GREEN}%-18s %-16s %-10s %-8s %s${NC}\n" "${name}" "${ip}" "${ports}" "${status}" "${desc}"
    else
        printf "  ${RED}%-18s %-16s %-10s %-8s %s${NC}\n" "${name}" "${ip}" "${ports}" "${status}" "${desc}"
    fi
done

echo ""
if [ "${RUNNING_COUNT}" -ge 12 ]; then
    ok "All ${RUNNING_COUNT}/12 simulation nodes are running"
else
    warn "${RUNNING_COUNT}/12 simulation nodes running"
fi

echo ""
echo "  IMPORTANT: Change the default admin password after first login!"
echo ""
echo "======================================================================"
echo ""
