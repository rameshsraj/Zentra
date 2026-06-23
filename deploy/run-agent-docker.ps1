# =============================================================================
# Zentra Agent — Docker Run Script (Windows)
# =============================================================================
# Loads the pre-built zentra-agent Docker image from tarball and runs it
# connected to a remote Zentra server (e.g. GCP).
#
# Prerequisites:
#   - Docker Desktop installed and running on Windows
#   - zentra-agent-latest.tar.gz in the same directory (from build server)
#
# Usage:
#   cd C:\zentra
#   .\run-agent-docker.ps1 -ApiEndpoint "http://34.68.222.94:5010" -AgentApiKey "isok_xxxxx"
#
# Or set environment variables:
#   $env:API_ENDPOINT = "http://34.68.222.94:5010"
#   $env:AGENT_API_KEY = "isok_xxxxx"
#   .\run-agent-docker.ps1
# =============================================================================

param(
    [string]$ApiEndpoint = $env:API_ENDPOINT,
    [string]$AgentApiKey = $env:AGENT_API_KEY,
    [string]$DcName      = $(if ($env:DC_NAME) { $env:DC_NAME } else { "dc-windows-01" }),
    [string]$Tarball     = "zentra-agent-latest.tar.gz"
)

$ErrorActionPreference = "Stop"

$IMAGE_NAME = "zentra-agent"
$IMAGE_TAG  = "latest"
$CONTAINER  = "zentra-agent"

# ─────────────────────────────────────────────────────────────────────────────
# Validate inputs
# ─────────────────────────────────────────────────────────────────────────────
if (-not $ApiEndpoint) {
    $ApiEndpoint = Read-Host "Zentra API endpoint (e.g. http://34.68.222.94:5010)"
}
if (-not $ApiEndpoint) {
    Write-Host "[ERROR] API endpoint is required." -ForegroundColor Red
    exit 1
}

if (-not $AgentApiKey) {
    $AgentApiKey = Read-Host "Agent API key (starts with isok_)"
}
if (-not $AgentApiKey) {
    Write-Host "[ERROR] Agent API key is required." -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra Agent — Docker Setup (Windows)"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  API endpoint:  $ApiEndpoint"
Write-Host "  DC name:       $DcName"
Write-Host "  Container:     $CONTAINER"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""

# ─────────────────────────────────────────────────────────────────────────────
# Check Docker
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "[1/4] Checking Docker..." -ForegroundColor Yellow
try {
    docker info | Out-Null
} catch {
    Write-Host "[ERROR] Docker is not running. Start Docker Desktop first." -ForegroundColor Red
    exit 1
}
Write-Host "  Docker is running." -ForegroundColor Green

# ─────────────────────────────────────────────────────────────────────────────
# Stop/remove old container
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "[2/4] Cleaning up old container..." -ForegroundColor Yellow
docker stop $CONTAINER 2>$null | Out-Null
docker rm -f $CONTAINER 2>$null | Out-Null
Write-Host "  Old container removed (if existed)." -ForegroundColor Green

# ─────────────────────────────────────────────────────────────────────────────
# Load image from tarball (if not already loaded)
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "[3/4] Loading agent image..." -ForegroundColor Yellow

$imageExists = docker images -q "${IMAGE_NAME}:${IMAGE_TAG}" 2>$null
if (-not $imageExists) {
    if (Test-Path $Tarball) {
        Write-Host "  Loading from $Tarball..."
        docker load -i $Tarball
        Write-Host "  Image loaded." -ForegroundColor Green
    } else {
        Write-Host "[ERROR] Image not found and tarball '$Tarball' not present." -ForegroundColor Red
        Write-Host "  Copy zentra-agent-latest.tar.gz from the build server first." -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "  Image already loaded." -ForegroundColor Green
}

# ─────────────────────────────────────────────────────────────────────────────
# Run the agent container
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "[4/4] Starting agent container..." -ForegroundColor Yellow

docker run -d `
    --name $CONTAINER `
    --restart unless-stopped `
    -e API_ENDPOINT="$ApiEndpoint" `
    -e AGENT_API_KEY="$AgentApiKey" `
    -e AGENT_MODE="remote" `
    -e DC_NAME="$DcName" `
    -e RUST_LOG="info" `
    "${IMAGE_NAME}:${IMAGE_TAG}"

Start-Sleep -Seconds 3

# ─────────────────────────────────────────────────────────────────────────────
# Verify
# ─────────────────────────────────────────────────────────────────────────────
$status = docker inspect -f '{{.State.Status}}' $CONTAINER 2>$null
if ($status -eq "running") {
    Write-Host ""
    Write-Host "  Agent is RUNNING" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "  Agent status: $status" -ForegroundColor Red
    Write-Host "  Check logs: docker logs $CONTAINER" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra Agent — Started"
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  COMMANDS:"
Write-Host "    Logs:       docker logs -f $CONTAINER"
Write-Host "    Stop:       docker stop $CONTAINER"
Write-Host "    Restart:    docker restart $CONTAINER"
Write-Host "    Remove:     docker rm -f $CONTAINER"
Write-Host "    Shell:      docker exec -it $CONTAINER bash"
Write-Host ""
Write-Host "  The agent connects to: $ApiEndpoint"
Write-Host "  Datacenter name:       $DcName"
Write-Host "======================================================"
Write-Host ""
