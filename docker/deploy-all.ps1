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

# Get simulation container IP by container name
# Uses JSON output to correctly handle containers on multiple networks
function Get-SimIp {
    param([string]$Name)
    try {
        $json = docker inspect $Name 2>$null | ConvertFrom-Json
        if ($LASTEXITCODE -eq 0 -and $json) {
            $networks = $json[0].NetworkSettings.Networks
            foreach ($net in $networks.PSObject.Properties) {
                $ip = $net.Value.IPAddress
                if ($ip -match '^\d+\.\d+\.\d+\.\d+$') { return $ip }
            }
        }
    } catch {}
    return "N/A"
}

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
# Step 2: Clean up existing containers
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Stopping and removing existing containers..."

$allContainers = @(
    "zentra",
    "srv-linux-01", "srv-linux-02", "srv-win-01", "srv-win-02",
    "sw-core-01", "sw-access-01", "fw-01", "stor-01",
    "mw-nginx-01", "mw-apache-01", "mw-tomcat-01", "db-oracle-01"
)

$removedCount = 0
foreach ($ctr in $allContainers) {
    $exists = docker ps -aq -f "name=^${ctr}$" 2>$null
    if ($exists) {
        docker stop $ctr >$null 2>$null
        docker rm -f $ctr >$null 2>$null
        $removedCount++
    }
}

if ($removedCount -gt 0) {
    Write-Ok "Removed $removedCount existing container(s)"
} else {
    Write-Ok "No existing containers to clean up"
}

# ---------------------------------------------------------------------------
# Step 3: Run Zentra Application Installer
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
# Step 4: Run Datacenter Simulation Setup
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
# Step 5: Ensure SSH tools are available in the Zentra container
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

# Generate SSH keys for root user (force regenerate to ensure no passphrase)
$ROOT_SSH_DIR = "/root/.ssh"
$ROOT_KEY = "$ROOT_SSH_DIR/id_rsa"
Write-Info "Regenerating SSH key pair for root (ensuring no passphrase)..."
docker exec zentra rm -f $ROOT_KEY "$ROOT_KEY.pub" >$null 2>$null
docker exec zentra mkdir -p $ROOT_SSH_DIR
docker exec zentra chmod 700 $ROOT_SSH_DIR
docker exec zentra ssh-keygen -t rsa -b 4096 -f $ROOT_KEY -N "" -q
docker exec zentra sh -c "echo 'Host *' > $ROOT_SSH_DIR/config"
docker exec zentra sh -c "echo '    StrictHostKeyChecking no' >> $ROOT_SSH_DIR/config"
docker exec zentra sh -c "echo '    UserKnownHostsFile /dev/null' >> $ROOT_SSH_DIR/config"
docker exec zentra sh -c "echo '    LogLevel ERROR' >> $ROOT_SSH_DIR/config"
docker exec zentra sh -c "echo '    GSSAPIAuthentication no' >> $ROOT_SSH_DIR/config"
docker exec zentra chmod 600 $ROOT_SSH_DIR/config
if ($LASTEXITCODE -eq 0) {
    Write-Ok "SSH key pair generated at $ROOT_KEY"
} else {
    Write-Warn "Failed to generate SSH key pair"
}

# ---------------------------------------------------------------------------
# Step 6: Verify connectivity to simulation nodes (host network)
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Verifying connectivity from Zentra to simulation nodes..."
Write-Info "Zentra uses host network - can reach simulation containers directly"

$firstSimIp = Get-SimIp -Name "srv-linux-01"
if ($firstSimIp -ne "N/A") {
    $pingOk = $false
    for ($attempt = 1; $attempt -le 5; $attempt++) {
        docker exec zentra ping -c 1 -W 2 $firstSimIp >$null 2>$null
        if ($LASTEXITCODE -eq 0) {
            $pingOk = $true
            break
        }
        Start-Sleep -Seconds 2
    }
    if ($pingOk) {
        Write-Ok "Zentra can reach simulation nodes (ping $firstSimIp OK)"
    } else {
        Write-Warn "Ping to $firstSimIp failed from Zentra container"
        Write-Warn "  Debug: docker exec zentra ip addr"
    }
} else {
    Write-Warn "Could not get IP for srv-linux-01 - skipping ping verification"
}

# Restore ErrorActionPreference after all docker exec blocks
$ErrorActionPreference = $prevEAP

# ---------------------------------------------------------------------------
# Step 7: Wait for SSH readiness on simulation nodes
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Waiting for SSH daemons to start on simulation nodes..."

$sshTargets = @(
    "srv-linux-01", "srv-linux-02", "srv-win-01", "srv-win-02",
    "sw-core-01", "sw-access-01", "fw-01", "stor-01",
    "mw-nginx-01", "mw-apache-01", "mw-tomcat-01", "db-oracle-01"
)

# Wait up to 30 seconds for SSH to come up on at least the first node
$sshReady = $false
for ($wait = 1; $wait -le 10; $wait++) {
    $firstSimIp = Get-SimIp -Name "srv-linux-01"
    if ($firstSimIp -ne "N/A") {
        docker exec zentra sh -c "echo > /dev/tcp/$firstSimIp/22" >$null 2>$null
        if ($LASTEXITCODE -eq 0) {
            $sshReady = $true
            break
        }
    }
    Write-Host "." -NoNewline
    Start-Sleep -Seconds 3
}
Write-Host ""

if ($sshReady) {
    Write-Ok "SSH is ready on simulation nodes"
} else {
    Write-Warn "SSH may not be ready on all nodes yet - will attempt key distribution anyway"
}

# Extra wait for all containers to stabilize
Start-Sleep -Seconds 5

# ---------------------------------------------------------------------------
# Step 8: Distribute SSH keys to simulation nodes
# ---------------------------------------------------------------------------
Write-Host ""
Write-Info "Distributing SSH keys to simulation nodes..."

$keyCount = 0
$failedNodes = @()
$prevEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
foreach ($target in $sshTargets) {
    $targetIp = Get-SimIp -Name $target
    if ($targetIp -ne "N/A") {
        $copyOk = $false
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            # Direct key push (ssh-copy-id is unreliable in non-interactive Docker)
            $shCmd = "sshpass -p zentra123 ssh -o StrictHostKeyChecking=no -o GSSAPIAuthentication=no -o ConnectTimeout=5 root@${targetIp} 'mkdir -p /root/.ssh && chmod 700 /root/.ssh && cat >> /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys' < ${ROOT_KEY}.pub"
            docker exec zentra sh -c "$shCmd" >$null 2>$null
            if ($LASTEXITCODE -eq 0) {
                # Verify key was actually installed
                docker exec zentra timeout 5 ssh -i $ROOT_KEY -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o GSSAPIAuthentication=no "root@${targetIp}" "echo ok" >$null 2>$null
                if ($LASTEXITCODE -eq 0) {
                    $copyOk = $true
                    break
                }
            }
            Start-Sleep -Seconds 2
        }
        if ($copyOk) {
            $keyCount++
        } else {
            $failedNodes += "$target ($targetIp)"
        }
    } else {
        $failedNodes += "$target (no IP)"
    }
}
$ErrorActionPreference = $prevEAP

if ($keyCount -gt 0) {
    Write-Ok "SSH keys distributed to $keyCount/$($sshTargets.Count) nodes"
}
if ($failedNodes.Count -gt 0) {
    Write-Warn "Failed nodes: $($failedNodes -join ', ')"
}

# Verify SSH works without password
Write-Info "Verifying passwordless SSH..."
$firstSimIp = Get-SimIp -Name "srv-linux-01"
if ($firstSimIp -ne "N/A") {
    $prevEAP2 = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $verifyOk = $false
    $lastError = ""
    for ($attempt = 1; $attempt -le 3; $attempt++) {
        $sshResult = docker exec zentra ssh -i $ROOT_KEY -o BatchMode=yes -o ConnectTimeout=5 -o GSSAPIAuthentication=no -o StrictHostKeyChecking=no "root@${firstSimIp}" hostname 2>&1
        $lastError = $sshResult
        if ($LASTEXITCODE -eq 0) {
            $verifyOk = $true
            break
        }
        Write-Info "  SSH attempt $attempt/3 failed, retrying..."
        Start-Sleep -Seconds 2
    }
    $ErrorActionPreference = $prevEAP2
    if ($verifyOk) {
        Write-Ok "Passwordless SSH verified (srv-linux-01: $sshResult)"
    } else {
        Write-Warn "Passwordless SSH verification failed"
        Write-Warn "  Last error: $lastError"
        Write-Warn "  Manual fix: docker exec zentra sh -c `"sshpass -p zentra123 ssh -o StrictHostKeyChecking=no root@$firstSimIp 'cat >> /root/.ssh/authorized_keys' `< /root/.ssh/id_rsa.pub`""
    }
} else {
    Write-Warn "Could not get IP for srv-linux-01 - skipping SSH verification"
}

# ---------------------------------------------------------------------------
# Step 9: Combined Summary
# ---------------------------------------------------------------------------
$elapsed = (Get-Date) - $startTime
$elapsedStr = "{0:mm\:ss}" -f $elapsed

# Detect IPs — exclude Docker/WSL virtual adapters (172.16-31.x.x, 10.x.x.x) and APIPA
try {
    $serverIP = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object {
        $_.IPAddress -ne "127.0.0.1" -and
        $_.PrefixOrigin -ne "WellKnown" -and
        $_.IPAddress -notmatch '^172\.(1[6-9]|2[0-9]|3[01])\.' -and
        $_.IPAddress -notmatch '^10\.' -and
        $_.IPAddress -notmatch '^169\.254\.'
    } | Select-Object -First 1).IPAddress
} catch { $serverIP = "localhost" }
if (-not $serverIP) { $serverIP = "localhost" }


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
