#!/usr/bin/env bash
# =============================================================================
# Zentra Datacenter Simulation - Traffic Generator (Linux)
# Generates realistic monitoring traffic across all 12 simulated nodes.
#
# Usage:
#   chmod +x traffic-sim.sh
#   sudo ./traffic-sim.sh
#
# Press Ctrl+C to stop.
#
# Prerequisites:
#   - Simulation must be running (setup.sh completed)
#   - Packages: snmpwalk/snmpget, curl, sshpass (optional), sqlplus (optional)
#   - Install: dnf install -y net-snmp-utils curl sshpass
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
DIM='\033[2m'
NC='\033[0m'

info()  { echo -e "${CYAN}[TRAFFIC]${NC} $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}      $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}    $1"; }
error() { echo -e "${RED}[ERROR]${NC}   $1"; }
dim()   { echo -e "${DIM}[${1}]${NC} ${2}"; }

# ---------------------------------------------------------------------------
# Detect IPs from running containers
# ---------------------------------------------------------------------------
get_container_ip() {
    docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$1" 2>/dev/null
}

echo ""
echo "======================================================"
echo "  Zentra Datacenter Simulation - Traffic Generator"
echo "======================================================"
echo ""

info "Detecting simulation node IPs from running containers..."

IP_SRV_LINUX_01=$(get_container_ip "srv-linux-01")
IP_SRV_LINUX_02=$(get_container_ip "srv-linux-02")
IP_SRV_WIN_01=$(get_container_ip "srv-win-01")
IP_SRV_WIN_02=$(get_container_ip "srv-win-02")
IP_SW_CORE_01=$(get_container_ip "sw-core-01")
IP_SW_ACCESS_01=$(get_container_ip "sw-access-01")
IP_FW_01=$(get_container_ip "fw-01")
IP_STOR_01=$(get_container_ip "stor-01")
IP_MW_NGINX_01=$(get_container_ip "mw-nginx-01")
IP_MW_APACHE_01=$(get_container_ip "mw-apache-01")
IP_MW_TOMCAT_01=$(get_container_ip "mw-tomcat-01")
IP_DB_ORACLE_01=$(get_container_ip "db-oracle-01")

# Verify at least some containers are running
FOUND=0
for ip in ${IP_SRV_LINUX_01} ${IP_SRV_LINUX_02} ${IP_MW_NGINX_01}; do
    [ -n "${ip}" ] && FOUND=$((FOUND + 1))
done

if [ "${FOUND}" -eq 0 ]; then
    error "No simulation containers detected. Run setup.sh first."
    exit 1
fi

ok "Detected container IPs"

# ---------------------------------------------------------------------------
# Check available tools
# ---------------------------------------------------------------------------
HAS_SNMPWALK=false
HAS_SNMPGET=false
HAS_CURL=false
HAS_SSHPASS=false
HAS_SQLPLUS=false

command -v snmpwalk >/dev/null 2>&1 && HAS_SNMPWALK=true
command -v snmpget  >/dev/null 2>&1 && HAS_SNMPGET=true
command -v curl     >/dev/null 2>&1 && HAS_CURL=true
command -v sshpass  >/dev/null 2>&1 && HAS_SSHPASS=true
command -v sqlplus  >/dev/null 2>&1 && HAS_SQLPLUS=true

info "Available tools:"
info "  snmpwalk: ${HAS_SNMPWALK}  snmpget: ${HAS_SNMPGET}  curl: ${HAS_CURL}"
info "  sshpass:  ${HAS_SSHPASS}  sqlplus: ${HAS_SQLPLUS}"

if [ "${HAS_SNMPWALK}" = false ] && [ "${HAS_CURL}" = false ]; then
    error "Need at least snmpwalk or curl. Install: dnf install -y net-snmp-utils curl"
    exit 1
fi

# ---------------------------------------------------------------------------
# Traffic generation functions
# ---------------------------------------------------------------------------

# SNMP walk a node (simulates Zentra SNMP polling)
poll_snmp() {
    local name="$1"
    local ip="$2"
    local oid="$3"
    if [ -z "${ip}" ]; then return; fi
    if [ "${HAS_SNMPWALK}" = true ]; then
        dim "${name}" "SNMP walk ${oid}"
        snmpwalk -v2c -c public -t 2 -r 1 "${ip}" "${oid}" >/dev/null 2>&1 || true
    fi
}

# SNMP get specific OID
get_snmp() {
    local name="$1"
    local ip="$2"
    local oid="$3"
    if [ -z "${ip}" ]; then return; fi
    if [ "${HAS_SNMPGET}" = true ]; then
        dim "${name}" "SNMP get ${oid}"
        snmpget -v2c -c public -t 2 -r 1 "${ip}" "${oid}" >/dev/null 2>&1 || true
    fi
}

# HTTP request (simulates Zentra middleware polling)
poll_http() {
    local name="$1"
    local url="$2"
    if [ "${HAS_CURL}" = true ]; then
        dim "${name}" "HTTP GET ${url}"
        curl -s -o /dev/null -w "" --connect-timeout 3 --max-time 5 "${url}" 2>/dev/null || true
    fi
}

# SSH command (simulates Zentra SSH-based server monitoring)
poll_ssh() {
    local name="$1"
    local ip="$2"
    local cmd="$3"
    if [ -z "${ip}" ]; then return; fi
    if [ "${HAS_SSHPASS}" = true ]; then
        dim "${name}" "SSH: ${cmd}"
        sshpass -p 'zentra123' ssh -o StrictHostKeyChecking=no -o ConnectTimeout=3 \
            root@"${ip}" "${cmd}" >/dev/null 2>&1 || true
    fi
}

# Oracle SQL query (simulates Zentra DB monitoring)
poll_oracle() {
    local name="$1"
    local ip="$2"
    local sql="$3"
    if [ -z "${ip}" ]; then return; fi
    if [ "${HAS_SQLPLUS}" = true ]; then
        dim "${name}" "SQL: ${sql}"
        echo "${sql}" | sqlplus -S "zentra_mon/zentra123@${ip}:1521/XE" >/dev/null 2>&1 || true
    else
        # Fallback: just test TCP connectivity to Oracle port
        if [ "${HAS_CURL}" = true ]; then
            dim "${name}" "TCP check :1521"
            curl -s -o /dev/null --connect-timeout 2 "telnet://${ip}:1521" 2>/dev/null || true
        fi
    fi
}

# ---------------------------------------------------------------------------
# Traffic simulation loop
# ---------------------------------------------------------------------------
CYCLE=0
INTERVAL=15

info "Starting traffic simulation (every ${INTERVAL}s). Press Ctrl+C to stop."
echo ""

trap 'echo ""; info "Traffic simulation stopped after ${CYCLE} cycles."; exit 0' INT TERM

while true; do
    CYCLE=$((CYCLE + 1))
    echo ""
    info "=== Cycle ${CYCLE} [$(date '+%H:%M:%S')] ==="

    # -----------------------------------------------------------------------
    # Linux Servers - SNMP + SSH
    # -----------------------------------------------------------------------
    poll_snmp "srv-linux-01" "${IP_SRV_LINUX_01}" "system"
    poll_snmp "srv-linux-01" "${IP_SRV_LINUX_01}" ".1.3.6.1.4.1.2021.11"
    poll_snmp "srv-linux-01" "${IP_SRV_LINUX_01}" ".1.3.6.1.4.1.2021.4"
    poll_snmp "srv-linux-01" "${IP_SRV_LINUX_01}" ".1.3.6.1.2.1.2.2"
    poll_ssh  "srv-linux-01" "${IP_SRV_LINUX_01}" "uptime; df -h; free -m; top -bn1 | head -5"

    poll_snmp "srv-linux-02" "${IP_SRV_LINUX_02}" "system"
    poll_snmp "srv-linux-02" "${IP_SRV_LINUX_02}" ".1.3.6.1.4.1.2021.11"
    poll_snmp "srv-linux-02" "${IP_SRV_LINUX_02}" ".1.3.6.1.4.1.2021.4"
    poll_ssh  "srv-linux-02" "${IP_SRV_LINUX_02}" "uptime; df -h; free -m"

    # -----------------------------------------------------------------------
    # Windows Servers - SNMP
    # -----------------------------------------------------------------------
    poll_snmp "srv-win-01" "${IP_SRV_WIN_01}" "system"
    poll_snmp "srv-win-01" "${IP_SRV_WIN_01}" ".1.3.6.1.2.1.25.2.3"
    poll_snmp "srv-win-01" "${IP_SRV_WIN_01}" ".1.3.6.1.2.1.25.3.3"
    get_snmp  "srv-win-01" "${IP_SRV_WIN_01}" ".1.3.6.1.2.1.25.1.1.0"

    poll_snmp "srv-win-02" "${IP_SRV_WIN_02}" "system"
    poll_snmp "srv-win-02" "${IP_SRV_WIN_02}" ".1.3.6.1.2.1.25.2.3"
    poll_snmp "srv-win-02" "${IP_SRV_WIN_02}" ".1.3.6.1.2.1.25.3.3"
    get_snmp  "srv-win-02" "${IP_SRV_WIN_02}" ".1.3.6.1.2.1.25.1.1.0"

    # -----------------------------------------------------------------------
    # Network Switches - SNMP
    # -----------------------------------------------------------------------
    poll_snmp "sw-core-01" "${IP_SW_CORE_01}" "system"
    poll_snmp "sw-core-01" "${IP_SW_CORE_01}" ".1.3.6.1.2.1.2.2"
    poll_snmp "sw-core-01" "${IP_SW_CORE_01}" ".1.3.6.1.2.1.47.1.1.1"
    get_snmp  "sw-core-01" "${IP_SW_CORE_01}" ".1.3.6.1.4.1.9.9.109.1.1.1.1.8.1"
    get_snmp  "sw-core-01" "${IP_SW_CORE_01}" ".1.3.6.1.4.1.9.9.13.1.3.1.3.1"

    poll_snmp "sw-access-01" "${IP_SW_ACCESS_01}" "system"
    poll_snmp "sw-access-01" "${IP_SW_ACCESS_01}" ".1.3.6.1.2.1.2.2"
    poll_snmp "sw-access-01" "${IP_SW_ACCESS_01}" ".1.3.6.1.2.1.47.1.1.1"

    # -----------------------------------------------------------------------
    # Firewall - SNMP
    # -----------------------------------------------------------------------
    poll_snmp "fw-01" "${IP_FW_01}" "system"
    poll_snmp "fw-01" "${IP_FW_01}" ".1.3.6.1.2.1.2.2"
    get_snmp  "fw-01" "${IP_FW_01}" ".1.3.6.1.4.1.25461.2.1.2.3.1.0"
    get_snmp  "fw-01" "${IP_FW_01}" ".1.3.6.1.4.1.25461.2.1.2.3.3.0"
    get_snmp  "fw-01" "${IP_FW_01}" ".1.3.6.1.4.1.25461.2.1.2.1.1.0"

    # -----------------------------------------------------------------------
    # Storage Array - SNMP
    # -----------------------------------------------------------------------
    poll_snmp "stor-01" "${IP_STOR_01}" "system"
    poll_snmp "stor-01" "${IP_STOR_01}" ".1.3.6.1.2.1.2.2"
    poll_snmp "stor-01" "${IP_STOR_01}" ".1.3.6.1.2.1.25.2.3"

    # -----------------------------------------------------------------------
    # Middleware - HTTP
    # -----------------------------------------------------------------------
    if [ -n "${IP_MW_NGINX_01}" ]; then
        poll_http "mw-nginx-01" "http://${IP_MW_NGINX_01}/"
        poll_http "mw-nginx-01" "http://${IP_MW_NGINX_01}:8080/nginx_status"
        poll_http "mw-nginx-01" "http://${IP_MW_NGINX_01}:8080/healthz"
    fi

    if [ -n "${IP_MW_APACHE_01}" ]; then
        poll_http "mw-apache-01" "http://${IP_MW_APACHE_01}/"
        poll_http "mw-apache-01" "http://${IP_MW_APACHE_01}:8080/server-status"
        poll_http "mw-apache-01" "http://${IP_MW_APACHE_01}:8080/server-status?auto"
    fi

    if [ -n "${IP_MW_TOMCAT_01}" ]; then
        poll_http "mw-tomcat-01" "http://${IP_MW_TOMCAT_01}:8080/"
        poll_http "mw-tomcat-01" "http://zentra:zentra123@${IP_MW_TOMCAT_01}:8080/manager/status"
        poll_http "mw-tomcat-01" "http://zentra:zentra123@${IP_MW_TOMCAT_01}:8080/manager/jmxproxy/?qry=java.lang:type=Memory"
    fi

    # -----------------------------------------------------------------------
    # Oracle Database
    # -----------------------------------------------------------------------
    poll_oracle "db-oracle-01" "${IP_DB_ORACLE_01}" "SELECT STATUS FROM V\$INSTANCE;"
    poll_oracle "db-oracle-01" "${IP_DB_ORACLE_01}" "SELECT TABLESPACE_NAME, STATUS FROM DBA_TABLESPACES;"
    poll_oracle "db-oracle-01" "${IP_DB_ORACLE_01}" "SELECT NAME, VALUE FROM V\$SYSSTAT WHERE NAME IN ('user commits','user rollbacks','db block gets','consistent gets','physical reads');"
    poll_oracle "db-oracle-01" "${IP_DB_ORACLE_01}" "SELECT COUNT(*) FROM V\$SESSION WHERE STATUS='ACTIVE';"

    # -----------------------------------------------------------------------
    # Burst traffic (every 5th cycle, simulate heavier load)
    # -----------------------------------------------------------------------
    if [ $((CYCLE % 5)) -eq 0 ]; then
        info "--- Burst cycle: generating extra load ---"
        for i in $(seq 1 10); do
            poll_http "mw-nginx-01" "http://${IP_MW_NGINX_01}/" &
            poll_http "mw-apache-01" "http://${IP_MW_APACHE_01}/" &
            poll_http "mw-tomcat-01" "http://${IP_MW_TOMCAT_01}:8080/" &
        done
        wait
        poll_snmp "sw-core-01" "${IP_SW_CORE_01}" ".1.3.6.1.2.1.2.2.1.10"
        poll_snmp "sw-core-01" "${IP_SW_CORE_01}" ".1.3.6.1.2.1.2.2.1.16"
    fi

    ok "Cycle ${CYCLE} complete"
    info "Sleeping ${INTERVAL}s..."
    sleep ${INTERVAL}
done
