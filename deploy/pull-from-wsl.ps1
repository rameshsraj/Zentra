# Direct WSL pull script - bypass VM, pull artifacts straight from WSL to Windows

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
$wslPrefix = "\\wsl$\$WslDistro"
$wslRepoUNC = "$wslPrefix$WslRepoPath"

Write-Host "Checking WSL repo artifacts at: $wslRepoUNC" -ForegroundColor Cyan

# Check if backend binary exists
$binaryPath = "$wslRepoUNC/target/release/zentra-server"
if (-not (Test-Path $binaryPath)) {
    throw "Backend binary not found at: $binaryPath"
}
$binItem = Get-Item $binaryPath -ErrorAction SilentlyContinue
if ($binItem) {
    Write-Host "✓ Backend binary found: $($binItem.Length) bytes" -ForegroundColor Green
}

# Check if frontend build exists
$frontendPath = "$wslRepoUNC/frontend/dist"
if (-not (Test-Path $frontendPath)) {
    throw "Frontend dist folder not found at: $frontendPath"
}
Write-Host "✓ Frontend build found at: $frontendPath" -ForegroundColor Green

# Check if migrations exist
$migrationsPath = "$wslRepoUNC/migrations"
$migCount = @(Get-ChildItem "$migrationsPath" -Filter "*.sql" -ErrorAction SilentlyContinue | Measure-Object).Count
if ($migCount -eq 0) {
    throw "Migration files not found at: $migrationsPath"
}
Write-Host "✓ Found $migCount migration files" -ForegroundColor Green

# Create output structure
Write-Host "`nPreparing output directory..." -ForegroundColor Cyan
$buildDir = "$LocalOutputRoot/zentra-export/build"
$migDir = "$LocalOutputRoot/zentra-export/migrations"

Remove-Item -Path "$LocalOutputRoot/zentra-export" -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $buildDir -Force | Out-Null
New-Item -ItemType Directory -Path $migDir -Force | Out-Null

# Copy binary
Write-Host "Copying backend binary..." -ForegroundColor Cyan
Copy-Item $binaryPath "$buildDir/zentra-server" -Force
$copiedBinary = Get-Item "$buildDir/zentra-server"
Write-Host "✓ Copied: $($copiedBinary.Length) bytes" -ForegroundColor Green

# Copy frontend
Write-Host "Copying frontend artifacts..." -ForegroundColor Cyan
$frontendDestDir = "$buildDir/frontend-dist"
New-Item -ItemType Directory -Path $frontendDestDir -Force | Out-Null
$frontendItems = @(Get-ChildItem "$frontendPath" -Force -ErrorAction SilentlyContinue)
foreach ($item in $frontendItems) {
    Copy-Item $item.FullName $frontendDestDir -Recurse -Force -ErrorAction SilentlyContinue
}
$frontendFiles = @(Get-ChildItem $frontendDestDir -Recurse | Measure-Object).Count
Write-Host "✓ Copied $frontendFiles frontend files" -ForegroundColor Green

# Copy migrations
Write-Host "Copying migration files..." -ForegroundColor Cyan
$migrationFiles = @(Get-ChildItem "$migrationsPath" -Filter "*.sql" -Force -ErrorAction SilentlyContinue)
foreach ($migFile in $migrationFiles) {
    Copy-Item $migFile.FullName $migDir -Force
}
$copiedMigs = @(Get-ChildItem "$migDir" -Filter "*.sql" | Measure-Object).Count
Write-Host "✓ Copied $copiedMigs migration files" -ForegroundColor Green

# Create .env.example
Write-Host "Creating .env.example..." -ForegroundColor Cyan
$envExample = @"
DATABASE_URL=postgresql://postgres:zentra@127.0.0.1:55434/zentra
ZENTRA_API_PORT=5006
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=change_me_min_32_chars
ZENTRA_MASTER_KEY=change_me_64_hex_chars
RUST_LOG=info
"@
$envExample | Out-File "$LocalOutputRoot/zentra-export/.env.example" -Encoding UTF8 -Force
Write-Host "✓ Created .env.example" -ForegroundColor Green

# Create tar.gz archive
Write-Host "`nCreating compressed archive..." -ForegroundColor Cyan
$tarPath = "$LocalOutputRoot/zentra-export.tar.gz"
Remove-Item $tarPath -Force -ErrorAction SilentlyContinue
Push-Location "$LocalOutputRoot"
& tar -czf "zentra-export.tar.gz" "zentra-export/"
Pop-Location
if (Test-Path $tarPath) {
    $tarSize = Get-Item $tarPath | Select-Object -ExpandProperty Length
    Write-Host "✓ Archive created: $tarPath ($tarSize bytes)" -ForegroundColor Green
} else {
    throw "Failed to create tar.gz archive"
}

# Summary
Write-Host "`n========== SUMMARY ==========" -ForegroundColor Cyan
Write-Host "Output directory: $LocalOutputRoot/zentra-export/" -ForegroundColor White
Write-Host "  - Backend binary: build/zentra-server" -ForegroundColor White
Write-Host "  - Frontend assets: build/frontend-dist/" -ForegroundColor White
Write-Host "  - Migrations: migrations/*.sql" -ForegroundColor White
Write-Host "  - Env template: .env.example" -ForegroundColor White
Write-Host "`nArchive: $tarPath" -ForegroundColor White
Write-Host "`nArtifacts ready for GCP deployment!" -ForegroundColor Green
