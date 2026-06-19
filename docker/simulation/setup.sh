#!/usr/bin/env bash
# =============================================================================
# Zentra Datacenter Simulation - Setup Script (Linux)
# Auto-detects host network, assigns IPs, and starts all simulated nodes.
#
# Usage:
#   chmod +x setup.sh
#   sudo ./setup.sh
#
# Prerequisites: Docker and docker-compose must be installed.
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
COMPOSE_TEMPLATE="${SCRIPT_DIR}/docker-compose.yml"
COMPOSE_GENERATED="${SCRIPT_DIR}/docker-compose.generated.yml"
IP_OFFSET_START=200

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

echo ""
echo "======================================================"
echo "  Zentra Datacenter Simulation - Setup (Linux)"
echo "======================================================"
echo ""

# ---------------------------------------------------------------------------
# Step 1: Check prerequisites
# ---------------------------------------------------------------------------
info "Checking prerequisites..."

if ! command -v docker >/dev/null 2>&1; then
    error "Docker is not installed."
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    error "Docker daemon is not running."
    exit 1
fi

# Check for docker-compose or docker compose
COMPOSE_CMD=""
if command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD="docker-compose"
elif docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD="docker compose"
else
    error "docker-compose is not installed."
    exit 1
fi

ok "Docker and ${COMPOSE_CMD} are available"

# ---------------------------------------------------------------------------
# Step 2: Detect host network
# ---------------------------------------------------------------------------
info "Detecting host network configuration..."

# Find the default route interface
HOST_IFACE=$(ip route | grep default | awk '{print $5}' | head -1)
if [ -z "${HOST_IFACE}" ]; then
    error "Cannot detect default network interface."
    echo "  Available interfaces:"
    ip -o link show | awk '{print "    " $2}'
    echo ""
    echo "  Set manually: export HOST_IFACE=eth0"
    exit 1
fi

# Get host IP and subnet
HOST_IP=$(ip -4 addr show "${HOST_IFACE}" | grep -oP '(?<=inet\s)\d+(\.\d+){3}')
HOST_CIDR=$(ip -4 addr show "${HOST_IFACE}" | grep -oP '(?<=inet\s)\d+(\.\d+){3}/\d+')
HOST_SUBNET_MASK=$(echo "${HOST_CIDR}" | cut -d'/' -f2)
HOST_GATEWAY=$(ip route | grep default | awk '{print $3}' | head -1)

# Calculate subnet base using proper bitwise masking
IFS='.' read -r o1 o2 o3 o4 <<< "${HOST_IP}"

if [ "${HOST_SUBNET_MASK}" -ge 24 ]; then
    # /24-/32: zero out last octet
    HOST_SUBNET="${o1}.${o2}.${o3}.0/${HOST_SUBNET_MASK}"
    IP_BASE="${o1}.${o2}.${o3}"
elif [ "${HOST_SUBNET_MASK}" -ge 16 ]; then
    # /16-/23: mask the third octet properly
    # For /23: mask = 0xFE (254), e.g. 11 & 254 = 10 → 10.88.10.0/23
    BITS_IN_O3=$((HOST_SUBNET_MASK - 16))
    MASK_O3=$(( (255 << (8 - BITS_IN_O3)) & 255 ))
    MASKED_O3=$(( o3 & MASK_O3 ))
    HOST_SUBNET="${o1}.${o2}.${MASKED_O3}.0/${HOST_SUBNET_MASK}"
    IP_BASE="${o1}.${o2}.${o3}"
else
    HOST_SUBNET="${o1}.${o2}.0.0/${HOST_SUBNET_MASK}"
    IP_BASE="${o1}.${o2}.${o3}"
fi

ok "Interface: ${HOST_IFACE}"
ok "Host IP:   ${HOST_IP}"
ok "Subnet:    ${HOST_SUBNET}"
ok "Gateway:   ${HOST_GATEWAY}"

# ---------------------------------------------------------------------------
# Step 3: Assign IPs to simulation nodes
# ---------------------------------------------------------------------------
info "Assigning IPs from ${IP_BASE}.${IP_OFFSET_START}+..."

# Node IP assignments (starting from offset)
IP_SRV_LINUX_01="${IP_BASE}.$((IP_OFFSET_START + 1))"
IP_SRV_LINUX_02="${IP_BASE}.$((IP_OFFSET_START + 2))"
IP_SRV_WIN_01="${IP_BASE}.$((IP_OFFSET_START + 3))"
IP_SRV_WIN_02="${IP_BASE}.$((IP_OFFSET_START + 4))"
IP_SW_CORE_01="${IP_BASE}.$((IP_OFFSET_START + 5))"
IP_SW_ACCESS_01="${IP_BASE}.$((IP_OFFSET_START + 6))"
IP_FW_01="${IP_BASE}.$((IP_OFFSET_START + 7))"
IP_STOR_01="${IP_BASE}.$((IP_OFFSET_START + 8))"
IP_MW_NGINX_01="${IP_BASE}.$((IP_OFFSET_START + 9))"
IP_MW_APACHE_01="${IP_BASE}.$((IP_OFFSET_START + 10))"
IP_MW_TOMCAT_01="${IP_BASE}.$((IP_OFFSET_START + 11))"
IP_DB_ORACLE_01="${IP_BASE}.$((IP_OFFSET_START + 12))"

# Check for IP conflicts
info "Checking for IP conflicts..."
CONFLICT=false
for ip in ${IP_SRV_LINUX_01} ${IP_SRV_LINUX_02} ${IP_SRV_WIN_01} ${IP_SRV_WIN_02} \
          ${IP_SW_CORE_01} ${IP_SW_ACCESS_01} ${IP_FW_01} ${IP_STOR_01} \
          ${IP_MW_NGINX_01} ${IP_MW_APACHE_01} ${IP_MW_TOMCAT_01} ${IP_DB_ORACLE_01}; do
    if ping -c 1 -W 1 "${ip}" >/dev/null 2>&1; then
        warn "IP ${ip} is already in use!"
        CONFLICT=true
    fi
done

if [ "${CONFLICT}" = true ]; then
    warn "Some IPs are in use. Adjust IP_OFFSET_START in this script (currently ${IP_OFFSET_START})."
    echo -n "Continue anyway? (y/N): "
    read -r answer
    if [ "${answer}" != "y" ] && [ "${answer}" != "Y" ]; then
        exit 1
    fi
fi

ok "IP assignments ready"

# ---------------------------------------------------------------------------
# Step 4: Stop all running simulation containers
# ---------------------------------------------------------------------------
SIM_CONTAINERS="srv-linux-01 srv-linux-02 srv-win-01 srv-win-02 sw-core-01 sw-access-01 fw-01 stor-01 mw-nginx-01 mw-apache-01 mw-tomcat-01 db-oracle-01"

info "Phase 1/5: Stopping any running simulation containers..."

# Stop via compose if a previous generated file exists
if [ -f "${COMPOSE_GENERATED}" ]; then
    info "Stopping compose services..."
    ${COMPOSE_CMD} -f "${COMPOSE_GENERATED}" stop 2>/dev/null || true
    sleep 3
fi

# Stop individual containers that might be running outside compose
for name in ${SIM_CONTAINERS}; do
    if docker ps --format '{{.Names}}' | grep -q "^${name}$"; then
        info "  Stopping ${name}..."
        docker stop "${name}" >/dev/null 2>&1 || true
    fi
done

sleep 5
ok "All simulation containers stopped"

# ---------------------------------------------------------------------------
# Step 5: Remove old containers, volumes, and networks
# ---------------------------------------------------------------------------
info "Phase 2/5: Removing old containers..."

if [ -f "${COMPOSE_GENERATED}" ]; then
    ${COMPOSE_CMD} -f "${COMPOSE_GENERATED}" down --volumes --remove-orphans 2>/dev/null || true
    sleep 3
fi

for name in ${SIM_CONTAINERS}; do
    docker rm -f "${name}" 2>/dev/null || true
done

sleep 3
ok "Old containers removed"

info "Phase 2/5: Removing old volumes and networks..."

# Remove any simulation volumes
for name in ${SIM_CONTAINERS}; do
    docker volume rm "${name}" 2>/dev/null || true
done

sleep 2

# Remove both the bare name and any compose-prefixed name (e.g. simulation_zentra-dc)
docker network rm zentra-dc 2>/dev/null || true
for old_net in $(docker network ls --format '{{.Name}}' 2>/dev/null | grep "zentra-dc"); do
    docker network rm "${old_net}" 2>/dev/null || true
done

sleep 3
ok "Old volumes and networks removed"

# Remove dangling images
DANGLING=$(docker images --filter "dangling=true" -q 2>/dev/null)
if [ -n "${DANGLING}" ]; then
    info "Removing dangling images..."
    docker rmi ${DANGLING} 2>/dev/null || true
    sleep 2
    ok "Dangling images cleaned"
fi

ok "Full cleanup complete"

# ---------------------------------------------------------------------------
# Step 6: Generate and configure docker-compose
# ---------------------------------------------------------------------------
info "Phase 3/5: Generating docker-compose configuration..."

sed \
    -e "s|__HOST_IFACE__|${HOST_IFACE}|g" \
    -e "s|__HOST_SUBNET__|${HOST_SUBNET}|g" \
    -e "s|__HOST_GATEWAY__|${HOST_GATEWAY}|g" \
    -e "s|__ZENTRA_HOST__|${HOST_IP}|g" \
    -e "s|__IP_SRV_LINUX_01__|${IP_SRV_LINUX_01}|g" \
    -e "s|__IP_SRV_LINUX_02__|${IP_SRV_LINUX_02}|g" \
    -e "s|__IP_SRV_WIN_01__|${IP_SRV_WIN_01}|g" \
    -e "s|__IP_SRV_WIN_02__|${IP_SRV_WIN_02}|g" \
    -e "s|__IP_SW_CORE_01__|${IP_SW_CORE_01}|g" \
    -e "s|__IP_SW_ACCESS_01__|${IP_SW_ACCESS_01}|g" \
    -e "s|__IP_FW_01__|${IP_FW_01}|g" \
    -e "s|__IP_STOR_01__|${IP_STOR_01}|g" \
    -e "s|__IP_MW_NGINX_01__|${IP_MW_NGINX_01}|g" \
    -e "s|__IP_MW_APACHE_01__|${IP_MW_APACHE_01}|g" \
    -e "s|__IP_MW_TOMCAT_01__|${IP_MW_TOMCAT_01}|g" \
    -e "s|__IP_DB_ORACLE_01__|${IP_DB_ORACLE_01}|g" \
    "${COMPOSE_TEMPLATE}" > "${COMPOSE_GENERATED}"

ok "Generated: ${COMPOSE_GENERATED}"

sleep 2

# ---------------------------------------------------------------------------
# Step 7: Pull required Docker images
# ---------------------------------------------------------------------------
info "Phase 4/5: Pulling required Docker images..."

${COMPOSE_CMD} -f "${COMPOSE_GENERATED}" pull

sleep 3
ok "All images pulled"

# ---------------------------------------------------------------------------
# Step 8: Start all simulation nodes
# ---------------------------------------------------------------------------
info "Phase 5/5: Starting all simulation nodes..."

${COMPOSE_CMD} -f "${COMPOSE_GENERATED}" up -d

sleep 5
ok "All containers started"

# ---------------------------------------------------------------------------
# Step 9: Wait for services to initialize
# ---------------------------------------------------------------------------
info "Waiting for services to fully initialize..."

for i in $(seq 1 12); do
    RUNNING=$(docker ps --filter "status=running" --format '{{.Names}}' | grep -cE 'srv-|sw-|fw-|stor-|mw-|db-' || true)
    if [ "${RUNNING}" -ge 12 ]; then
        break
    fi
    printf "."
    sleep 3
done
echo ""

RUNNING=$(docker ps --filter "status=running" --format '{{.Names}}' | grep -cE 'srv-|sw-|fw-|stor-|mw-|db-' || true)
if [ "${RUNNING}" -ge 12 ]; then
    ok "All ${RUNNING} of 12 simulation nodes are running"
else
    warn "${RUNNING} of 12 nodes running. Check failed containers with: docker ps -a"
fi

# ---------------------------------------------------------------------------
# Step 10: Display summary table
# ---------------------------------------------------------------------------
echo ""
echo "======================================================"
echo "  Zentra Datacenter Simulation - Node Summary"
echo "======================================================"
echo ""
printf "%-20s %-18s %-10s %-30s\n" "COMPONENT" "IP ADDRESS" "PORT(S)" "DESCRIPTION"
printf "%-20s %-18s %-10s %-30s\n" "--------------------" "------------------" "----------" "------------------------------"
printf "%-20s %-18s %-10s %-30s\n" "srv-linux-01"   "${IP_SRV_LINUX_01}"   "22,161"   "Rocky Linux 8 (Web+Agent)"
printf "%-20s %-18s %-10s %-30s\n" "srv-linux-02"   "${IP_SRV_LINUX_02}"   "22,161"   "Ubuntu 22.04 (App+Agent)"
printf "%-20s %-18s %-10s %-30s\n" "srv-win-01"     "${IP_SRV_WIN_01}"     "22,161"   "Windows Server 2022 (AD/DNS)"
printf "%-20s %-18s %-10s %-30s\n" "srv-win-02"     "${IP_SRV_WIN_02}"     "22,161"   "Windows Server 2019 (Files)"
printf "%-20s %-18s %-10s %-30s\n" "sw-core-01"     "${IP_SW_CORE_01}"     "22,161"   "Cisco Catalyst 9300 (Core)"
printf "%-20s %-18s %-10s %-30s\n" "sw-access-01"   "${IP_SW_ACCESS_01}"   "22,161"   "Arista 7050X (Access)"
printf "%-20s %-18s %-10s %-30s\n" "fw-01"          "${IP_FW_01}"          "22,161"   "Palo Alto PA-3260 (Firewall)"
printf "%-20s %-18s %-10s %-30s\n" "stor-01"        "${IP_STOR_01}"        "22,161"   "Dell PowerStore 500T (SAN)"
printf "%-20s %-18s %-10s %-30s\n" "mw-nginx-01"    "${IP_MW_NGINX_01}"    "22,80,8080" "Nginx (Reverse Proxy)"
printf "%-20s %-18s %-10s %-30s\n" "mw-apache-01"   "${IP_MW_APACHE_01}"   "22,80,8080" "Apache HTTP Server"
printf "%-20s %-18s %-10s %-30s\n" "mw-tomcat-01"   "${IP_MW_TOMCAT_01}"   "22,8080"  "Apache Tomcat 9"
printf "%-20s %-18s %-10s %-30s\n" "db-oracle-01"   "${IP_DB_ORACLE_01}"   "22,1521"  "Oracle 21c XE (ZENTRADB)"
echo ""
echo "------------------------------------------------------"
echo "  SNMP Community:    public"
echo "  SSH Credentials:   root / zentra123 (all containers)"
echo "  Zentra Agents:     srv-linux-01, srv-linux-02 (REMOTE mode → ${HOST_IP}:5006)"
echo "  Tomcat Manager:    zentra / zentra123"
echo "  Nginx Status:      http://<nginx-ip>:8080/nginx_status"
echo "  Apache Status:     http://<apache-ip>:8080/server-status"
echo "  Oracle:            SYS/zentra123 or zentra_mon/zentra123"
echo "                     SID: XE  |  Service: ZENTRADB"
echo "------------------------------------------------------"
echo ""
echo "  To add these nodes in Zentra, go to:"
echo "    Infrastructure > Servers  (for servers)"
echo "    Infrastructure > Network  (for switches/firewall)"
echo "    Infrastructure > Storage  (for storage arrays)"
echo "    Middleware > [Nginx/Apache/Tomcat]"
echo "    Database > Oracle"
echo ""
echo "  To stop the simulation:"
echo "    ${COMPOSE_CMD} -f ${COMPOSE_GENERATED} down"
echo ""
echo "======================================================"
