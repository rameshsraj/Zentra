#Requires -Version 5.1
<#
.SYNOPSIS
    Deploy Zentra from local source to a remote VM.

.DESCRIPTION
    1. Packages source (excluding .git / target / node_modules / dist).
    2. Uploads the archive and extracts it on the server.
    3. Builds the React frontend (npm run build).
    4. Compiles zentra-api, zentra-agent, zentra-collector with cargo.
    5. Installs binaries, writes systemd service + nginx config, starts services.
    6. Runs a health check and prints a full summary.

.PARAMETER IP
    Target server IP address.  Example: 172.19.149.102

.PARAMETER SshPassword
    SSH password for the root account.  Default: no1knows

.PARAMETER HostKey
    PuTTY-format host key fingerprint for strict verification.

.PARAMETER RemoteDir
    Absolute path on the server where Zentra lives.  Default: /store/Zentra

.PARAMETER DbPassword
    PostgreSQL password used in the runtime env file.  Default: zantaz

.EXAMPLE
    .\deploy\deploy-to-vm.ps1 -IP 172.19.149.102

.EXAMPLE
    .\deploy\deploy-to-vm.ps1 -IP 10.0.0.5 -SshPassword mypass
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0, HelpMessage = "Server IP address")]
    [ValidateNotNullOrEmpty()]
    [string]$IP,

    [string]$SshUser     = "root",
    [string]$SshPassword = "no1knows",
    [string]$HostKey     = "ssh-ed25519 255 SHA256:YV3/bnFf99fyNUqZGvYpxAUQLzTCZOl+CWpYzJPFxj8",
    [string]$RemoteDir   = "/store/Zentra",
    [string]$DbPassword  = "zantaz"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDir   = $PSScriptRoot
$ProjectRoot = Split-Path $ScriptDir -Parent
$PlinkExe    = Join-Path $ScriptDir "plink.exe"
$PscpExe     = Join-Path $ScriptDir "pscp.exe"
$StartTime   = Get-Date
$Summary     = [ordered]@{}

# ── Preflight checks ───────────────────────────────────────────────────────────
foreach ($bin in @($PlinkExe, $PscpExe)) {
    if (-not (Test-Path $bin)) { throw "Required tool not found: $bin" }
}
if (-not (Get-Command tar -ErrorAction SilentlyContinue)) {
    throw "tar not found. Windows 10 1803+ or WSL required."
}

# ── Console helpers ────────────────────────────────────────────────────────────
function Write-Step([string]$msg) {
    Write-Host ""
    Write-Host ("=" * 62) -ForegroundColor Cyan
    Write-Host "  $msg" -ForegroundColor Cyan
    Write-Host ("=" * 62) -ForegroundColor Cyan
}
function Write-Ok([string]$msg)   { Write-Host "  [OK]   $msg" -ForegroundColor Green  }
function Write-Info([string]$msg) { Write-Host "         $msg" -ForegroundColor Gray   }

function Bail([string]$msg) {
    Write-Host ""
    Write-Host "  [FAIL] $msg" -ForegroundColor Red
    exit 1
}

# ── SSH / SCP helpers ──────────────────────────────────────────────────────────
$PlinkBase = @("-ssh", "-batch", "-pw", $SshPassword, "-hostkey", $HostKey, "${SshUser}@${IP}")

function Invoke-Remote([string]$cmd) {
    $out = & $PlinkExe @PlinkBase $cmd 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host ($out -join "`n") -ForegroundColor Red
        Bail "Remote command failed (exit $LASTEXITCODE)"
    }
    return ($out -join "`n")
}

function Invoke-RemoteRaw([string]$cmd) {
    return (& $PlinkExe @PlinkBase $cmd 2>&1) -join "`n"
}

function Upload-File([string]$local, [string]$remote) {
    & $PscpExe -pw $SshPassword -hostkey $HostKey -q $local "${SshUser}@${IP}:${remote}" 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { Bail "Upload failed: $local -> $remote" }
}

# Upload a string as a remote bash script (writes with Unix line endings)
function Upload-Script([string]$content, [string]$remotePath) {
    $tmp = [System.IO.Path]::GetTempFileName()
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes(($content -replace "`r`n", "`n"))
        [System.IO.File]::WriteAllBytes($tmp, $bytes)
        & $PscpExe -pw $SshPassword -hostkey $HostKey -q $tmp "${SshUser}@${IP}:${remotePath}" 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { Bail "Script upload failed: $remotePath" }
    }
    finally { Remove-Item $tmp -Force -ErrorAction SilentlyContinue }
}

# Poll a remote log file until __BUILD_OK__ or __BUILD_FAIL__ appears
function Wait-Build([string]$logFile, [string]$label) {
    $elapsed = 0
    $maxWait = 2400   # 40 min hard cap
    Write-Info "  Waiting for $label to finish compiling ..."
    while ($elapsed -lt $maxWait) {
        Start-Sleep -Seconds 15
        $elapsed += 15
        $tail = Invoke-RemoteRaw "tail -5 $logFile 2>/dev/null"
        if ($tail -match '__BUILD_OK__')   { Write-Ok "$label compiled (${elapsed}s)"; return }
        if ($tail -match '__BUILD_FAIL__') {
            $errs = Invoke-RemoteRaw "grep '^error' $logFile 2>/dev/null | head -30"
            Write-Host $errs -ForegroundColor Red
            Bail "$label build FAILED — see $logFile on server"
        }
        if ($elapsed % 60 -eq 0) { Write-Info "  ... $label still compiling (${elapsed}s)" }
    }
    Bail "$label timed out after ${maxWait}s"
}

# Phase timing helpers
$PhaseStart = $StartTime
function Start-Phase([string]$msg) { Write-Step $msg; $script:PhaseStart = Get-Date }
function End-Phase([string]$key)   { $Summary[$key] = "$([int]((Get-Date) - $script:PhaseStart).TotalSeconds)s" }

Write-Host ""
Write-Host "  Zentra Deploy  |  target: $IP  |  $(Get-Date -Format 'yyyy-MM-dd HH:mm')" -ForegroundColor White

# ══════════════════════════════════════════════════════════════════════════════
#  Phase 1 — Package & upload source
# ══════════════════════════════════════════════════════════════════════════════
Start-Phase "Phase 1/5  ·  Package & upload source"

$parentDir = Split-Path $ProjectRoot -Parent
$projName  = Split-Path $ProjectRoot -Leaf
$tarFile   = Join-Path $env:TEMP "zentra-deploy-$(Get-Date -Format 'yyyyMMddHHmmss').tar.gz"

Write-Info "Creating archive (excluding .git / target / node_modules / dist) ..."
& tar -czf $tarFile `
    "--exclude=.git" `
    "--exclude=target" `
    "--exclude=node_modules" `
    "--exclude=dist" `
    "--exclude=*.log" `
    -C $parentDir $projName 2>&1 | Out-Null

if ($LASTEXITCODE -ne 0) { Bail "tar failed — is tar available? (requires Windows 10 1803+)" }

$sizeMB = [math]::Round((Get-Item $tarFile).Length / 1MB, 1)
Write-Info "Archive size: $sizeMB MB  — uploading ..."

Upload-File $tarFile "/tmp/zentra-deploy.tar.gz"
Remove-Item $tarFile -Force

Write-Info "Extracting on server ..."
$remoteParent = Split-Path $RemoteDir -Parent    # /store
Invoke-Remote "tar -xzf /tmp/zentra-deploy.tar.gz -C $remoteParent && rm -f /tmp/zentra-deploy.tar.gz" | Out-Null

# Fix env file after extraction (port, JWT secret, master key)
$envFixScript = @'
#!/bin/bash
set -e
ENV="/store/Zentra/deploy/vm-runtime.env"

# Ensure DATABASE_URL uses port 5434
sed -i 's|:5432/|:5434/|g' "$ENV"

# Regenerate JWT secret if it is the placeholder (<32 chars)
JWT=$(grep '^ZENTRA_JWT_SECRET=' "$ENV" | cut -d= -f2-)
if [ "${#JWT}" -lt 32 ]; then
    NEW=$(openssl rand -hex 32)
    sed -i "s|^ZENTRA_JWT_SECRET=.*|ZENTRA_JWT_SECRET=$NEW|" "$ENV"
    echo "  JWT secret regenerated"
fi

# Regenerate master key if it is the placeholder (<32 chars)
MASTER=$(grep '^ZENTRA_MASTER_KEY=' "$ENV" | cut -d= -f2-)
if [ "${#MASTER}" -lt 32 ]; then
    NEW=$(openssl rand -hex 32)
    sed -i "s|^ZENTRA_MASTER_KEY=.*|ZENTRA_MASTER_KEY=$NEW|" "$ENV"
    echo "  Master key regenerated"
fi

echo "ENV_FIXED"
'@

Upload-Script $envFixScript "/tmp/fix-env.sh"
$out = Invoke-Remote "bash /tmp/fix-env.sh 2>&1"
if ($out -notmatch "ENV_FIXED") { Bail "Env fix script failed: $out" }
if ($out -match "regenerated") { Write-Info $out }

End-Phase "Source upload"
Write-Ok "Source uploaded ($sizeMB MB) and extracted to $RemoteDir"

# ══════════════════════════════════════════════════════════════════════════════
#  Phase 2 — Frontend build
# ══════════════════════════════════════════════════════════════════════════════
Start-Phase "Phase 2/5  ·  Build frontend (npm)"

$frontendScript = @'
#!/bin/bash
set -e
cd /store/Zentra/frontend

echo "  npm ci ..."
npm ci --prefer-offline --silent 2>/dev/null || npm install --silent

echo "  npm run build ..."
npm run build 2>&1 | tail -6

echo "FRONTEND_BUILD_OK"
'@

Upload-Script $frontendScript "/tmp/build-frontend.sh"
$out = Invoke-Remote "bash /tmp/build-frontend.sh 2>&1"
Write-Info ($out -replace "(?m)^", "  ")
if ($out -notmatch "FRONTEND_BUILD_OK") { Bail "Frontend build failed" }

End-Phase "Frontend build"
Write-Ok "Frontend built — dist at $RemoteDir/frontend/dist"

# ══════════════════════════════════════════════════════════════════════════════
#  Phase 3 — Build Rust binaries
# ══════════════════════════════════════════════════════════════════════════════
Start-Phase "Phase 3/5  ·  Build backend (zentra-api · zentra-agent · zentra-collector)"

# Sequential build: zentra-api compiles all shared deps first, so
# zentra-agent and zentra-collector are fast (minutes, not 10-15 min).
$buildScript = @'
#!/bin/bash
cd /store/Zentra
rm -f /tmp/build-api.log /tmp/build-agent.log /tmp/build-collector.log

cargo build --release -p zentra-api     >> /tmp/build-api.log     2>&1 \
    && echo __BUILD_OK__ >> /tmp/build-api.log     || echo __BUILD_FAIL__ >> /tmp/build-api.log

cargo build --release -p zentra-agent   >> /tmp/build-agent.log   2>&1 \
    && echo __BUILD_OK__ >> /tmp/build-agent.log   || echo __BUILD_FAIL__ >> /tmp/build-agent.log

cargo build --release -p zentra-collector >> /tmp/build-collector.log 2>&1 \
    && echo __BUILD_OK__ >> /tmp/build-collector.log || echo __BUILD_FAIL__ >> /tmp/build-collector.log
'@

Upload-Script $buildScript "/tmp/build-all.sh"
Write-Info "Launching build in background (zentra-api first — ~10-15 min for first run) ..."
Invoke-Remote "chmod +x /tmp/build-all.sh && nohup /tmp/build-all.sh &>/dev/null &" | Out-Null

Wait-Build "/tmp/build-api.log"       "zentra-api"
Wait-Build "/tmp/build-agent.log"     "zentra-agent"
Wait-Build "/tmp/build-collector.log" "zentra-collector"

End-Phase "Backend build"
Write-Ok "All three binaries compiled"

# ══════════════════════════════════════════════════════════════════════════════
#  Phase 4 — Install binaries & services
# ══════════════════════════════════════════════════════════════════════════════
Start-Phase "Phase 4/5  ·  Install binaries, systemd services & nginx"

# This script is uploaded so bash heredocs work without any quoting issues.
$installScript = @'
#!/bin/bash
set -e
REMOTE_DIR="/store/Zentra"
BIN_DIR="$REMOTE_DIR/bin"

# ── Copy release binaries ──────────────────────────────────────────────────────
mkdir -p "$BIN_DIR/dl"
cp -f "$REMOTE_DIR/target/release/zentra-server"      "$BIN_DIR/zentra-server"
cp -f "$REMOTE_DIR/target/release/zentra-agent"       "$BIN_DIR/dl/zentra-agent-linux-x86_64"
cp -f "$REMOTE_DIR/target/release/zentra-collector"   "$BIN_DIR/dl/zentra-collector-linux-x86_64"
chmod +x "$BIN_DIR/zentra-server" \
         "$BIN_DIR/dl/zentra-agent-linux-x86_64" \
         "$BIN_DIR/dl/zentra-collector-linux-x86_64"
ls -lh "$BIN_DIR/zentra-server" "$BIN_DIR/dl/"*
echo "Binaries installed"

# ── Ensure zentra system user exists ──────────────────────────────────────────
id zentra &>/dev/null || useradd --system --no-create-home --shell /usr/sbin/nologin zentra

# ── systemd: zentra-api ────────────────────────────────────────────────────────
cat > /etc/systemd/system/zentra-api.service << 'SVCEOF'
[Unit]
Description=Zentra API Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=zentra
Group=zentra
WorkingDirectory=/store/Zentra
EnvironmentFile=/store/Zentra/deploy/vm-runtime.env
ExecStart=/store/Zentra/bin/zentra-server
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=zentra-api
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
SVCEOF

# ── Install nginx if missing ───────────────────────────────────────────────────
if ! command -v nginx &>/dev/null; then
    echo "Installing nginx ..."
    DEBIAN_FRONTEND=noninteractive apt-get install -y nginx &>/dev/null
fi

# ── nginx site (port 5010 → proxy to API on 5006) ─────────────────────────────
cat > /etc/nginx/sites-available/zentra << 'NGXEOF'
server {
    listen 5010;
    server_name _;

    root  /store/Zentra/frontend/dist;
    index index.html;

    # React SPA
    location / {
        try_files $uri $uri/ /index.html;
    }

    # REST API
    location /api/ {
        proxy_pass         http://127.0.0.1:5006;
        proxy_set_header   Host            $host;
        proxy_set_header   X-Real-IP       $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 300s;
    }

    # OTLP HTTP endpoints
    location /v1/ {
        proxy_pass         http://127.0.0.1:5006;
        proxy_set_header   Host            $host;
        proxy_set_header   X-Real-IP       $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 300s;
    }
}
NGXEOF

ln -sf /etc/nginx/sites-available/zentra /etc/nginx/sites-enabled/zentra
rm -f /etc/nginx/sites-enabled/default
nginx -t

# ── Reload daemon & restart services ──────────────────────────────────────────
systemctl daemon-reload
systemctl enable zentra-api nginx

systemctl restart zentra-api
sleep 3

if systemctl is-active --quiet nginx; then
    systemctl reload nginx || systemctl restart nginx
else
    systemctl start nginx
fi

sleep 2

# ── Verify both services are active ───────────────────────────────────────────
FAILED=0
for svc in zentra-api nginx; do
    if systemctl is-active --quiet "$svc"; then
        echo "  OK: $svc"
    else
        echo "  FAIL: $svc" >&2
        systemctl status "$svc" --no-pager -l | tail -15 >&2
        FAILED=1
    fi
done

[ "$FAILED" -eq 0 ] && echo "INSTALL_OK" || exit 1
'@

Upload-Script $installScript "/tmp/install-services.sh"
$out = Invoke-Remote "bash /tmp/install-services.sh 2>&1"
Write-Info ($out -replace "(?m)^", "  ")
if ($out -notmatch "INSTALL_OK") { Bail "Service installation failed" }

End-Phase "Install services"
Write-Ok "Binaries installed, zentra-api + nginx running"

# ══════════════════════════════════════════════════════════════════════════════
#  Phase 5 — Health check
# ══════════════════════════════════════════════════════════════════════════════
Start-Phase "Phase 5/5  ·  Health check"

$apiHealth  = $null
$nginxCode  = $null

for ($i = 1; $i -le 12; $i++) {
    $apiHealth = Invoke-RemoteRaw "curl -sf http://127.0.0.1:5006/api/health 2>/dev/null"
    if ($apiHealth -match '"status"\s*:\s*"healthy"') { break }
    Write-Info "Waiting for API to start ... ($i/12)"
    Start-Sleep -Seconds 5
}

if ($apiHealth -notmatch '"status"\s*:\s*"healthy"') {
    Bail "API health check failed. Response: $apiHealth"
}

$nginxCode = Invoke-RemoteRaw "curl -sf -o /dev/null -w '%{http_code}' http://127.0.0.1:5010/ 2>/dev/null"

End-Phase "Health check"
Write-Ok "API healthy: $apiHealth"
Write-Ok "nginx HTTP $nginxCode on port 5010"

# ══════════════════════════════════════════════════════════════════════════════
#  Summary
# ══════════════════════════════════════════════════════════════════════════════
$totalSec = [int]((Get-Date) - $StartTime).TotalSeconds

Write-Host ""
Write-Host ("=" * 62) -ForegroundColor Green
Write-Host "  DEPLOY COMPLETE" -ForegroundColor Green
Write-Host ("=" * 62) -ForegroundColor Green
Write-Host ""
Write-Host "  Target     : $IP"
Write-Host "  Completed  : $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
Write-Host ""
Write-Host "  Phase timings:"
foreach ($k in $Summary.Keys) {
    Write-Host ("    {0,-24} {1}" -f $k, $Summary[$k])
}
Write-Host ("    {0,-24} {1}s" -f "Total", $totalSec)
Write-Host ""
Write-Host "  Endpoints:"
Write-Host "    Frontend         http://${IP}:5010"
Write-Host "    API (REST)       http://${IP}:5006/api"
Write-Host "    API Health       http://${IP}:5006/api/health"
Write-Host "    OTLP gRPC        ${IP}:4317"
Write-Host ""
Write-Host "  Agent binaries (on server):"
Write-Host "    $RemoteDir/bin/dl/zentra-agent-linux-x86_64"
Write-Host "    $RemoteDir/bin/dl/zentra-collector-linux-x86_64"
Write-Host ""
Write-Host "  Login:"
Write-Host "    Email    : admin@Zentra.local"
Write-Host "    Password : changeme  (must change on first login)"
Write-Host ("=" * 62) -ForegroundColor Green
Write-Host ""
