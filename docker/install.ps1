# =============================================================================
# Zentra - Automated Installation Script (Windows PowerShell)
# Downloads/loads the Docker image and starts all services automatically.
#
# Usage:
#   Right-click → Run with PowerShell
#   OR: powershell -ExecutionPolicy Bypass -File install.ps1
#   OR: powershell -ExecutionPolicy Bypass -File install.ps1 -NoWait
#
# Prerequisites: Docker Desktop must be installed and running.
# =============================================================================
param([switch]$NoWait)

$ErrorActionPreference = "Stop"

# ---------------------------------------------------------------------------
# Configuration - edit these if needed
# ---------------------------------------------------------------------------
$ContainerName = "zentra"
$ImageName     = "zentra:latest"
$WebPort       = "5010"
$VolumeName    = "zentra-data"
$LogDir        = "C:\zentra\logs"
$EnvFile       = "C:\zentra\.env"

# GHCR settings (only used if pulling from registry)
$GhcrImage = "ghcr.io/rameshsraj/zentra:latest"

# ---------------------------------------------------------------------------
# Helper functions
# ---------------------------------------------------------------------------
function Write-Info  { Write-Host "[INFO]  $args" -ForegroundColor Cyan }
function Write-Ok    { Write-Host "[OK]    $args" -ForegroundColor Green }
function Write-Warn  { Write-Host "[WARN]  $args" -ForegroundColor Yellow }
function Write-Err   { Write-Host "[ERROR] $args" -ForegroundColor Red }

Write-Host ""
Write-Host "======================================================" -ForegroundColor White
Write-Host "  Zentra - Automated Installer (Windows)"               -ForegroundColor White
Write-Host "======================================================" -ForegroundColor White
Write-Host ""

# ---------------------------------------------------------------------------
# Step 1: Check prerequisites
# ---------------------------------------------------------------------------
Write-Info "Checking prerequisites..."

try {
    $dockerVersion = docker --version 2>&1
    if ($LASTEXITCODE -ne 0) { throw "Docker not found" }
} catch {
    Write-Err "Docker is not installed or not in PATH."
    Write-Host ""
    Write-Host "  Install Docker Desktop from:"
    Write-Host "    https://www.docker.com/products/docker-desktop"
    Write-Host ""
    Write-Host "  After installation, ensure Docker Desktop is running."
    Write-Host ""
    Read-Host "Press Enter to exit"
    exit 1
}

try {
    docker info 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "Docker not running" }
} catch {
    Write-Err "Docker Desktop is not running. Please start Docker Desktop and try again."
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Ok "Docker is installed and running ($dockerVersion)"

# ---------------------------------------------------------------------------
# Step 2: Clean up existing installation
# ---------------------------------------------------------------------------
Write-Info "Cleaning up any previous Zentra installation..."

# Stop running container
$running = docker ps --format '{{.Names}}' 2>$null | Where-Object { $_ -eq $ContainerName }
if ($running) {
    Write-Info "Stopping running container '$ContainerName'..."
    docker stop $ContainerName 2>$null | Out-Null
    Start-Sleep -Seconds 3
    Write-Ok "Container stopped"
}

# Remove existing container
$existing = docker ps -a --format '{{.Names}}' 2>$null | Where-Object { $_ -eq $ContainerName }
if ($existing) {
    Write-Info "Removing old container '$ContainerName'..."
    docker rm -f $ContainerName 2>$null | Out-Null
    Start-Sleep -Seconds 2
    Write-Ok "Old container removed"
}

# Remove old volume (fresh database on each install)
$volExists = docker volume ls --format '{{.Name}}' 2>$null | Where-Object { $_ -eq $VolumeName }
if ($volExists) {
    Write-Info "Removing old data volume '$VolumeName'..."
    docker volume rm $VolumeName 2>$null | Out-Null
    Start-Sleep -Seconds 2
    Write-Ok "Old volume removed (fresh database will be created)"
}

# Remove old zentra images
$oldImages = docker images $ImageName -q 2>$null
if ($oldImages) {
    Write-Info "Removing old zentra image(s)..."
    docker rmi -f $oldImages 2>$null | Out-Null
    Start-Sleep -Seconds 2
    Write-Ok "Old zentra images removed"
}

# Remove stale/dangling images
$danglingImages = docker images --filter "dangling=true" -q 2>$null
if ($danglingImages) {
    Write-Info "Removing dangling images..."
    docker rmi $danglingImages 2>$null | Out-Null
    Start-Sleep -Seconds 2
    Write-Ok "Dangling images cleaned"
}

# Remove old .env to regenerate with fresh secrets
if (Test-Path $EnvFile) {
    Write-Info "Removing old environment file..."
    Remove-Item -Force $EnvFile
    Write-Ok "Old .env removed (will regenerate with fresh secrets)"
}

Write-Ok "Cleanup complete"

# ---------------------------------------------------------------------------
# Step 3: Load or pull the Docker image
# ---------------------------------------------------------------------------
Write-Info "Loading Zentra Docker image..."

$tarball = $null
$candidates = @("zentra-latest.tar.gz")
$candidates += Get-ChildItem -Path "." -Filter "zentra*.tar.gz" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name

foreach ($f in $candidates) {
    if (Test-Path $f) {
        $tarball = $f
        break
    }
}

if ($tarball) {
    Write-Info "Found tarball: $tarball - loading..."
    docker load -i $tarball
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Failed to load image from $tarball"
        Read-Host "Press Enter to exit"
        exit 1
    }
    Write-Ok "Image loaded from $tarball"
} else {
    # Check if image already exists
    $imgCheck = docker image inspect $ImageName 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "Image $ImageName already exists locally"
    } else {
        Write-Info "No tarball found. Pulling from GHCR..."
        Write-Warn "If the image is private, run first: docker login ghcr.io"
        docker pull $GhcrImage
        if ($LASTEXITCODE -ne 0) {
            Write-Err "Failed to pull image from GHCR"
            Read-Host "Press Enter to exit"
            exit 1
        }
        docker tag $GhcrImage $ImageName
        Write-Ok "Image pulled from GHCR"
    }
}

# Verify image
docker image inspect $ImageName 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Err "Image $ImageName not found. Cannot continue."
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Ok "Image ready: $ImageName"

# ---------------------------------------------------------------------------
# Step 4: Create .env file
# ---------------------------------------------------------------------------
if (-not (Test-Path $EnvFile)) {
    Write-Info "Creating default environment file at $EnvFile..."

    # Create directory
    $envDir = Split-Path $EnvFile
    if (-not (Test-Path $envDir)) {
        New-Item -ItemType Directory -Path $envDir -Force | Out-Null
    }

    # Generate random secrets
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $jwtSecret = [BitConverter]::ToString($bytes).Replace("-", "").ToLower()

    $bytes2 = New-Object byte[] 16
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes2)
    $pgPass = [BitConverter]::ToString($bytes2).Replace("-", "").ToLower()

    $envContent = @"
# === Zentra Configuration ===
# Generated by install.ps1 on $(Get-Date -Format "yyyy-MM-dd")

# PostgreSQL
POSTGRES_USER=postgres
POSTGRES_PASSWORD=$pgPass
POSTGRES_DB=zentra
POSTGRES_PORT=5434

# Zentra API
ZENTRA_JWT_SECRET=$jwtSecret
ZENTRA_MASTER_KEY=$jwtSecret
ZENTRA_ALLOW_INSECURE_JWT=1
ZENTRA_API_PORT=5006
ZENTRA_ALLOW_ORIGINS=*
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001

# Logging
RUST_LOG=info
"@
    $envContent | Out-File -FilePath $EnvFile -Encoding utf8 -NoNewline
    Write-Ok "Environment file created: $EnvFile"
    Write-Info "  PostgreSQL password: $pgPass"
    Write-Info "  JWT secret: auto-generated (64 chars)"
} else {
    Write-Ok "Environment file already exists: $EnvFile"
}

# ---------------------------------------------------------------------------
# Step 5: Create log directory
# ---------------------------------------------------------------------------
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
}
Write-Ok "Log directory ready: $LogDir"

# ---------------------------------------------------------------------------
# Step 6: Start the container
# ---------------------------------------------------------------------------
Write-Info "Starting Zentra container..."

docker run -d `
    --name $ContainerName `
    -p "${WebPort}:5010" `
    --restart unless-stopped `
    --env-file $EnvFile `
    -v "${VolumeName}:/var/lib/postgresql/data" `
    -v "${LogDir}:/opt/zentra/logs" `
    $ImageName | Out-Null

if ($LASTEXITCODE -ne 0) {
    Write-Err "Failed to start container"
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Ok "Container '$ContainerName' started"

# ---------------------------------------------------------------------------
# Step 7: Wait for health check
# ---------------------------------------------------------------------------
Write-Info "Waiting for all services to start..."

$healthy = $false
for ($i = 1; $i -le 30; $i++) {
    try {
        $response = Invoke-RestMethod -Uri "http://localhost:${WebPort}/api/health" -TimeoutSec 3 -ErrorAction SilentlyContinue
        if ($response.status) {
            $healthy = $true
            break
        }
    } catch { }
    Write-Host "." -NoNewline
    Start-Sleep -Seconds 2
}
Write-Host ""

if ($healthy) {
    Write-Ok "All services are running and healthy!"
} else {
    Write-Warn "Health check not yet passing - services may still be starting."
    Write-Warn "Check logs with: docker logs $ContainerName"
}

# ---------------------------------------------------------------------------
# Step 8: Print summary
# ---------------------------------------------------------------------------
try {
    # Exclude loopback, Docker/WSL virtual adapters (172.16-31.x.x, 10.x.x.x), and APIPA
    $serverIP = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object {
        $_.IPAddress -ne "127.0.0.1" -and
        $_.PrefixOrigin -ne "WellKnown" -and
        $_.IPAddress -notmatch '^172\.(1[6-9]|2[0-9]|3[01])\.' -and
        $_.IPAddress -notmatch '^10\.' -and
        $_.IPAddress -notmatch '^169\.254\.'
    } | Select-Object -First 1).IPAddress
} catch {
    $serverIP = "localhost"
}
if (-not $serverIP) { $serverIP = "localhost" }

Write-Host ""
Write-Host "======================================================" -ForegroundColor Green
Write-Host "  Zentra - Installation Complete!"                       -ForegroundColor Green
Write-Host "======================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Web UI:     http://${serverIP}:${WebPort}"
Write-Host "  Login:      admin@Zentra.local / changeme"
Write-Host ""
Write-Host "  Container:  docker ps"
Write-Host "  Logs:       docker logs -f $ContainerName"
Write-Host "  Stop:       docker stop $ContainerName"
Write-Host "  Start:      docker start $ContainerName"
Write-Host "  Shell:      docker exec -it $ContainerName bash"
Write-Host ""
Write-Host "  IMPORTANT: Change the default admin password after first login!" -ForegroundColor Yellow
Write-Host "======================================================" -ForegroundColor Green
Write-Host ""

if (-not $NoWait) { Read-Host "Press Enter to close" }
