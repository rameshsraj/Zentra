# =============================================================================
# Zentra - Full Deployment Script (Windows PowerShell)
# Runs both the Zentra application installer and the datacenter simulation
# setup in sequence, then displays a combined summary.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File deploy-all.ps1
#
# What this does:
#   1. Installs/updates Zentra (Docker container with Oracle client)
#   2. Deploys the datacenter simulation (12 simulated nodes)
#   3. Displays a combined summary of all services
#
# Prerequisites: Docker Desktop must be installed and running.
# =============================================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Cyan }
function Write-Ok    { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn  { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err   { Write-Host "[ERROR] $args" -ForegroundColor Red }

$startTime = Get-Date

Write-Host ""
Write-Host "======================================================" -ForegroundColor White
Write-Host "  Zentra - Full Deployment (App + Simulation)"          -ForegroundColor White
Write-Host "======================================================" -ForegroundColor White
Write-Host ""

# ---------------------------------------------------------------------------
# Step 1: Check Docker
# ---------------------------------------------------------------------------
Write-Info "Verifying Docker is available..."

try {
    docker --version >$null 2>$null
    if ($LASTEXITCODE -ne 0) { throw "Docker not found" }
    docker info >$null 2>$null
    if ($LASTEXITCODE -ne 0) { throw "Docker not running" }
} catch {
    Write-Err "Docker Desktop is not installed or not running."
    Write-Host "  Install from: https://www.docker.com/products/docker-desktop"
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Ok "Docker is ready"

# ---------------------------------------------------------------------------
# Step 2: Run Zentra Application Installer
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Phase 1/2: Zentra Application"                        -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

$installScript = Join-Path $ScriptDir "install.ps1"
if (-not (Test-Path $installScript)) {
    Write-Err "install.ps1 not found at: $installScript"
    Read-Host "Press Enter to exit"
    exit 1
}

$installSuccess = $true
try {
    & $installScript -NoWait
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        $installSuccess = $false
    }
} catch {
    Write-Warn "Zentra install encountered an issue: $_"
    $installSuccess = $false
}

if ($installSuccess) {
    Write-Ok "Zentra application deployment complete"
} else {
    Write-Warn "Zentra application deployment had issues - continuing with simulation..."
}

Start-Sleep -Seconds 3

# ---------------------------------------------------------------------------
# Step 3: Run Datacenter Simulation Setup
# ---------------------------------------------------------------------------
Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Phase 2/2: Datacenter Simulation"                     -ForegroundColor Cyan
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

$simScript = Join-Path $ScriptDir "simulation\setup.ps1"
if (-not (Test-Path $simScript)) {
    Write-Err "simulation\setup.ps1 not found at: $simScript"
    Read-Host "Press Enter to exit"
    exit 1
}

$simSuccess = $true
try {
    & $simScript -NoWait
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        $simSuccess = $false
    }
} catch {
    Write-Warn "Simulation setup encountered an issue: $_"
    $simSuccess = $false
}

if ($simSuccess) {
    Write-Ok "Datacenter simulation deployment complete"
} else {
    Write-Warn "Simulation deployment had issues"
}

# ---------------------------------------------------------------------------
# Step 4: Ensure SSH tools are available in the Zentra container
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Checking SSH tools in Zentra container..."

# Suppress stderr from docker exec calls (GPG key imports, SSH warnings, test failures)
# $ErrorActionPreference = "Stop" treats any native stderr as a terminating error
$prevEAP = $ErrorActionPreference
$ErrorActionPreference = "SilentlyContinue"

# Check if sshpass is available
docker exec zentra which sshpass >$null 2>$null
if ($LASTEXITCODE -ne 0) {
    Write-Info "Installing openssh-clients and sshpass in Zentra container..."
    docker exec zentra dnf install -y openssh-clients sshpass 2>&1 | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "SSH tools installed successfully"
    } else {
        Write-Warn "Failed to install SSH tools"
    }
} else {
    Write-Ok "SSH tools already available"
}

# Generate SSH keys if missing (or regenerate if passphrase-protected)
# NOTE: zentra-server runs as the 'zentra' user, so keys must live under
# /opt/zentra/.ssh so the app can find and use them.
$ZENTRA_SSH_DIR = "/opt/zentra/.ssh"
$ZENTRA_KEY = "$ZENTRA_SSH_DIR/id_rsa"
docker exec zentra test -f $ZENTRA_KEY >$null 2>$null
$keyExists = ($LASTEXITCODE -eq 0)
if ($keyExists) {
    # Verify key has no passphrase (can be loaded without TTY)
    docker exec zentra ssh-keygen -y -f $ZENTRA_KEY -P "" >$null 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Existing SSH key has a passphrase - regenerating..."
        docker exec zentra rm -f $ZENTRA_KEY "$ZENTRA_KEY.pub" >$null 2>$null
        $keyExists = $false
    } else {
        Write-Ok "SSH key pair already exists (no passphrase)"
    }
}

if (-not $keyExists) {
    Write-Info "Generating SSH key pair for zentra user..."
    docker exec zentra mkdir -p $ZENTRA_SSH_DIR
    docker exec zentra chmod 700 $ZENTRA_SSH_DIR
    docker exec zentra ssh-keygen -t rsa -b 4096 -f $ZENTRA_KEY -N "" -q
    docker exec zentra chown -R zentra:zentra $ZENTRA_SSH_DIR
    docker exec zentra sh -c "echo 'Host *' > $ZENTRA_SSH_DIR/config"
    docker exec zentra sh -c "echo '    StrictHostKeyChecking no' >> $ZENTRA_SSH_DIR/config"
    docker exec zentra sh -c "echo '    UserKnownHostsFile /dev/null' >> $ZENTRA_SSH_DIR/config"
    docker exec zentra sh -c "echo '    LogLevel ERROR' >> $ZENTRA_SSH_DIR/config"
    docker exec zentra sh -c "echo '    GSSAPIAuthentication no' >> $ZENTRA_SSH_DIR/config"
    docker exec zentra chmod 600 $ZENTRA_SSH_DIR/config
    docker exec zentra chown -R zentra:zentra $ZENTRA_SSH_DIR
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "SSH key pair generated at $ZENTRA_KEY"
    } else {
        Write-Warn "Failed to generate SSH key pair"
    }
}

# ---------------------------------------------------------------------------
# Step 5: Connect Zentra to the simulation network
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Connecting Zentra container to simulation network..."

# Detect the simulation network name dynamically
$simNetwork = ""
$netFmt = '{{.Name}}'
$netList = docker network ls --format $netFmt 2>$null
if ($netList) {
    foreach ($line in $netList) {
        if ($line -match "zentra-dc") {
            $simNetwork = $line.Trim()
            break
        }
    }
}

if (-not $simNetwork) {
    Write-Warn "Simulation network not found. Is the simulation running?"
} else {
    Write-Info "Found simulation network: $simNetwork"

    # Try to connect (will fail harmlessly if already connected)
    docker network connect $simNetwork zentra >$null 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "Zentra connected to simulation network"
    } else {
        # Already connected or other issue - check if reachable
        Write-Ok "Zentra is already connected to $simNetwork"
    }

    # Verify connectivity
    Write-Info "Verifying connectivity from Zentra to simulation nodes..."
    $firstSimIp = Get-SimIp -Name "srv-linux-01"
    if ($firstSimIp -ne "N/A") {
        docker exec zentra ping -c 1 -W 2 $firstSimIp >$null 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Ok "Zentra can reach simulation nodes (ping $firstSimIp OK)"
        } else {
            Write-Warn "Ping to $firstSimIp failed from Zentra container"
        }
    } else {
        Write-Warn "Could not get IP for srv-linux-01 - skipping ping verification"
    }
}

# Restore ErrorActionPreference after all docker exec blocks
$ErrorActionPreference = $prevEAP

# ---------------------------------------------------------------------------
# Step 6: Distribute SSH keys to simulation nodes
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Distributing SSH keys to simulation nodes..."

$sshTargets = @(
    "srv-linux-01", "srv-linux-02", "srv-win-01", "srv-win-02",
    "sw-core-01", "sw-access-01", "fw-01", "stor-01",
    "mw-nginx-01", "mw-apache-01", "mw-tomcat-01", "db-oracle-01"
)

$keyCount = 0
$prevEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
foreach ($target in $sshTargets) {
    $ipFmt = '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}'
    $targetIp = docker inspect --format $ipFmt $target 2>$null
    if ($targetIp -and $targetIp -match '^\d+\.\d+\.\d+\.\d+$') {
        docker exec zentra sshpass -p "zentra123" ssh-copy-id -i $ZENTRA_KEY.pub -o StrictHostKeyChecking=no -o GSSAPIAuthentication=no root@$targetIp >$null 2>$null
        if ($LASTEXITCODE -eq 0) {
            $keyCount++
        }
    }
}
$ErrorActionPreference = $prevEAP

if ($keyCount -gt 0) {
    Write-Ok "SSH keys distributed to $keyCount nodes"
} else {
    Write-Warn "Could not distribute SSH keys to any nodes"
}

# Verify SSH works without password
Write-Info "Verifying passwordless SSH..."
$firstSimIp = Get-SimIp -Name "srv-linux-01"
if ($firstSimIp -ne "N/A") {
    $prevEAP2 = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $sshResult = docker exec zentra ssh -i $ZENTRA_KEY -o BatchMode=yes -o ConnectTimeout=5 -o GSSAPIAuthentication=no root@$firstSimIp hostname 2>$null
    $ErrorActionPreference = $prevEAP2
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "Passwordless SSH verified (srv-linux-01: $sshResult)"
    } else {
        Write-Warn "Passwordless SSH verification failed - try manually: docker exec zentra sshpass -p zentra123 ssh-copy-id root@$firstSimIp"
    }
} else {
    Write-Warn "Could not get IP for srv-linux-01 - skipping SSH verification"
}

# ---------------------------------------------------------------------------
# Step 7: Combined Summary
# ---------------------------------------------------------------------------
$elapsed = (Get-Date) - $startTime
$elapsedStr = "{0:mm\:ss}" -f $elapsed

# Detect IPs
try {
    $serverIP = (Get-NetIPAddress -AddressFamily IPv4 |
        Where-Object { $_.IPAddress -ne "127.0.0.1" -and $_.PrefixOrigin -ne "WellKnown" } |
        Select-Object -First 1).IPAddress
} catch { $serverIP = "localhost" }
if (-not $serverIP) { $serverIP = "localhost" }

# Get simulation container IPs
function Get-SimIp {
    param([string]$Name)
    try {
        $fmt = '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}'
        $ip = docker inspect --format $fmt $Name 2>$null
        if ($LASTEXITCODE -eq 0 -and $ip -match '^\d+\.\d+\.\d+\.\d+$') { return $ip }
    } catch {}
    return "N/A"
}

Write-Host ""
Write-Host ""
Write-Host "======================================================================" -ForegroundColor Green
Write-Host "  Zentra Full Deployment - Summary                   [$elapsedStr]"    -ForegroundColor Green
Write-Host "======================================================================" -ForegroundColor Green
Write-Host ""

$zentraStatus = if ($installSuccess) { "Running" } else { "Check Needed" }
$zentraColor  = if ($installSuccess) { "Green" } else { "Yellow" }

Write-Host "  ZENTRA APPLICATION" -ForegroundColor Cyan
Write-Host "  ------------------"
Write-Host "  Status:       $zentraStatus" -ForegroundColor $zentraColor
Write-Host "  Web UI:       http://${serverIP}:5010"
Write-Host "  Login:        admin@Zentra.local / changeme"
Write-Host ""

# Simulation section
Write-Host '  DATACENTER SIMULATION (12 Nodes)' -ForegroundColor Cyan
Write-Host "  ---------------------------------"

$simNodes = @(
    @{ Name = "srv-linux-01";  Ports = "22,161";   Desc = "Rocky Linux 8 (Web Server)" },
    @{ Name = "srv-linux-02";  Ports = "22,161";   Desc = "Ubuntu 22.04 (App Server)" },
    @{ Name = "srv-win-01";    Ports = "22,161";     Desc = "Windows Server 2022 (AD/DNS)" },
    @{ Name = "srv-win-02";    Ports = "22,161";     Desc = "Windows Server 2019 (Files)" },
    @{ Name = "sw-core-01";    Ports = "22,161";     Desc = "Cisco Catalyst 9300 (Core)" },
    @{ Name = "sw-access-01";  Ports = "22,161";     Desc = "Arista 7050X (Access)" },
    @{ Name = "fw-01";         Ports = "22,161";     Desc = "Palo Alto PA-3260 (Firewall)" },
    @{ Name = "stor-01";       Ports = "22,161";     Desc = "Dell PowerStore 500T (SAN)" },
    @{ Name = "mw-nginx-01";   Ports = "22,80,8080"; Desc = "Nginx (Reverse Proxy)" },
    @{ Name = "mw-apache-01";  Ports = "22,80,8080"; Desc = "Apache HTTP Server" },
    @{ Name = "mw-tomcat-01";  Ports = "22,8080";    Desc = "Apache Tomcat 9" },
    @{ Name = "db-oracle-01";  Ports = "22,1521";    Desc = "Oracle 21c XE (ZENTRADB)" }
)

Write-Host ""
Write-Host ("{0,-18} {1,-16} {2,-10} {3,-8} {4}" -f "  COMPONENT","IP ADDRESS","PORT(S)","STATUS","DESCRIPTION")
Write-Host ("{0,-18} {1,-16} {2,-10} {3,-8} {4}" -f "  -----------------","----------------","----------","--------","----------------------------")

$runningCount = 0
foreach ($node in $simNodes) {
    $ip = Get-SimIp -Name $node.Name
    $isRunning = $false
    try {
        $fmt2 = '{{.State.Running}}'
        $state = docker inspect --format $fmt2 $node.Name 2>$null
        if ($state -eq "true") { $isRunning = $true }
    } catch {}

    $status = if ($isRunning) { "UP" } else { "DOWN" }
    $color  = if ($isRunning) { "Green" } else { "Red" }
    if ($isRunning) { $runningCount++ }

    $line = "{0,-18} {1,-16} {2,-10} {3,-8} {4}" -f "  $($node.Name)", $ip, $node.Ports, $status, $node.Desc
    Write-Host $line -ForegroundColor $color
}

Write-Host ""
if ($runningCount -ge 12) {
    Write-Ok "All $runningCount/12 simulation nodes are running"
} else {
    Write-Warn "$runningCount/12 simulation nodes running"
}

Write-Host ""
Write-Host "  IMPORTANT: Change the default admin password after first login!" -ForegroundColor Yellow
Write-Host ""
Write-Host "======================================================================" -ForegroundColor Green
Write-Host ""

Read-Host "Press Enter to close"
