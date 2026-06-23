# =============================================================================
# Zentra - Local Lab Launcher (Windows)
# =============================================================================
# Spins up 3 Linux containers + 1 zentra-agent on Docker Desktop.
# The agent connects to your GCP Zentra server.
#
# Prerequisites:
#   - Docker Desktop running
#   - zentra-agent-latest.tar.gz in this folder
#   - Agent API key from Zentra UI (starts with isok_)
#
# Usage:
#   .\start-lab.ps1 -ApiEndpoint "http://<GCP_IP>:5010" -AgentApiKey "isok_xxxxx"
# =============================================================================

param(
    [string]$ApiEndpoint  = $env:API_ENDPOINT,
    [string]$AgentApiKey  = $env:AGENT_API_KEY,
    [string]$DcName       = $(if ($env:DC_NAME) { $env:DC_NAME } else { "dc-local-lab" }),
    [string]$AgentTarball = "zentra-agent-latest.tar.gz"
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra - Local Lab Setup"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

# -----------------------------------------------------------------------------
# Step 1 - Docker check
# -----------------------------------------------------------------------------
Write-Host "[1/5] Checking Docker..." -ForegroundColor Yellow
try {
    docker info 2>$null | Out-Null
} catch {
    Write-Host "[ERROR] Docker is not running. Start Docker Desktop first." -ForegroundColor Red
    exit 1
}
Write-Host "  Docker is running." -ForegroundColor Green

# -----------------------------------------------------------------------------
# Step 2 - Load agent image
# -----------------------------------------------------------------------------
Write-Host "[2/5] Checking zentra-agent image..." -ForegroundColor Yellow
$agentImage = docker images -q "zentra-agent:latest" 2>$null
if (-not $agentImage) {
    $tarPath = $null
    if (Test-Path $AgentTarball)                                      { $tarPath = $AgentTarball }
    elseif (Test-Path "$PSScriptRoot\zentra-agent-latest.tar.gz")     { $tarPath = "$PSScriptRoot\zentra-agent-latest.tar.gz" }

    if ($tarPath) {
        Write-Host "  Loading from $tarPath ..."
        docker load -i $tarPath
    } else {
        Write-Host "[ERROR] zentra-agent-latest.tar.gz not found in this folder." -ForegroundColor Red
        Write-Host "        Copy it from the build server first." -ForegroundColor Red
        exit 1
    }
}
Write-Host "  zentra-agent:latest ready." -ForegroundColor Green

# -----------------------------------------------------------------------------
# Step 3 - Configuration
# -----------------------------------------------------------------------------
Write-Host "[3/5] Configuration..." -ForegroundColor Yellow

if (-not $ApiEndpoint) {
    Write-Host ""
    Write-Host "  Enter your Zentra server URL (example: http://34.68.222.94:5010)"
    $ApiEndpoint = Read-Host "  API Endpoint"
}
if (-not $ApiEndpoint) {
    Write-Host "[ERROR] API endpoint is required." -ForegroundColor Red
    exit 1
}

if (-not $AgentApiKey) {
    Write-Host ""
    Write-Host "  Get an Agent API key from $ApiEndpoint -> Fleet -> Register Agent."
    Write-Host "  The key starts with 'isok_'."
    $AgentApiKey = Read-Host "  Agent API Key"
}
if (-not $AgentApiKey) {
    Write-Host "[ERROR] Agent API key is required." -ForegroundColor Red
    exit 1
}

$keyPreview = $AgentApiKey.Substring(0, [Math]::Min(13, $AgentApiKey.Length))
Write-Host "  Endpoint: $ApiEndpoint" -ForegroundColor White
Write-Host "  DC Name:  $DcName"      -ForegroundColor White
Write-Host "  Key:      $keyPreview..." -ForegroundColor White

# -----------------------------------------------------------------------------
# Step 4 - Start containers
# -----------------------------------------------------------------------------
Write-Host "[4/5] Starting lab containers..." -ForegroundColor Yellow

$env:API_ENDPOINT  = $ApiEndpoint
$env:AGENT_API_KEY = $AgentApiKey
$env:DC_NAME       = $DcName

docker compose down 2>$null | Out-Null
docker compose up -d

Start-Sleep -Seconds 5

# -----------------------------------------------------------------------------
# Step 5 - Verify
# -----------------------------------------------------------------------------
Write-Host "[5/5] Verifying containers..." -ForegroundColor Yellow
Write-Host ""

$containers = @("lab-linux-01", "lab-nginx-01", "lab-httpd-01", "zentra-agent")
foreach ($c in $containers) {
    $status = docker inspect -f '{{.State.Status}}' $c 2>$null
    if ($status -eq "running") {
        Write-Host "  $c : RUNNING" -ForegroundColor Green
    } else {
        Write-Host "  $c : $status" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra Local Lab - Started!"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  CONTAINERS:" -ForegroundColor White
Write-Host "    lab-linux-01  (172.30.0.10)  Rocky Linux + SSH + SNMP"
Write-Host "    lab-nginx-01  (172.30.0.11)  Alpine + Nginx + SSH"
Write-Host "    lab-httpd-01  (172.30.0.12)  Alpine + Apache + SSH"
Write-Host "    zentra-agent  (172.30.0.2)   Collects and pushes to GCP"
Write-Host ""
Write-Host "  GCP SERVER:  $ApiEndpoint" -ForegroundColor White
Write-Host "  DC NAME:     $DcName"      -ForegroundColor White
Write-Host ""
Write-Host "  COMMANDS:" -ForegroundColor White
Write-Host "    Agent logs:   docker logs -f zentra-agent"
Write-Host "    All logs:     docker compose logs -f"
Write-Host "    Stop lab:     docker compose down"
Write-Host "    Restart:      docker compose restart"
Write-Host "    Shell:        docker exec -it lab-linux-01 bash"
Write-Host ""
Write-Host "  NEXT STEPS:" -ForegroundColor White
Write-Host "    1. Open $ApiEndpoint in your browser"
Write-Host "    2. Go to Fleet - the agent should appear as '$DcName'"
Write-Host "    3. Check Infrastructure -> Servers for the 3 hosts"
Write-Host "    4. View metrics on dashboards"
Write-Host "======================================================"
Write-Host ""
