# =============================================================================
# Zentra Datacenter Simulation - Setup Script (Windows PowerShell)
# Auto-detects host network, assigns IPs, and starts all simulated nodes.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File setup.ps1
#   powershell -ExecutionPolicy Bypass -File setup.ps1 -NoWait
#
# Prerequisites: Docker Desktop must be installed and running.
# =============================================================================
param([switch]$NoWait)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
$ComposeTemplate  = Join-Path $ScriptDir "docker-compose.yml"
$ComposeGenerated = Join-Path $ScriptDir "docker-compose.generated.yml"

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Cyan }
function Write-Ok    { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn  { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err   { Write-Host "[ERROR] $args" -ForegroundColor Red }

Write-Host ""
Write-Host "======================================================" -ForegroundColor White
Write-Host "  Zentra Datacenter Simulation - Setup (Windows)"       -ForegroundColor White
Write-Host "======================================================" -ForegroundColor White
Write-Host ""

# ---------------------------------------------------------------------------
# Step 1: Check prerequisites
# ---------------------------------------------------------------------------
Write-Info "Checking prerequisites..."

try {
    docker --version 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Docker not found" }
} catch {
    Write-Err "Docker is not installed or not in PATH."
    Read-Host "Press Enter to exit"
    exit 1
}

try {
    docker info 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Docker not running" }
} catch {
    Write-Err "Docker Desktop is not running."
    Read-Host "Press Enter to exit"
    exit 1
}

# Check for docker-compose
$ComposeCmd = ""
try {
    docker compose version 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) { $ComposeCmd = "docker compose" }
} catch {}
if (-not $ComposeCmd) {
    try {
        docker-compose --version 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) { $ComposeCmd = "docker-compose" }
    } catch {}
}
if (-not $ComposeCmd) {
    Write-Err "docker-compose is not available."
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Ok "Docker and $ComposeCmd are available"

# ---------------------------------------------------------------------------
# Step 2: Configure Docker bridge network
# ---------------------------------------------------------------------------
Write-Info "Configuring Docker bridge network for simulation..."

# On Windows Docker Desktop, macvlan is not supported.
# We use a dedicated Docker bridge network with a fixed subnet.
# All containers get predictable IPs within this subnet.
$DockerSubnet  = "10.100.0.0/24"
$DockerGateway = "10.100.0.1"
$IpBase        = "10.100.0"
$HostIface     = "bridge"
$HostSubnet    = $DockerSubnet
$HostGateway   = $DockerGateway

# Detect host IP (for display purposes only)
try {
    $adapter = Get-NetAdapter | Where-Object {
        $_.Status -eq "Up" -and $_.InterfaceDescription -notmatch "Loopback|vEthernet|Hyper-V|WSL"
    } | Sort-Object -Property ifIndex | Select-Object -First 1
    $ipConfig = Get-NetIPAddress -InterfaceIndex $adapter.ifIndex -AddressFamily IPv4 |
        Where-Object { $_.IPAddress -ne "127.0.0.1" } | Select-Object -First 1
    $HostIP = $ipConfig.IPAddress
} catch {
    $HostIP = "localhost"
}

Write-Ok "Docker Subnet: $DockerSubnet"
Write-Ok "Gateway:       $DockerGateway"
Write-Ok "Host IP:       $HostIP (for Zentra Web UI)"

# ---------------------------------------------------------------------------
# Step 3: Assign IPs to simulation nodes (within Docker subnet)
# ---------------------------------------------------------------------------
$SimIpStart = 10
Write-Info "Assigning IPs from ${IpBase}.${SimIpStart}+..."

$IpSrvLinux01  = "${IpBase}.$($SimIpStart + 1)"
$IpSrvLinux02  = "${IpBase}.$($SimIpStart + 2)"
$IpSrvWin01    = "${IpBase}.$($SimIpStart + 3)"
$IpSrvWin02    = "${IpBase}.$($SimIpStart + 4)"
$IpSwCore01    = "${IpBase}.$($SimIpStart + 5)"
$IpSwAccess01  = "${IpBase}.$($SimIpStart + 6)"
$IpFw01        = "${IpBase}.$($SimIpStart + 7)"
$IpStor01      = "${IpBase}.$($SimIpStart + 8)"
$IpMwNginx01   = "${IpBase}.$($SimIpStart + 9)"
$IpMwApache01  = "${IpBase}.$($SimIpStart + 10)"
$IpMwTomcat01  = "${IpBase}.$($SimIpStart + 11)"
$IpDbOracle01  = "${IpBase}.$($SimIpStart + 12)"

Write-Ok "IP assignments ready"

# ---------------------------------------------------------------------------
# Step 4: Stop all running simulation containers
# ---------------------------------------------------------------------------
$SimContainers = @("srv-linux-01","srv-linux-02","srv-win-01","srv-win-02",
                   "sw-core-01","sw-access-01","fw-01","stor-01",
                   "mw-nginx-01","mw-apache-01","mw-tomcat-01","db-oracle-01")

Write-Info "Phase 1/5: Stopping any running simulation containers..."

# Stop via compose if a previous generated file exists
if (Test-Path $ComposeGenerated) {
    Write-Info "Stopping compose services..."
    try { Invoke-Expression "$ComposeCmd -f `"$ComposeGenerated`" stop 2>&1" | Out-Null } catch {}
    Start-Sleep -Seconds 3
}

# Stop individual containers that might be running outside compose
foreach ($c in $SimContainers) {
    try {
        $isRunning = docker ps --format '{{.Names}}' 2>&1 | Where-Object { $_ -eq $c }
        if ($isRunning) {
            Write-Info "  Stopping $c..."
            docker stop $c 2>&1 | Out-Null
        }
    } catch {}
}

Start-Sleep -Seconds 5
Write-Ok "All simulation containers stopped"

# ---------------------------------------------------------------------------
# Step 5: Remove old containers, volumes, and networks
# ---------------------------------------------------------------------------
Write-Info "Phase 2/5: Removing old containers..."

if (Test-Path $ComposeGenerated) {
    try { Invoke-Expression "$ComposeCmd -f `"$ComposeGenerated`" down --volumes --remove-orphans 2>&1" | Out-Null } catch {}
    Start-Sleep -Seconds 3
}

foreach ($c in $SimContainers) {
    try { docker rm -f $c 2>&1 | Out-Null } catch {}
}

Start-Sleep -Seconds 3
Write-Ok "Old containers removed"

Write-Info "Phase 2/5: Removing old volumes and networks..."

foreach ($c in $SimContainers) {
    try { docker volume rm $c 2>&1 | Out-Null } catch {}
}

Start-Sleep -Seconds 2

# Remove both the bare name and any compose-prefixed name (e.g. simulation_zentra-dc)
try { docker network rm zentra-dc 2>&1 | Out-Null } catch {}
$oldNets = docker network ls --format '{{.Name}}' 2>$null | Where-Object { $_ -match "zentra-dc" }
foreach ($net in $oldNets) {
    try { docker network rm $net 2>&1 | Out-Null } catch {}
}

Start-Sleep -Seconds 3
Write-Ok "Old volumes and networks removed"

# Remove dangling images
try {
    $danglingImages = docker images --filter "dangling=true" -q 2>&1
    if ($danglingImages) {
        Write-Info "Removing dangling images..."
        docker rmi $danglingImages 2>&1 | Out-Null
        Start-Sleep -Seconds 2
        Write-Ok "Dangling images cleaned"
    }
} catch {}

Write-Ok "Full cleanup complete"

# ---------------------------------------------------------------------------
# Step 6: Generate and configure docker-compose
# ---------------------------------------------------------------------------
Write-Info "Phase 3/5: Generating docker-compose configuration..."

# On Windows with Docker Desktop, macvlan is not supported.
# Use bridge network instead.

$content = Get-Content $ComposeTemplate -Raw

# Replace macvlan with bridge for Windows compatibility
$content = $content -replace "driver: macvlan", "driver: bridge"
$content = $content -replace "driver_opts:", "# driver_opts (macvlan disabled on Windows):"
$content = $content -replace "parent: __HOST_IFACE__", "# parent: $HostIface"

$content = $content -replace "__HOST_IFACE__", $HostIface
$content = $content -replace "__HOST_SUBNET__", $HostSubnet
$content = $content -replace "__HOST_GATEWAY__", $HostGateway
$content = $content -replace "__IP_SRV_LINUX_01__", $IpSrvLinux01
$content = $content -replace "__IP_SRV_LINUX_02__", $IpSrvLinux02
$content = $content -replace "__IP_SRV_WIN_01__", $IpSrvWin01
$content = $content -replace "__IP_SRV_WIN_02__", $IpSrvWin02
$content = $content -replace "__IP_SW_CORE_01__", $IpSwCore01
$content = $content -replace "__IP_SW_ACCESS_01__", $IpSwAccess01
$content = $content -replace "__IP_FW_01__", $IpFw01
$content = $content -replace "__IP_STOR_01__", $IpStor01
$content = $content -replace "__IP_MW_NGINX_01__", $IpMwNginx01
$content = $content -replace "__IP_MW_APACHE_01__", $IpMwApache01
$content = $content -replace "__IP_MW_TOMCAT_01__", $IpMwTomcat01
$content = $content -replace "__IP_DB_ORACLE_01__", $IpDbOracle01

$content | Out-File -FilePath $ComposeGenerated -Encoding utf8 -NoNewline

Write-Ok "Generated: $ComposeGenerated"

Start-Sleep -Seconds 2

# ---------------------------------------------------------------------------
# Step 7: Pull required Docker images
# ---------------------------------------------------------------------------
Write-Info "Phase 4/5: Pulling required Docker images..."

Invoke-Expression "$ComposeCmd -f `"$ComposeGenerated`" pull"

Start-Sleep -Seconds 3
Write-Ok "All images pulled"

# ---------------------------------------------------------------------------
# Step 8: Start all simulation nodes
# ---------------------------------------------------------------------------
Write-Info "Phase 5/5: Starting all simulation nodes..."

Invoke-Expression "$ComposeCmd -f `"$ComposeGenerated`" up -d"

Start-Sleep -Seconds 5
Write-Ok "All containers started"

# ---------------------------------------------------------------------------
# Step 9: Wait for services to initialize
# ---------------------------------------------------------------------------
Write-Info "Waiting for services to fully initialize..."

for ($i = 1; $i -le 12; $i++) {
    $running = (docker ps --filter "status=running" --format '{{.Names}}' | Select-String -Pattern "srv-|sw-|fw-|stor-|mw-|db-").Count
    if ($running -ge 12) { break }
    Write-Host "." -NoNewline
    Start-Sleep -Seconds 3
}
Write-Host ""

$running = (docker ps --filter "status=running" --format '{{.Names}}' | Select-String -Pattern "srv-|sw-|fw-|stor-|mw-|db-").Count
if ($running -ge 12) {
    Write-Ok "All $running of 12 simulation nodes are running"
} else {
    Write-Warn "$running of 12 nodes running. Check failed containers with: docker ps -a"
}

# ---------------------------------------------------------------------------
# Step 10: Display summary table
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "======================================================" -ForegroundColor Green
Write-Host "  Zentra Datacenter Simulation - Node Summary"          -ForegroundColor Green
Write-Host "======================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Network: Docker bridge (${DockerSubnet})" -ForegroundColor DarkGray
Write-Host "  NOTE: These IPs are reachable from the Zentra container." -ForegroundColor DarkGray
Write-Host "        Use these IPs when adding targets in Zentra." -ForegroundColor DarkGray
Write-Host ""

$header = "{0,-20} {1,-18} {2,-10} {3,-30}" -f "COMPONENT","IP ADDRESS","PORT(S)","DESCRIPTION"
$separator = "{0,-20} {1,-18} {2,-10} {3,-30}" -f ("=" * 20),("=" * 18),("=" * 10),("=" * 30)

Write-Host $header
Write-Host $separator
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "srv-linux-01",   $IpSrvLinux01,  "22,161",  "Rocky Linux 8 (Web Server)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "srv-linux-02",   $IpSrvLinux02,  "22,161",  "Ubuntu 22.04 (App Server)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "srv-win-01",     $IpSrvWin01,    "22,161",  "Windows Server 2022 (AD/DNS)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "srv-win-02",     $IpSrvWin02,    "22,161",  "Windows Server 2019 (Files)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "sw-core-01",     $IpSwCore01,    "22,161",  "Cisco Catalyst 9300 (Core)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "sw-access-01",   $IpSwAccess01,  "22,161",  "Arista 7050X (Access)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "fw-01",          $IpFw01,        "22,161",  "Palo Alto PA-3260 (Firewall)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "stor-01",        $IpStor01,      "22,161",  "Dell PowerStore 500T (SAN)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "mw-nginx-01",    $IpMwNginx01,   "22,80,8080", "Nginx (Reverse Proxy)")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "mw-apache-01",   $IpMwApache01,  "22,80,8080", "Apache HTTP Server")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "mw-tomcat-01",   $IpMwTomcat01,  "22,8080", "Apache Tomcat 9")
Write-Host ("{0,-20} {1,-18} {2,-10} {3,-30}" -f "db-oracle-01",   $IpDbOracle01,  "22,1521", "Oracle 21c XE (ZENTRADB)")

Write-Host ""
Write-Host "------------------------------------------------------"
Write-Host "  SNMP Community:    public"
Write-Host "  SSH Credentials:   root / zentra123 (all containers)"
Write-Host "  Tomcat Manager:    zentra / zentra123"
Write-Host "  Nginx Status:      http://<nginx-ip>:8080/nginx_status"
Write-Host "  Apache Status:     http://<apache-ip>:8080/server-status"
Write-Host "  Oracle:            SYS/zentra123 or zentra_mon/zentra123"
Write-Host "                     SID: XE  |  Service: ZENTRADB"
Write-Host "------------------------------------------------------"
Write-Host ""
Write-Host "  To add these nodes in Zentra, go to:"
Write-Host "    Infrastructure > Servers  (for servers)"
Write-Host "    Infrastructure > Network  (for switches/firewall)"
Write-Host "    Infrastructure > Storage  (for storage arrays)"
Write-Host "    Middleware > [Nginx/Apache/Tomcat]"
Write-Host "    Database > Oracle"
Write-Host ""
Write-Host "  To stop the simulation:"
Write-Host "    $ComposeCmd -f $ComposeGenerated down"
Write-Host ""
Write-Host "======================================================" -ForegroundColor Green
Write-Host ""

if (-not $NoWait) { Read-Host "Press Enter to close" }
