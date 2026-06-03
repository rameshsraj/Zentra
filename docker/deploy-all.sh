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
# Step 2: Run Zentra Application Installer
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
# Step 3: Run Datacenter Simulation Setup
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
# Step 4: Ensure SSH tools are available in the Zentra container
# ---------------------------------------------------------------------------
echo ""
info "Checking SSH tools in Zentra container..."

if docker exec zentra which sshpass >/dev/null 2>&1; then
    ok "SSH tools already available"
else
    info "Installing openssh-clients and sshpass in Zentra container..."
    if docker exec zentra dnf install -y openssh-clients sshpass >/dev/null 2>&1; then
        ok "SSH tools installed (openssh-clients, sshpass)"
    else
        warn "Failed to install SSH tools - SSH key setup may not work"
    fi
fi

# Generate SSH keys if missing (or regenerate if passphrase-protected)
KEY_EXISTS=false
if docker exec zentra test -f /root/.ssh/id_rsa 2>/dev/null; then
    # Verify key has no passphrase
    if docker exec zentra ssh-keygen -y -f /root/.ssh/id_rsa -P "" >/dev/null 2>&1; then
        ok "SSH key pair already exists (no passphrase)"
        KEY_EXISTS=true
    else
        warn "Existing SSH key has a passphrase - regenerating..."
        docker exec zentra rm -f /root/.ssh/id_rsa /root/.ssh/id_rsa.pub 2>/dev/null
    fi
fi

if [ "${KEY_EXISTS}" = false ]; then
    info "Generating SSH key pair in Zentra container..."
    docker exec zentra mkdir -p /root/.ssh
    docker exec zentra chmod 700 /root/.ssh
    docker exec zentra ssh-keygen -t rsa -b 4096 -f /root/.ssh/id_rsa -N "" -q
    docker exec zentra sh -c "echo 'Host *' > /root/.ssh/config"
    docker exec zentra sh -c "echo '    StrictHostKeyChecking no' >> /root/.ssh/config"
    docker exec zentra sh -c "echo '    UserKnownHostsFile /dev/null' >> /root/.ssh/config"
    docker exec zentra sh -c "echo '    LogLevel ERROR' >> /root/.ssh/config"
    docker exec zentra sh -c "echo '    GSSAPIAuthentication no' >> /root/.ssh/config"
    docker exec zentra chmod 600 /root/.ssh/config
    if [ $? -eq 0 ]; then
        ok "SSH key pair generated"
    else
        warn "Failed to generate SSH key pair"
    fi
fi

# ---------------------------------------------------------------------------
# Step 5: Connect Zentra to the simulation network
# ---------------------------------------------------------------------------
echo ""
info "Connecting Zentra container to simulation network..."

# Detect the simulation network name dynamically
SIM_NETWORK=$(docker network ls --format '{{.Name}}' 2>/dev/null | grep "zentra-dc" | head -1)

if [ -z "${SIM_NETWORK}" ]; then
    warn "Simulation network not found. Is the simulation running?"
else
    info "Found simulation network: ${SIM_NETWORK}"
    if docker inspect zentra --format '{{range $k, $v := .NetworkSettings.Networks}}{{$k}} {{end}}' 2>/dev/null | grep -q "${SIM_NETWORK}"; then
        ok "Zentra is already connected to ${SIM_NETWORK}"
    else
        if docker network connect "${SIM_NETWORK}" zentra 2>/dev/null; then
            ok "Zentra connected to simulation network (${SIM_NETWORK})"
            info "Zentra can now reach all simulation nodes directly"
        else
            warn "Could not connect Zentra to simulation network"
        fi
    fi

    # Verify connectivity
    info "Verifying connectivity from Zentra to simulation nodes..."
    FIRST_SIM_IP=$(get_sim_ip srv-linux-01)
    if [ -n "${FIRST_SIM_IP}" ]; then
        if docker exec zentra ping -c 1 -W 2 "${FIRST_SIM_IP}" >/dev/null 2>&1; then
            ok "Zentra can reach simulation nodes (ping ${FIRST_SIM_IP} OK)"
        else
            warn "Ping to ${FIRST_SIM_IP} failed from Zentra container"
        fi
    fi
fi

# ---------------------------------------------------------------------------
# Step 6: Distribute SSH keys to simulation nodes
# ---------------------------------------------------------------------------
echo ""
info "Distributing SSH keys to simulation nodes..."

SSH_TARGETS="srv-linux-01 srv-linux-02 srv-win-01 srv-win-02 sw-core-01 sw-access-01 fw-01 stor-01 mw-nginx-01 mw-apache-01 mw-tomcat-01 db-oracle-01"
KEY_COUNT=0

for target in ${SSH_TARGETS}; do
    TARGET_IP=$(get_sim_ip "${target}")
    if [ -n "${TARGET_IP}" ]; then
        if docker exec zentra sshpass -p "zentra123" ssh-copy-id -o StrictHostKeyChecking=no -o GSSAPIAuthentication=no "root@${TARGET_IP}" >/dev/null 2>&1; then
            KEY_COUNT=$((KEY_COUNT + 1))
        fi
    fi
done

if [ "${KEY_COUNT}" -gt 0 ]; then
    ok "SSH keys distributed to ${KEY_COUNT} nodes"
else
    warn "Could not distribute SSH keys to any nodes"
fi

# Verify SSH works without password
info "Verifying passwordless SSH..."
FIRST_SIM_IP=$(get_sim_ip srv-linux-01)
if [ -n "${FIRST_SIM_IP}" ]; then
    SSH_RESULT=$(docker exec zentra ssh -o BatchMode=yes -o ConnectTimeout=5 -o GSSAPIAuthentication=no "root@${FIRST_SIM_IP}" hostname 2>/dev/null)
    if [ $? -eq 0 ]; then
        ok "Passwordless SSH verified (srv-linux-01: ${SSH_RESULT})"
    else
        warn "Passwordless SSH verification failed - try manually: docker exec zentra sshpass -p zentra123 ssh-copy-id root@${FIRST_SIM_IP}"
    fi
else
    warn "Could not get IP for srv-linux-01 - skipping SSH verification"
fi

# ---------------------------------------------------------------------------
# Step 7: Combined Summary
# ---------------------------------------------------------------------------
END_TIME=$(date +%s)
ELAPSED=$(( END_TIME - START_TIME ))
ELAPSED_MIN=$(( ELAPSED / 60 ))
ELAPSED_SEC=$(( ELAPSED % 60 ))
ELAPSED_STR=$(printf "%02d:%02d" ${ELAPSED_MIN} ${ELAPSED_SEC})

SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
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
