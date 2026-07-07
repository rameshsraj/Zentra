# Direct WSL pull script - bypass VM, pull artifacts straight from WSL to Windows
#
# The output layout matches what install-from-binaries.sh expects:
#
#   artifacts\vm-build\zentra-export\
#     zentra-server          <- binary
#     frontend-dist\         <- built React app
#     migrations\            <- SQL files
#     .env.example           <- reference config
#
# To create a client release package, zip or tar the zentra-export\ folder
# and give the client install-from-binaries.sh alongside it.

param(
    [string]$WslDistro = "Ubuntu",
    [string]$WslRepoPath = "/store/Zentra",
    [string]$LocalOutputRoot = "C:\GitHub\Zentra\artifacts\vm-build"
)

$ErrorActionPreference = "Stop"

function Assert-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' not found in PATH."
    }
}

Assert-Command "tar"

# WSL paths are accessible via \\wsl$\distro\path
# Use Join-Path + string Replace to correctly build backslash-only UNC paths
$wslPrefix  = "\\wsl$\$WslDistro"
$wslRepoUNC = Join-Path $wslPrefix ($WslRepoPath.TrimStart('/').Replace('/', '\'))

Write-Host "Checking WSL repo artifacts at: $wslRepoUNC" -ForegroundColor Cyan

# Check if backend binary exists
$binaryPath = "$wslRepoUNC\target\release\zentra-server"
if (-not (Test-Path $binaryPath)) {
    throw "Backend binary not found at: $binaryPath"
}
$binItem = Get-Item $binaryPath -ErrorAction SilentlyContinue
if ($binItem) {
    Write-Host "✓ Backend binary found: $($binItem.Length) bytes" -ForegroundColor Green
}

# Check if frontend build exists
$frontendPath = "$wslRepoUNC\frontend\dist"
if (-not (Test-Path $frontendPath)) {
    throw "Frontend dist folder not found at: $frontendPath"
}
Write-Host "✓ Frontend build found at: $frontendPath" -ForegroundColor Green

# Check if migrations exist
$migrationsPath = "$wslRepoUNC\migrations"
$migCount = @(Get-ChildItem "$migrationsPath" -Filter "*.sql" -ErrorAction SilentlyContinue | Measure-Object).Count
if ($migCount -eq 0) {
    throw "Migration files not found at: $migrationsPath"
}
Write-Host "✓ Found $migCount migration files" -ForegroundColor Green

# Create output structure
# Layout matches install-from-binaries.sh expectations:
#   zentra-export/
#     zentra-server        <- binary at top level
#     frontend-dist/       <- static frontend
#     migrations/          <- SQL files
#     .env.example
Write-Host "`nPreparing output directory..." -ForegroundColor Cyan
$exportDir  = "$LocalOutputRoot\zentra-export"
$migDir     = "$exportDir\migrations"

Remove-Item -Path $exportDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $exportDir -Force | Out-Null
New-Item -ItemType Directory -Path $migDir   -Force | Out-Null

# Copy binary (single file -- UNC works fine for this)
Write-Host "Copying backend binary..." -ForegroundColor Cyan
Copy-Item $binaryPath "$exportDir\zentra-server" -Force
$copiedBinary = Get-Item "$exportDir\zentra-server"
Write-Host "✓ Copied: $($copiedBinary.Length) bytes" -ForegroundColor Green

# Copy frontend -- use wsl cp because UNC deep-tree recursion is unreliable
Write-Host "Copying frontend artifacts..." -ForegroundColor Cyan
$frontendDestDir = "$exportDir\frontend-dist"
New-Item -ItemType Directory -Path $frontendDestDir -Force | Out-Null
# Convert Windows paths to WSL /mnt/ paths for the wsl cp command
$wslFrontendSrc  = "$WslRepoPath/frontend/dist/."
$wslDestDrive    = ($frontendDestDir -replace '^([A-Za-z]):\\', { "/mnt/$($_.Groups[1].Value.ToLower())/" }) -replace '\\','/'
& wsl -d $WslDistro -u root -- bash -lc "cp -a '$wslFrontendSrc' '$wslDestDrive'"
if ($LASTEXITCODE -ne 0) { throw "wsl cp failed for frontend dist" }
$frontendFiles = [int](& wsl -d $WslDistro -u root -- bash -lc "find '$wslDestDrive' -type f | wc -l")
Write-Host "✓ Copied $frontendFiles frontend files" -ForegroundColor Green

# Copy migrations -- use wsl cp for same reason
Write-Host "Copying migration files..." -ForegroundColor Cyan
$wslMigSrc  = "$WslRepoPath/migrations"
$wslMigDest = ($migDir -replace '^([A-Za-z]):\\', { "/mnt/$($_.Groups[1].Value.ToLower())/" }) -replace '\\','/'
& wsl -d $WslDistro -u root -- bash -lc "cp '$wslMigSrc'/*.sql '$wslMigDest/' 2>/dev/null || true"
$copiedMigs = @(Get-ChildItem "$migDir" -Filter "*.sql" | Measure-Object).Count
Write-Host "✓ Copied $copiedMigs migration files" -ForegroundColor Green

# Create .env.example
Write-Host "Creating .env.example..." -ForegroundColor Cyan
$envExample = @"
DATABASE_URL=postgresql://postgres:zantaz@127.0.0.1:5434/zentra
ZENTRA_API_PORT=5006
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=ZentraSecureAPIKey2026!@#XyZ9p2qR7mW4nV
ZENTRA_MASTER_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
RUST_LOG=info
"@
$envExample | Out-File "$exportDir\.env.example" -Encoding UTF8 -Force
Write-Host "✓ Created .env.example" -ForegroundColor Green

# Create tar.gz archive (the client release package)
Write-Host "`nCreating release archive..." -ForegroundColor Cyan
$tarPath = Join-Path $LocalOutputRoot "zentra-release.tar.gz"
Remove-Item $tarPath -Force -ErrorAction SilentlyContinue
$tarResult = & tar -czf $tarPath -C $LocalOutputRoot "zentra-export" 2>&1
if ($LASTEXITCODE -eq 0 -and (Test-Path $tarPath)) {
    $tarSize = [math]::Round((Get-Item $tarPath).Length / 1KB, 0)
    Write-Host "✓ Release archive: $tarPath (${tarSize} KB)" -ForegroundColor Green
} else {
    Write-Host "! Archive creation failed (non-fatal): $tarResult" -ForegroundColor Yellow
    $global:LASTEXITCODE = 0
}

# Summary
Write-Host "`n========== SUMMARY ==========" -ForegroundColor Cyan
Write-Host "Release folder  : $exportDir" -ForegroundColor White
Write-Host "  zentra-server        <- binary" -ForegroundColor White
Write-Host "  frontend-dist/       <- static frontend ($frontendFiles files)" -ForegroundColor White
Write-Host "  migrations/          <- SQL files ($copiedMigs files)" -ForegroundColor White
Write-Host "  .env.example         <- config template" -ForegroundColor White
Write-Host "`nRelease archive : $tarPath" -ForegroundColor White
Write-Host ""
Write-Host "To install on a client server:" -ForegroundColor Cyan
Write-Host "  1. Copy zentra-release.tar.gz + install-from-binaries.sh to the server" -ForegroundColor White
Write-Host "  2. tar -xzf zentra-release.tar.gz" -ForegroundColor White
Write-Host "  3. sudo bash install-from-binaries.sh --binary-dir ./zentra-export" -ForegroundColor White
Write-Host ""
Write-Host "Optional flags:" -ForegroundColor Cyan
Write-Host "  --install-dir /opt/zentra   (where to install, default: /opt/zentra)" -ForegroundColor White
Write-Host "  --web-port 8080             (nginx port, default: 5010)" -ForegroundColor White
Write-Host "  --pg-port 5434              (postgres port, default: 5434)" -ForegroundColor White
Write-Host "  --pg-password mypassword    (postgres password)" -ForegroundColor White
Write-Host "  --dry-run                   (preview without making changes)" -ForegroundColor White
