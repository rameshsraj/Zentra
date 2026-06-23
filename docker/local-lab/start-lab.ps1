# =============================================================================
# Zentra — Local Lab Launcher (Windows)
# =============================================================================
# Spins up 3 Linux containers + 1 zentra-agent on Docker Desktop.
# The agent connects to your GCP Zentra server.
#
# Prerequisites:
#   - Docker Desktop running
#   - zentra-agent image loaded (docker load -i zentra-agent-latest.tar.gz)
#   - Agent API key from Zentra UI (see instructions below)
#
# Usage:
#   .\start-lab.ps1 -ApiEndpoint "http://<GCP_IP>:5010" -AgentApiKey "isok_xxxxx"
# =============================================================================

param(
    [string]$ApiEndpoint = $env:API_ENDPOINT,
    [string]$AgentApiKey = $env:AGENT_API_KEY,
    [string]$DcName      = $(if ($env:DC_NAME) { $env:DC_NAME } else { "dc-local-lab" }),
    [string]$AgentTarball = "zentra-agent-latest.tar.gz"
)

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra — Local Lab Setup"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

# ─────────────────────────────────────────────────────────────────────
# Step 0: Check Docker
# ─────────────────────────────────────────────────────────────────────
Write-Host "[1/5] Checking Docker..." -ForegroundColor Yellow
try {
    docker info 2>$null | Out-Null
} catch {
    Write-Host "[ERROR] Docker is not running. Start Docker Desktop first." -ForegroundColor Red
    exit 1
}
Write-Host "  Docker is running." -ForegroundColor Green

# ─────────────────────────────────────────────────────────────────────
# Step 1: Load agent image if needed
# ─────────────────────────────────────────────────────────────────────
Write-Host "[2/5] Checking zentra-agent image..." -ForegroundColor Yellow
$agentImage = docker images -q "zentra-agent:latest" 2>$null
if (-not $agentImage) {
    if (Test-Path $AgentTarball) {
        Write-Host "  Loading from $AgentTarball..."
        docker load -i $AgentTarball
    } elseif (Test-Path "$PSScriptRoot\zentra-agent-latest.tar.gz") {
        Write-Host "  Loading from $PSScriptRoot\zentra-agent-latest.tar.gz..."
        docker load -i "$PSScriptRoot\zentra-agent-latest.tar.gz"
    } else {
        Write-Host "[ERROR] zentra-agent image not found. Load it first:" -ForegroundColor Red
        Write-Host "  docker load -i zentra-agent-latest.tar.gz" -ForegroundColor Yellow
        exit 1
    }
}
Write-Host "  zentra-agent:latest image ready." -ForegroundColor Green

# ─────────────────────────────────────────────────────────────────────
# Step 2: Get configuration
# ─────────────────────────────────────────────────────────────────────
Write-Host "[3/5] Configuration..." -ForegroundColor Yellow

if (-not $ApiEndpoint) {
    Write-Host ""
    Write-Host "  Enter your GCP Zentra server URL." -ForegroundColor White
    Write-Host "  Example: http://34.68.222.94:5010" -ForegroundColor DarkGray
    $ApiEndpoint = Read-Host "  API Endpoint"
}
if (-not $ApiEndpoint) {
    Write-Host "[ERROR] API endpoint is required." -ForegroundColor Red
    exit 1
}

if (-not $AgentApiKey) {
    Write-Host ""
    Write-Host "  ┌─────────────────────────────────────────────────────────────┐" -ForegroundColor DarkGray
    Write-Host "  │ HOW TO GET AN AGENT API KEY:                                │" -ForegroundColor DarkGray
    Write-Host "  │                                                             │" -ForegroundColor DarkGray
    Write-Host "  │ Option A — Zentra UI:                                       │" -ForegroundColor DarkGray
    Write-Host "  │   1. Open $ApiEndpoint in your browser" -ForegroundColor DarkGray
    Write-Host "  │   2. Login as admin                                         │" -ForegroundColor DarkGray
    Write-Host "  │   3. Go to Fleet > Install Agent                            │" -ForegroundColor DarkGray
    Write-Host "  │   4. Enter datacenter name, click Register                  │" -ForegroundColor DarkGray
    Write-Host "  │   5. Copy the isok_xxx key                                  │" -ForegroundColor DarkGray
    Write-Host "  │                                                             │" -ForegroundColor DarkGray
    Write-Host "  │ Option B — curl:                                            │" -ForegroundColor DarkGray
    Write-Host "  │   LOGIN:                                                    │" -ForegroundColor DarkGray
    Write-Host "  │   curl -s $ApiEndpoint/api/auth/login \" -ForegroundColor DarkGray
    Write-Host "  │     -H 'Content-Type: application/json' \                   │" -ForegroundColor DarkGray
    Write-Host "  │     -d '{`"email`":`"admin@Zentra.local`",`"password`":`"changeme`"}'│" -ForegroundColor DarkGray
    Write-Host "  │   # Copy the token from the response                        │" -ForegroundColor DarkGray
    Write-Host "  │                                                             │" -ForegroundColor DarkGray
    Write-Host "  │   REGISTER:                                                 │" -ForegroundColor DarkGray
    Write-Host "  │   curl -s $ApiEndpoint/api/agents/register \" -ForegroundColor DarkGray
    Write-Host "  │     -H 'Authorization: Bearer <TOKEN>' \                    │" -ForegroundColor DarkGray
    Write-Host "  │     -H 'Content-Type: application/json' \                   │" -ForegroundColor DarkGray
    Write-Host "  │     -d '{`"datacenter_name`":`"dc-local-lab`"}'                │" -ForegroundColor DarkGray
    Write-Host "  │   # Copy the api_key (starts with isok_)                    │" -ForegroundColor DarkGray
    Write-Host "  └─────────────────────────────────────────────────────────────┘" -ForegroundColor DarkGray
    Write-Host ""
    $AgentApiKey = Read-Host "  Agent API Key (isok_...)"
}
if (-not $AgentApiKey) {
    Write-Host "[ERROR] Agent API key is required." -ForegroundColor Red
    exit 1
}

Write-Host "  Endpoint: $ApiEndpoint" -ForegroundColor White
Write-Host "  DC Name:  $DcName" -ForegroundColor White
Write-Host "  Key:      $($AgentApiKey.Substring(0, [Math]::Min(13, $AgentApiKey.Length)))..." -ForegroundColor White

# ─────────────────────────────────────────────────────────────────────
# Step 3: Create .env for docker compose
# ─────────────────────────────────────────────────────────────────────
Write-Host "[4/5] Starting lab containers..." -ForegroundColor Yellow

$env:API_ENDPOINT = $ApiEndpoint
$env:AGENT_API_KEY = $AgentApiKey
$env:DC_NAME = $DcName

# Stop any existing lab
docker compose down 2>$null | Out-Null

# Start the lab
docker compose up -d

Start-Sleep -Seconds 5

# ─────────────────────────────────────────────────────────────────────
# Step 4: Verify
# ─────────────────────────────────────────────────────────────────────
Write-Host "[5/5] Verifying containers..." -ForegroundColor Yellow
Write-Host ""

$containers = @("lab-linux-01", "lab-nginx-01", "lab-httpd-01", "zentra-agent")
foreach ($c in $containers) {
    $status = docker inspect -f '{{.State.Status}}' $c 2>$null
    if ($status -eq "running") {
        Write-Host "  $c — RUNNING" -ForegroundColor Green
    } else {
        Write-Host "  $c — $status" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra Local Lab — Started!"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  CONTAINERS:" -ForegroundColor White
Write-Host "    lab-linux-01  (172.30.0.10) — Rocky Linux + SSH + SNMP"
Write-Host "    lab-nginx-01  (172.30.0.11) — Alpine + Nginx + SSH"
Write-Host "    lab-httpd-01  (172.30.0.12) — Alpine + Apache + SSH"
Write-Host "    zentra-agent  (172.30.0.2)  — Collects & pushes to GCP"
Write-Host ""
Write-Host "  GCP SERVER:  $ApiEndpoint" -ForegroundColor White
Write-Host "  DC NAME:     $DcName" -ForegroundColor White
Write-Host ""
Write-Host "  COMMANDS:" -ForegroundColor White
Write-Host "    Agent logs:     docker logs -f zentra-agent"
Write-Host "    All logs:       docker compose logs -f"
Write-Host "    Stop lab:       docker compose down"
Write-Host "    Restart:        docker compose restart"
Write-Host "    Shell into:     docker exec -it lab-linux-01 bash"
Write-Host ""
Write-Host "  NEXT STEPS:" -ForegroundColor White
Write-Host "    1. Open $ApiEndpoint in your browser"
Write-Host "    2. Go to Fleet — agent should appear as '$DcName'"
Write-Host "    3. Check Targets — 3 hosts should be discovered"
Write-Host "    4. View metrics on dashboards"
Write-Host "======================================================"
Write-Host ""
