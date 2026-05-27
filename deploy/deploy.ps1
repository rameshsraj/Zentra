#!/usr/bin/env pwsh
# =============================================================
# Zentra Deployment Script
# Deploys source to remote Linux server, compiles, and starts
# =============================================================

$ErrorActionPreference = "Stop"

# ---- Configuration (from .env) ----
$SERVER_HOST  = "10.88.11.162"
$SERVER_PORT  = 22
$SERVER_USER  = "root"
$SERVER_PASS  = "skyline"
$DEPLOY_PATH  = "/store/Zentra"
$BACKEND_PORT = 5006
$FRONTEND_PORT= 5010
$AGENT_PORT   = 5001

$PROJECT_ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path "$PROJECT_ROOT\Cargo.toml")) {
    $PROJECT_ROOT = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$PROJECT_ROOT\Cargo.toml")) {
    $PROJECT_ROOT = "C:\Project-code\Zentra"
}

Write-Host "============================================" -ForegroundColor Cyan
Write-Host " Zentra Deployment" -ForegroundColor Cyan
Write-Host " Server: $SERVER_USER@$SERVER_HOST" -ForegroundColor Cyan
Write-Host " Path:   $DEPLOY_PATH" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan

$PLINK  = Join-Path $PSScriptRoot "plink.exe"
$PSCP   = Join-Path $PSScriptRoot "pscp.exe"

# ---- Helper: Run SSH command via plink ----
function Invoke-SSH {
    param([string]$Command, [int]$TimeoutSec = 600)
    Write-Host "[SSH] $Command" -ForegroundColor DarkGray
    $pinfo = New-Object System.Diagnostics.ProcessStartInfo
    $pinfo.FileName = $PLINK
    $pinfo.Arguments = "-batch -pw $SERVER_PASS -P $SERVER_PORT $SERVER_USER@$SERVER_HOST `"$Command`""
    $pinfo.RedirectStandardOutput = $true
    $pinfo.RedirectStandardError = $true
    $pinfo.UseShellExecute = $false
    $pinfo.CreateNoWindow = $true

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $pinfo
    $process.Start() | Out-Null

    # Read output asynchronously to avoid deadlock
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()

    $finished = $process.WaitForExit($TimeoutSec * 1000)

    if (-not $finished) {
        $process.Kill()
        Write-Host "[SSH] TIMEOUT after ${TimeoutSec}s - killed" -ForegroundColor Yellow
    }

    $stdout = $stdoutTask.Result
    $stderr = $stderrTask.Result

    if ($stdout) { Write-Host $stdout }
    if ($stderr -and $process.ExitCode -ne 0) { Write-Host $stderr -ForegroundColor Yellow }
    return $process.ExitCode
}

# ---- Helper: SCP upload via pscp ----
function Invoke-SCP {
    param([string]$LocalPath, [string]$RemotePath, [switch]$Recursive)
    $rFlag = ""
    if ($Recursive) { $rFlag = "-r" }
    Write-Host "[SCP] $LocalPath -> $RemotePath" -ForegroundColor DarkGray
    $args_str = "-batch -pw $SERVER_PASS -P $SERVER_PORT $rFlag `"$LocalPath`" $SERVER_USER@${SERVER_HOST}:$RemotePath"
    $pinfo = New-Object System.Diagnostics.ProcessStartInfo
    $pinfo.FileName = $PSCP
    $pinfo.Arguments = $args_str
    $pinfo.RedirectStandardOutput = $true
    $pinfo.RedirectStandardError = $true
    $pinfo.UseShellExecute = $false
    $pinfo.CreateNoWindow = $true

    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $pinfo
    $process.Start() | Out-Null

    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()

    $finished = $process.WaitForExit(300000)  # 5 min timeout

    if (-not $finished) {
        $process.Kill()
        Write-Host "[SCP] TIMEOUT - killed" -ForegroundColor Yellow
        return 1
    }

    $stderr = $stderrTask.Result
    if ($process.ExitCode -ne 0) {
        Write-Host "[SCP] ERROR: $stderr" -ForegroundColor Red
    }
    return $process.ExitCode
}

# =============================================================
# STEP 1: Build frontend locally (already done, use dist/)
# =============================================================
Write-Host ""
Write-Host "STEP 1: Building frontend..." -ForegroundColor Green

Push-Location "$PROJECT_ROOT\frontend"
try {
    if (-not (Test-Path "node_modules")) {
        Write-Host "Installing npm dependencies..."
        npm install --legacy-peer-deps 2>&1 | Out-Null
    }
    Write-Host "Building frontend for production..."
    $ErrorActionPreference = "Continue"
    npx --yes vite build 2>&1 | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
    $ErrorActionPreference = "Stop"
    if (-not (Test-Path "dist\index.html")) {
        throw "Frontend build failed - dist/index.html not found"
    }
    Write-Host "Frontend build OK" -ForegroundColor Green
} finally {
    Pop-Location
}

# =============================================================
# STEP 2: Prepare deployment archive
# =============================================================
Write-Host ""
Write-Host "STEP 2: Preparing deployment archive..." -ForegroundColor Green

$STAGING = "$env:TEMP\Zentra-deploy"
if (Test-Path $STAGING) { Remove-Item -Recurse -Force $STAGING }
New-Item -ItemType Directory -Path $STAGING | Out-Null

# Copy Rust source
Copy-Item "$PROJECT_ROOT\Cargo.toml" "$STAGING\Cargo.toml"
Copy-Item "$PROJECT_ROOT\Cargo.lock" "$STAGING\Cargo.lock"
Copy-Item "$PROJECT_ROOT\crates"    "$STAGING\crates"   -Recurse
Copy-Item "$PROJECT_ROOT\plugins"   "$STAGING\plugins"  -Recurse
# .env lives on the server already — do not overwrite it
Write-Host "  Skipping .env (using server's existing .env)" -ForegroundColor Yellow

# Copy frontend dist
New-Item -ItemType Directory -Path "$STAGING\frontend\dist" -Force | Out-Null
Copy-Item "$PROJECT_ROOT\frontend\dist\*" "$STAGING\frontend\dist" -Recurse

# Copy SQL migrations
if (Test-Path "$PROJECT_ROOT\deploy\sql") {
    New-Item -ItemType Directory -Path "$STAGING\deploy\sql" -Force | Out-Null
    Copy-Item "$PROJECT_ROOT\deploy\sql\*" "$STAGING\deploy\sql" -Recurse
}

# Exclude build artifacts from Rust
if (Test-Path "$STAGING\target") { Remove-Item -Recurse -Force "$STAGING\target" }

# Create tar.gz
$ARCHIVE = "$env:TEMP\Zentra-deploy.tar.gz"
if (Test-Path $ARCHIVE) { Remove-Item $ARCHIVE }
Push-Location $STAGING
tar -czf $ARCHIVE *
Pop-Location
$archiveSize = (Get-Item $ARCHIVE).Length / 1MB
Write-Host "Archive created: $([math]::Round($archiveSize, 1)) MB" -ForegroundColor Green

# =============================================================
# STEP 3: Kill old services on remote
# =============================================================
Write-Host ""
Write-Host "STEP 3: Stopping old services on remote..." -ForegroundColor Green

Invoke-SSH "pkill -f zentra-server 2>/dev/null; pkill -f zentra-agent 2>/dev/null; echo 'Old processes killed'"
Invoke-SSH "fuser -k ${BACKEND_PORT}/tcp 2>/dev/null; fuser -k ${AGENT_PORT}/tcp 2>/dev/null; echo 'Ports freed: $BACKEND_PORT, $AGENT_PORT'"
# Clean old frontend assets to prevent stale cache
Invoke-SSH "rm -rf $DEPLOY_PATH/frontend/dist/assets/* 2>/dev/null; echo 'Old frontend assets cleaned'"

# =============================================================
# STEP 4: Upload to server
# =============================================================
Write-Host ""
Write-Host "STEP 4: Uploading to server..." -ForegroundColor Green

Invoke-SSH "mkdir -p $DEPLOY_PATH"
$scpResult = Invoke-SCP -LocalPath $ARCHIVE -RemotePath "$DEPLOY_PATH/Zentra-deploy.tar.gz"
if ($scpResult -ne 0) { throw "SCP upload failed" }

# Extract on server
Invoke-SSH "cd $DEPLOY_PATH; tar -xzf Zentra-deploy.tar.gz; rm Zentra-deploy.tar.gz; echo Extracted OK"

# =============================================================
# STEP 5: Install Rust toolchain if needed
# =============================================================
Write-Host ""
Write-Host "STEP 5: Ensuring Rust toolchain on server..." -ForegroundColor Green

Invoke-SSH 'which cargo >/dev/null 2>&1 || { curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; source ~/.cargo/env; echo Rust installed; }'
Invoke-SSH 'source ~/.cargo/env 2>/dev/null; rustc --version'

# Install system dependencies for pcap
Invoke-SSH 'apt-get update -qq; apt-get install -y -qq libpcap-dev pkg-config build-essential 2>/dev/null; yum install -y libpcap-devel pkgconfig gcc 2>/dev/null; echo System deps OK'

# =============================================================
# STEP 6: Compile on server (with timeout + retry)
# =============================================================
Write-Host ""
Write-Host "STEP 6: Compiling on server (this may take several minutes)..." -ForegroundColor Green

$compileCmd = "source ~/.cargo/env 2>/dev/null; cd $DEPLOY_PATH; cargo build --release --bin zentra-server --bin zentra-agent 2>&1"

# First attempt - 10 min timeout
Write-Host "  Compile attempt 1/3..." -ForegroundColor Yellow
$exitCode = Invoke-SSH $compileCmd -TimeoutSec 600

if ($exitCode -ne 0) {
    Write-Host "  Compile attempt 1 failed or timed out. Retrying..." -ForegroundColor Yellow
    Start-Sleep -Seconds 5
    $exitCode = Invoke-SSH $compileCmd -TimeoutSec 900

    if ($exitCode -ne 0) {
        Write-Host "  Compile attempt 2 failed. Final retry with 15 min timeout..." -ForegroundColor Yellow
        Start-Sleep -Seconds 5
        $exitCode = Invoke-SSH $compileCmd -TimeoutSec 900

        if ($exitCode -ne 0) {
            throw "Compilation failed after 3 attempts"
        }
    }
}

Write-Host "Compilation successful!" -ForegroundColor Green

# =============================================================
# STEP 7: Deploy frontend with simple HTTP server
# =============================================================
Write-Host ""
Write-Host "STEP 7: Setting up frontend server..." -ForegroundColor Green

# Create a simple nginx config or use python
$nginxConf = @"
server {
    listen $FRONTEND_PORT;
    root $DEPLOY_PATH/frontend/dist;
    index index.html;

    location / {
        try_files \`$uri \`$uri/ /index.html;
    }

    location /api/ {
        proxy_pass http://127.0.0.1:$BACKEND_PORT;
        proxy_set_header Host \`$host;
        proxy_set_header X-Real-IP \`$remote_addr;
    }
}
"@

Invoke-SSH 'which nginx >/dev/null 2>&1; if [ $? -eq 0 ]; then echo nginx found; else echo nginx not found - will use python; fi'

# Create startup script on server
$startupScript = @'
#!/bin/bash
set -e

export DEPLOY_PATH=__DEPLOY_PATH__
export BACKEND_PORT=__BACKEND_PORT__
export FRONTEND_PORT=__FRONTEND_PORT__

cd __DEPLOY_PATH__

# Source .env
set -a; source .env 2>/dev/null; set +a

# Kill any existing services
pkill -f zentra-server 2>/dev/null || true
pkill -f zentra-agent 2>/dev/null || true
fuser -k __BACKEND_PORT__/tcp 2>/dev/null || true
fuser -k __FRONTEND_PORT__/tcp 2>/dev/null || true
sleep 1

# Start API server
echo "Starting zentra-server on port __BACKEND_PORT__..."
nohup ./target/release/zentra-server > /var/log/zentra-server.log 2>&1 &
echo "PID: $!"

# Start agent
echo "Starting zentra-agent..."
nohup ./target/release/zentra-agent > /var/log/zentra-agent.log 2>&1 &
echo "PID: $!"

# Start frontend (python fallback if no nginx)
if which nginx >/dev/null 2>&1; then
    echo "Using nginx for frontend"
else
    echo "Starting python HTTP server for frontend on port __FRONTEND_PORT__..."
    cd __DEPLOY_PATH__/frontend/dist
    nohup python3 -m http.server __FRONTEND_PORT__ > /var/log/Zentra-frontend.log 2>&1 &
    echo "PID: $!"
fi

echo ""
echo "============================================"
echo " Zentra services started!"
echo " API:      http://__SERVER_HOST__:__BACKEND_PORT__"
echo " Frontend: http://__SERVER_HOST__:__FRONTEND_PORT__"
echo "============================================"
'@
$startupScript = $startupScript -replace '__DEPLOY_PATH__', $DEPLOY_PATH
$startupScript = $startupScript -replace '__BACKEND_PORT__', $BACKEND_PORT
$startupScript = $startupScript -replace '__FRONTEND_PORT__', $FRONTEND_PORT
$startupScript = $startupScript -replace '__SERVER_HOST__', $SERVER_HOST

# Write startup script to a local temp file and SCP it
$startScriptPath = "$env:TEMP\Zentra-start.sh"
$startupScript | Out-File -FilePath $startScriptPath -Encoding utf8 -Force
# Fix line endings to unix
$content = [System.IO.File]::ReadAllText($startScriptPath)
$content = $content -replace "`r`n", "`n"
[System.IO.File]::WriteAllText($startScriptPath, $content)
Invoke-SCP -LocalPath $startScriptPath -RemotePath "$DEPLOY_PATH/start.sh"
Invoke-SSH "chmod +x $DEPLOY_PATH/start.sh"

# =============================================================
# STEP 8: Run SQL migrations
# =============================================================
Write-Host ""
Write-Host "STEP 8: Running SQL migrations..." -ForegroundColor Green

Invoke-SSH "cd $DEPLOY_PATH/deploy/sql && export PGPASSWORD='zantaz' && for f in 001_schema.sql 002_indexes.sql 003_license_management.sql 004_predictions.sql 005_scrape_targets.sql 006_annotations.sql 007_dashboards.sql 008_timescaledb_enhance.sql 009_dashboard_enhance.sql 010_rbac_audit.sql 011_favorites_blacklist.sql 012_auth_system.sql 013_network_security.sql 014_oracle_monitoring.sql 022_user_module_access.sql 023_user_menu_items.sql; do echo Applying: `$f; psql -h 10.88.11.162 -p 5434 -U postgres -d zentra -f `$f 2>&1 | tail -2; done && echo ALL_MIGRATIONS_APPLIED"

# =============================================================
# STEP 9: Start services
# =============================================================
Write-Host ""
Write-Host "STEP 9: Starting services..." -ForegroundColor Green

Invoke-SSH "bash $DEPLOY_PATH/start.sh"

# Restart nginx to serve new frontend
Write-Host "Restarting nginx..." -ForegroundColor Yellow
Invoke-SSH 'systemctl restart nginx 2>/dev/null && echo nginx restarted || echo nginx not available'

# Verify
Start-Sleep -Seconds 3
Invoke-SSH "curl -sf http://127.0.0.1:${BACKEND_PORT}/api/health 2>/dev/null; if [ \$? -eq 0 ]; then echo API health OK; else echo API not responding yet - may be starting; fi"
Invoke-SSH "curl -sf http://127.0.0.1:${FRONTEND_PORT}/ >/dev/null 2>&1; if [ \$? -eq 0 ]; then echo Frontend OK; else echo Frontend not responding; fi"

# =============================================================
# Done
# =============================================================
Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host " DEPLOYMENT COMPLETE" -ForegroundColor Green
Write-Host " API:      http://${SERVER_HOST}:${BACKEND_PORT}" -ForegroundColor Green
Write-Host " Frontend: http://${SERVER_HOST}:${FRONTEND_PORT}" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green

# Cleanup
Remove-Item -Recurse -Force $STAGING -ErrorAction SilentlyContinue
Remove-Item $ARCHIVE -ErrorAction SilentlyContinue
