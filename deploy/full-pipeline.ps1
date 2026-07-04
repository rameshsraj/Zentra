#!/usr/bin/env pwsh
# =============================================================
# Zentra Full Deploy Pipeline
#
# FLOW (run this for every code change):
#
#   STEP 1  Push source code: Windows git folder → WSL Ubuntu
#   STEP 2  Build + compile + quick test in WSL
#   STEP 3  Copy built artifacts (binary + frontend) → local artifacts/ folder
#   STEP 4  Push source code to GCP VM
#   STEP 5  Rebuild on GCP VM (cargo + vite + migrate + restart)
#   STEP 6  Quick smoke test on GCP VM
#
# Usage:
#   .\deploy\full-pipeline.ps1                   # full run (most common)
#   .\deploy\full-pipeline.ps1 -SkipSync         # skip step 1 (WSL already up to date)
#   .\deploy\full-pipeline.ps1 -SkipBuild        # skip steps 2-3 (artifacts already built)
#   .\deploy\full-pipeline.ps1 -SkipGcp          # steps 1-3 only, no GCP
#   .\deploy\full-pipeline.ps1 -FullSetup        # first-time GCP setup (installs all packages)
#   .\deploy\full-pipeline.ps1 -UploadOnly       # steps 1-4: build locally, upload built binaries to GCP, extract, stop
# =============================================================

param(
    [string]$ProjectId    = "project-95cdeea7-e2b2-4480-848",
    [string]$Instance     = "instance-20260629-111005",
    [string]$Zone         = "us-central1-a",
    [string]$VmUser       = "zentra",
    [string]$WslDistro    = "Ubuntu",
    [string]$WslRepoPath  = "/store/Zentra",
    [string]$WindowsRepo  = "C:\GitHub\Zentra",
    [string]$ArtifactRoot = "C:\GitHub\Zentra\artifacts\vm-build",
    [string]$GcpRemoteDir = "/store/Zentra",
    [switch]$SkipSync,
    [switch]$SkipBuild,
    [switch]$SkipGcp,
    [switch]$FullSetup,
    [switch]$UploadOnly   # Stop after SCP + extract on GCP (steps 1-4 only); no build/setup/smoke
)

$ErrorActionPreference = "Stop"
$ScriptDir = $PSScriptRoot

# ── helpers ───────────────────────────────────────────────────────────────────
function Step { param([string]$Msg) Write-Host "`n===[ $Msg ]===" -ForegroundColor Cyan }
function OK   { param([string]$Msg) Write-Host "  OK  $Msg" -ForegroundColor Green }
function Warn { param([string]$Msg) Write-Host "  !   $Msg" -ForegroundColor Yellow }
function Fail { param([string]$Msg) Write-Host "  ERR $Msg" -ForegroundColor Red; throw $Msg }

$UseWslGcloud = -not (Get-Command "gcloud" -ErrorAction SilentlyContinue)

function Convert-ToWslPath {
    param([string]$Path)
    $full = [System.IO.Path]::GetFullPath($Path)
    if ($full -match '^([A-Za-z]):\\(.*)$') {
        return "/mnt/$($matches[1].ToLower())/$($matches[2] -replace '\\','/')"
    }
    return ($full -replace '\\','/')
}

# Run gcloud -- falls back to WSL gcloud if not in Windows PATH
function Invoke-Gcloud {
    param([string[]]$CmdArgs)
    if ($UseWslGcloud) {
        $esc = ($CmdArgs | ForEach-Object { "'" + ($_ -replace "'","'\\''") + "'" }) -join " "
        & wsl -d $WslDistro -u root -- bash -lc "gcloud $esc"
    } else {
        & gcloud @CmdArgs
    }
    if ($LASTEXITCODE -ne 0) { Fail "gcloud failed: gcloud $($CmdArgs -join ' ')" }
}

# Run bash inside WSL -- base64-encodes the script to survive Windows CRLF
function Invoke-Wsl {
    param([string]$BashCmd, [string]$Label = "")
    if ($Label) { Write-Host "  -> $Label" -ForegroundColor DarkGray }
    $clean = $BashCmd -replace "`r`n", "`n" -replace "`r", "`n"
    $b64   = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($clean))
    & wsl -d $WslDistro -u root -- bash -lc "echo '$b64' | base64 -d | bash"
    if ($LASTEXITCODE -ne 0) { Fail "WSL command failed ($Label)" }
}

# Run a script on GCP via gcloud ssh -- base64-encodes to avoid quoting issues
function Invoke-GcpScript {
    param([string]$BashScript, [string]$Target, [string]$Zone)
    $b64 = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($BashScript))
    Invoke-Gcloud @("compute","ssh",$Target,"--zone",$Zone,"--command",
        "echo '$b64' | base64 -d > /tmp/zrun.sh && chmod +x /tmp/zrun.sh && sudo bash /tmp/zrun.sh && rm -f /tmp/zrun.sh")
}

$gcpTarget = if ([string]::IsNullOrWhiteSpace($VmUser)) { $Instance } else { "$VmUser@$Instance" }

Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  Zentra Full Deploy Pipeline" -ForegroundColor Cyan
Write-Host "  GCP : $gcpTarget  Zone: $Zone" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan

# =============================================================================
# STEP 1 -- Push source code: Windows git folder --> WSL Ubuntu
# =============================================================================
if (-not $SkipSync) {
    Step "STEP 1/6  Push source --> WSL $WslDistro : $WslRepoPath"

    $wslSrc = Convert-ToWslPath $WindowsRepo
    Invoke-Wsl "mkdir -p '$WslRepoPath'" "ensure WSL repo dir"

    Invoke-Wsl @"
rsync -a --delete \
  --exclude='target/' \
  --exclude='node_modules/' \
  --exclude='.git/' \
  --exclude='artifacts/' \
  --exclude='*.log' \
  --exclude='deploy/login.json' \
  --exclude='deploy/oraconn.json' \
  --exclude='.env' \
  '$wslSrc/' '$WslRepoPath/'
"@ "rsync source code"

    OK "Source synced to WSL $WslRepoPath"
} else {
    Warn "STEP 1 skipped -- using existing WSL source"
}

# =============================================================================
# STEP 2 -- Build + compile + quick test in WSL
# =============================================================================
if (-not $SkipBuild) {
    Step "STEP 2/6  Build + compile + test in WSL"

    # 2a -- Rust backend
    Write-Host "  Building Rust backend (cargo)..." -ForegroundColor Yellow
    Invoke-Wsl @"
set -euo pipefail
[[ -f /root/.cargo/env ]] && source /root/.cargo/env || true
if ! command -v rustup >/dev/null 2>&1; then
    echo "rustup not found -- installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source /root/.cargo/env
fi
rustup toolchain install stable --no-self-update 2>/dev/null || true
rustup default stable 2>/dev/null || true
export PATH="/root/.cargo/bin:`$PATH"
echo "rustc: `$(rustc --version)"
echo "cargo: `$(cargo --version)"
cd '$WslRepoPath'
cargo build --release --bin zentra-server
echo "Binary: `$(du -sh target/release/zentra-server | cut -f1)"
"@ "cargo build --release"
    OK "Backend compiled: $WslRepoPath/target/release/zentra-server"

    # 2b -- React frontend
    Write-Host "  Building React frontend (vite)..." -ForegroundColor Yellow
    Invoke-Wsl @"
set -euo pipefail
cd '$WslRepoPath/frontend'
npm install --legacy-peer-deps --silent
node node_modules/vite/bin/vite.js build
echo "Frontend files: `$(find dist -type f | wc -l)"
"@ "npm install + vite build"
    OK "Frontend compiled: $WslRepoPath/frontend/dist"

    # 2c -- Binary sanity check
    Invoke-Wsl @"
set -euo pipefail
BIN='$WslRepoPath/target/release/zentra-server'
[[ -x "`$BIN" ]] || { echo "Binary missing or not executable: `$BIN"; exit 1; }
echo "Binary check OK (`$(du -sh "`$BIN" | cut -f1))"
"@ "binary sanity check"
    OK "Binary test passed"
} else {
    Warn "STEP 2 skipped -- using existing WSL build"
}

# =============================================================================
# STEP 3 -- Copy built artifacts --> local artifacts/ folder
# =============================================================================
if (-not $SkipBuild) {
    Step "STEP 3/6  Copy artifacts --> $ArtifactRoot"

    & "$ScriptDir\pull-from-wsl.ps1" `
        -WslDistro       $WslDistro `
        -WslRepoPath     $WslRepoPath `
        -LocalOutputRoot $ArtifactRoot
    if ($LASTEXITCODE -ne 0) { Fail "pull-from-wsl.ps1 failed" }

    $exportDir = Join-Path $ArtifactRoot "zentra-export"
    $binItem   = Get-Item "$exportDir\zentra-server" -ErrorAction SilentlyContinue
    $feCount   = @(Get-ChildItem "$exportDir\frontend-dist" -Recurse -File -ErrorAction SilentlyContinue).Count
    $migCount  = @(Get-ChildItem "$exportDir\migrations" -Filter "*.sql" -ErrorAction SilentlyContinue).Count
    $binMB     = if ($binItem) { [math]::Round($binItem.Length / 1MB, 1) } else { "?" }
    OK "Binary: ${binMB} MB  |  Frontend: $feCount files  |  Migrations: $migCount SQL files"
} else {
    Warn "STEP 3 skipped -- using existing $ArtifactRoot\zentra-export"
}

if ($SkipGcp) {
    Warn "STEP 4-6 skipped (-SkipGcp)"
    Write-Host ""
    Write-Host "  Pipeline complete (local build only)." -ForegroundColor Green
    exit 0
}

Invoke-Gcloud @("config","set","project",$ProjectId)

# =============================================================================
# STEP 4 -- Push to GCP VM
#   -UploadOnly : tars the built artifact (zentra-export) and scps binaries only
#   normal      : tars source code so GCP VM can recompile
# =============================================================================
if ($UploadOnly) {
    Step "STEP 4/6  Upload built binaries --> GCP VM ($Instance)"

    $exportDir  = Join-Path $ArtifactRoot "zentra-export"
    if (-not (Test-Path "$exportDir\zentra-server")) { Fail "Built artifacts not found at $exportDir -- run without -SkipBuild first" }

    $tarName    = "zentra-bin.tar.gz"
    $winTarPath = Join-Path $env:TEMP $tarName
    $wslExport  = Convert-ToWslPath $exportDir
    $wslTarWin  = Convert-ToWslPath $winTarPath

    Write-Host "  Creating binary archive from artifacts..." -ForegroundColor Yellow
    Invoke-Wsl @"
set -euo pipefail
cd '$wslExport/..'
tar -czf '$wslTarWin' zentra-export/
echo "Archive: `$(du -sh '$wslTarWin' | cut -f1)"
"@ "tar zentra-export"

    if (-not (Test-Path $winTarPath)) { Fail "Binary archive not found at $winTarPath" }

    Write-Host "  Uploading binaries to GCP VM..." -ForegroundColor Yellow
    $scpSrc = if ($UseWslGcloud) { Convert-ToWslPath $winTarPath } else { $winTarPath }
    Invoke-Gcloud @("compute","scp","--zone",$Zone,$scpSrc,"${gcpTarget}:/tmp/$tarName")

    Write-Host "  Extracting on GCP VM..." -ForegroundColor Yellow
    Invoke-Gcloud @("compute","ssh",$gcpTarget,"--zone",$Zone,"--command",
        "sudo mkdir -p '$GcpRemoteDir' && sudo tar -xzf /tmp/$tarName --strip-components=1 -C '$GcpRemoteDir' && sudo rm /tmp/$tarName && echo 'Extracted OK'")

    OK "Binaries uploaded and extracted to GCP $GcpRemoteDir"
    Write-Host ""
    Write-Host "  Stopped (-UploadOnly). Steps 5-6 skipped." -ForegroundColor Yellow
    Write-Host "  To continue manually on the VM:" -ForegroundColor DarkGray
    Write-Host "    gcloud compute ssh $gcpTarget --zone $Zone" -ForegroundColor DarkGray
    Write-Host ""
    exit 0
}

Step "STEP 4/6  Push source code --> GCP VM ($Instance)"

$tarName    = "zentra-src.tar.gz"
$wslTarPath = "/tmp/$tarName"
$winTarPath = Join-Path $env:TEMP $tarName

Write-Host "  Creating source archive in WSL..." -ForegroundColor Yellow
Invoke-Wsl @"
set -euo pipefail
cd '$WslRepoPath'
tar -czf '$wslTarPath' \
  --exclude='target' \
  --exclude='node_modules' \
  --exclude='.git' \
  --exclude='artifacts' \
  --exclude='docker' \
  --exclude='*.log' \
  --exclude='deploy/login.json' \
  --exclude='deploy/oraconn.json' \
  --exclude='.env' \
  .
echo "Archive: `$(du -sh '$wslTarPath' | cut -f1)"
"@ "tar source tree"

# Copy tar from WSL to Windows TEMP so gcloud scp can reach it
$wslTarWin = Convert-ToWslPath $winTarPath
Invoke-Wsl "cp '$wslTarPath' '$wslTarWin'" "copy tar to Windows TEMP"

if (-not (Test-Path $winTarPath)) { Fail "Source archive not found at $winTarPath" }

Write-Host "  Uploading to GCP VM..." -ForegroundColor Yellow
    $scpSrc = if ($UseWslGcloud) { Convert-ToWslPath $winTarPath } else { $winTarPath }
    Invoke-Gcloud @("compute","scp","--zone",$Zone,$scpSrc,"${gcpTarget}:/tmp/$tarName")

    Write-Host "  Extracting on GCP VM..." -ForegroundColor Yellow
    Invoke-Gcloud @("compute","ssh",$gcpTarget,"--zone",$Zone,"--command",
        "sudo mkdir -p '$GcpRemoteDir' && sudo tar -xzf /tmp/$tarName -C '$GcpRemoteDir' && sudo rm /tmp/$tarName && echo 'Extracted OK'")
OK "Source code pushed to GCP $GcpRemoteDir"

# =============================================================================
# STEP 5 -- Rebuild on GCP VM  (cargo + vite + migrate + restart)
# =============================================================================
Step "STEP 5/6  Rebuild on GCP VM"

if ($FullSetup) {
    Write-Host "  Running full setup_server.sh (first-time install)..." -ForegroundColor Yellow
    $setupLocal = "$ScriptDir\setup_server.sh"
    $scpSetup   = if ($UseWslGcloud) { Convert-ToWslPath $setupLocal } else { $setupLocal }
    Invoke-Gcloud @("compute","scp","--zone",$Zone,$scpSetup,"${gcpTarget}:/tmp/setup_server.sh")
    Invoke-Gcloud @("compute","ssh",$gcpTarget,"--zone",$Zone,"--command",
        "chmod +x /tmp/setup_server.sh && sudo bash /tmp/setup_server.sh")
    OK "Full server setup complete"
} else {
    Write-Host "  Compiling + restarting services on GCP VM..." -ForegroundColor Yellow

    $rebuildScript = @'
#!/usr/bin/env bash
set -euo pipefail
DEPLOY="/store/Zentra"
mkdir -p "$DEPLOY/logs" "$DEPLOY/bin"

echo "--- [5a] Rust build ---"
[[ -f /root/.cargo/env ]] && source /root/.cargo/env || true
if ! command -v rustup >/dev/null 2>&1; then
    echo "Installing rustup on GCP VM..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source /root/.cargo/env
fi
rustup toolchain install stable --no-self-update 2>/dev/null || true
rustup default stable 2>/dev/null || true
export PATH="/root/.cargo/bin:$PATH"
echo "rustc: $(rustc --version)"
cd "$DEPLOY"
cargo build --release --bin zentra-server 2>&1 | tee -a "$DEPLOY/logs/deploy.log"
install -m 755 target/release/zentra-server bin/zentra-server
echo "Binary installed: $(du -sh bin/zentra-server | cut -f1)"

echo "--- [5b] Frontend build ---"
cd "$DEPLOY/frontend"
npm install --legacy-peer-deps --silent
node node_modules/vite/bin/vite.js build
echo "Frontend files: $(find dist -type f | wc -l)"

echo "--- [5c] Apply migrations ---"
source "$DEPLOY/.env" 2>/dev/null || true
PG_PORT="${ZENTRA_PG_PORT:-5434}"
PG_DB="${POSTGRES_DB:-zentra}"
PG_USER="${POSTGRES_USER:-postgres}"
PG_PASS="${POSTGRES_PASSWORD:-zantaz}"

PGPASSWORD="$PG_PASS" psql -v ON_ERROR_STOP=1 \
    -h 127.0.0.1 -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" \
    -c "CREATE TABLE IF NOT EXISTS zentra_schema_migrations (
          filename TEXT PRIMARY KEY,
          applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        );" 2>/dev/null || true

shopt -s nullglob
for m in "$DEPLOY/migrations/"*.sql; do
    name="$(basename "$m")"
    applied="$(PGPASSWORD="$PG_PASS" psql -tAc \
        "SELECT 1 FROM zentra_schema_migrations WHERE filename='$name'" \
        -h 127.0.0.1 -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" 2>/dev/null | tr -d '[:space:]')"
    if [[ "$applied" == "1" ]]; then
        echo "  skip (applied): $name"; continue
    fi
    echo "  apply: $name"
    PGPASSWORD="$PG_PASS" psql -v ON_ERROR_STOP=1 \
        -h 127.0.0.1 -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -f "$m"
    PGPASSWORD="$PG_PASS" psql -v ON_ERROR_STOP=1 \
        -h 127.0.0.1 -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" \
        -c "INSERT INTO zentra_schema_migrations(filename) VALUES ('$name') ON CONFLICT DO NOTHING;"
done
echo "Migrations done."

echo "--- [5d] Restart services ---"
chown -R zentra:zentra "$DEPLOY" 2>/dev/null || true

if systemctl is-enabled zentra-api >/dev/null 2>&1; then
    systemctl restart zentra-api
    sleep 5
    if systemctl is-active zentra-api >/dev/null 2>&1; then
        echo "zentra-api: active"
    else
        systemctl status zentra-api --no-pager | tail -20
        echo "ERROR: zentra-api failed to start -- check $DEPLOY/logs/api.log"
        exit 1
    fi
else
    pkill -f zentra-server 2>/dev/null || true
    sleep 2
    set -a; source "$DEPLOY/.env"; set +a
    nohup "$DEPLOY/bin/zentra-server" >> "$DEPLOY/logs/api.log" 2>&1 &
    echo "zentra-server started (pid $!)"
    sleep 5
fi

systemctl reload nginx 2>/dev/null || systemctl restart nginx 2>/dev/null || true
echo "nginx: $(systemctl is-active nginx 2>/dev/null || echo ok)"
'@
    # NOTE: $rebuildScript uses single-quote heredoc above so PowerShell variables
    # are not expanded -- the bash script runs as written on the GCP VM.
    Invoke-GcpScript -BashScript $rebuildScript -Target $gcpTarget -Zone $Zone
    OK "Backend + frontend rebuilt and services restarted on GCP"
}

# =============================================================================
# STEP 6 -- Quick smoke test on GCP VM
# =============================================================================
Step "STEP 6/6  Smoke test"

$smokeScript = @'
#!/usr/bin/env bash
set -uo pipefail
BASE="http://127.0.0.1:5006"
FE="http://127.0.0.1:5010"
PASS=0; FAIL=0

check() {
    local name="$1" url="$2" expected="${3:-200}"
    code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 10 "$url" 2>/dev/null || echo "000")
    if [[ "$code" == "$expected" ]]; then
        echo "  PASS: $name  (HTTP $code)"; ((PASS++)) || true
    else
        echo "  FAIL: $name  (expected $expected, got $code)"; ((FAIL++)) || true
    fi
}

echo ""
check "API health"       "$BASE/api/health"     "200"
check "Login endpoint"   "$BASE/api/auth/login" "405"
check "Frontend (nginx)" "$FE"                  "200"

HEALTH=$(curl -s --max-time 5 "$BASE/api/health" 2>/dev/null || echo "{}")
if echo "$HEALTH" | grep -qE '"status"\s*:\s*"(ok|healthy)"'; then
    echo "  PASS: DB connected (health body OK)"; ((PASS++)) || true
else
    echo "  WARN: DB status unknown -- health body: $HEALTH"; ((FAIL++)) || true
fi

echo ""
echo "  Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
'@

try {
    Invoke-GcpScript -BashScript $smokeScript -Target $gcpTarget -Zone $Zone
    OK "All smoke tests passed"
} catch {
    Warn "Smoke test failures -- to debug on GCP VM run:"
    Warn "  gcloud compute ssh $gcpTarget --zone $Zone --command 'tail -80 /store/Zentra/logs/api.log'"
}

Write-Host ""
Write-Host "============================================" -ForegroundColor Green
Write-Host "  Pipeline complete!" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
