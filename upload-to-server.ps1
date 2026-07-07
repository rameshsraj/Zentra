#!/usr/bin/env pwsh
# upload-to-server.ps1
# Uploads a local source folder to a remote Linux server via SCP/SSH (using plink/pscp or ssh/scp).
#
# Usage:
#   .\upload-to-server.ps1 -SourceFolder "C:\Projects\Zentra" -ServerHost "10.88.11.162" -ServerPath "/store/Zentra" -Username "root" -Password "skyline"
#   .\upload-to-server.ps1 -SourceFolder "C:\Projects\Zentra" -ServerHost "10.88.11.162" -ServerPath "/store/Zentra" -Username "root" -Password "skyline" -Port 22

param(
    [Parameter(Mandatory = $true,  HelpMessage = "Local source folder to upload")]
    [string]$SourceFolder,

    [Parameter(Mandatory = $true,  HelpMessage = "Remote server hostname or IP")]
    [string]$ServerHost,

    [Parameter(Mandatory = $true,  HelpMessage = "Remote path on the server")]
    [string]$ServerPath,

    [Parameter(Mandatory = $true,  HelpMessage = "SSH username")]
    [string]$Username,

    [Parameter(Mandatory = $true,  HelpMessage = "SSH password")]
    [string]$Password,

    [Parameter(Mandatory = $false, HelpMessage = "SSH port (default: 22)")]
    [int]$Port = 22
)

# ── Helpers ───────────────────────────────────────────────────────────────────
function Write-Info  { param($msg) Write-Host "`n>>> $msg" -ForegroundColor Cyan }
function Write-Ok    { param($msg) Write-Host "    ✓  $msg" -ForegroundColor Green }
function Write-Fail  { param($msg) Write-Host "    ✗  $msg" -ForegroundColor Red }
function Write-Step  { param($msg) Write-Host "    →  $msg" -ForegroundColor Yellow }

# ── Validate source folder ────────────────────────────────────────────────────
if (-not (Test-Path $SourceFolder)) {
    Write-Fail "Source folder not found: $SourceFolder"
    exit 1
}
$SourceFolder = (Resolve-Path $SourceFolder).Path.TrimEnd('\').TrimEnd('/')

# ── Detect upload tool ────────────────────────────────────────────────────────
# Prefer rsync (via WSL or native), then scp, then pscp (PuTTY)
$UseRsync  = $false
$UseScp    = $false
$UsePscp   = $false
$UseWslRsync = $false

if (Get-Command rsync -ErrorAction SilentlyContinue) {
    $UseRsync = $true
} elseif (Get-Command wsl -ErrorAction SilentlyContinue) {
    $wslCheck = wsl which rsync 2>$null
    if ($wslCheck) { $UseWslRsync = $true }
}

if (-not $UseRsync -and -not $UseWslRsync) {
    if (Get-Command scp -ErrorAction SilentlyContinue) {
        $UseScp = $true
    } elseif (Get-Command pscp -ErrorAction SilentlyContinue) {
        $UsePscp = $true
    } else {
        Write-Fail "No upload tool found. Install one of: rsync, OpenSSH (scp), or PuTTY (pscp)."
        Write-Step "Install OpenSSH: Settings → Apps → Optional Features → OpenSSH Client"
        Write-Step "Install PuTTY:   https://www.putty.org/"
        exit 1
    }
}

# ── Print summary ─────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Zentra Upload Script" -ForegroundColor Cyan
Write-Host "  Source : $SourceFolder" -ForegroundColor White
Write-Host "  Target : ${Username}@${ServerHost}:${ServerPath}" -ForegroundColor White
Write-Host "  Port   : $Port" -ForegroundColor White
Write-Host "======================================================" -ForegroundColor Cyan

# ── Excludes (mirrors the bash script) ───────────────────────────────────────
$Excludes = @("target", "node_modules", ".git", "*.log")

# ── Upload ────────────────────────────────────────────────────────────────────
Write-Info "Uploading source files to server..."

if ($UseRsync) {
    Write-Step "Using rsync"
    $excludeArgs = $Excludes | ForEach-Object { "--exclude=$_" }
    $rsyncArgs = @(
        "-az", "--progress",
        "-e", "ssh -p $Port -o StrictHostKeyChecking=no"
    ) + $excludeArgs + @(
        "$SourceFolder/",
        "${Username}@${ServerHost}:${ServerPath}/"
    )
    # rsync with password via sshpass if available
    if (Get-Command sshpass -ErrorAction SilentlyContinue) {
        & sshpass -p $Password rsync @rsyncArgs
    } else {
        Write-Step "sshpass not found — rsync may prompt for password"
        & rsync @rsyncArgs
    }

} elseif ($UseWslRsync) {
    Write-Step "Using rsync via WSL"
    # Convert Windows path to WSL path
    $wslSource = ($SourceFolder -replace '\\', '/') -replace '^([A-Za-z]):', '/mnt/$1'.ToLower()
    $wslSource = $wslSource -replace '^/mnt/([A-Za-z])', { "/mnt/$($_.Groups[1].Value.ToLower())" }
    $excludeStr = ($Excludes | ForEach-Object { "--exclude=$_" }) -join " "
    $wslCmd = "sshpass -p '$Password' rsync -az --progress $excludeStr -e 'ssh -p $Port -o StrictHostKeyChecking=no' '$wslSource/' '${Username}@${ServerHost}:${ServerPath}/'"
    wsl bash -c $wslCmd

} elseif ($UseScp) {
    Write-Step "Using scp (OpenSSH)"
    # scp doesn't support excludes natively — create a temp filtered copy
    $TempDir = Join-Path $env:TEMP "zentra_upload_$(Get-Random)"
    Write-Step "Creating filtered temp copy at $TempDir"
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null

    # Use robocopy to mirror with excludes
    $robocopyExcludeDirs = $Excludes | Where-Object { $_ -notlike "*.log" }
    $robocopyExcludeFiles = $Excludes | Where-Object { $_ -like "*.*" }
    $robocopyArgs = @($SourceFolder, $TempDir, "/MIR", "/NP", "/NFL", "/NDL", "/NJH")
    if ($robocopyExcludeDirs) { $robocopyArgs += "/XD"; $robocopyArgs += $robocopyExcludeDirs }
    if ($robocopyExcludeFiles) { $robocopyArgs += "/XF"; $robocopyArgs += $robocopyExcludeFiles }
    & robocopy @robocopyArgs | Out-Null

    # Now scp the filtered copy
    $Env:SSHPASS = $Password
    $scpArgs = @(
        "-P", $Port,
        "-o", "StrictHostKeyChecking=no",
        "-o", "PubkeyAuthentication=no",
        "-r",
        "$TempDir/",
        "${Username}@${ServerHost}:${ServerPath}/"
    )
    if (Get-Command sshpass -ErrorAction SilentlyContinue) {
        & sshpass -p $Password scp @scpArgs
    } else {
        Write-Step "Tip: set SSH_ASKPASS or use PuTTY's pscp for password-based SCP without prompts"
        & scp @scpArgs
    }

    # Cleanup temp
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue

} elseif ($UsePscp) {
    Write-Step "Using pscp (PuTTY)"
    # pscp: -r recursive, -P port, -pw password, -batch no prompts
    $pscpArgs = @(
        "-r",
        "-P", $Port,
        "-pw", $Password,
        "-batch",
        "$SourceFolder\",
        "${Username}@${ServerHost}:${ServerPath}/"
    )
    & pscp @pscpArgs
}

if ($LASTEXITCODE -ne 0) {
    Write-Fail "Upload failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Ok "Upload complete → ${Username}@${ServerHost}:${ServerPath}"

# ── Optional: trigger the remote build script ─────────────────────────────────
Write-Host ""
$RunBuild = Read-Host "Run the remote build script now? (./restart-services.sh) [y/N]"
if ($RunBuild -match '^[Yy]') {
    Write-Info "Triggering remote build..."
    $sshCmd = "cd $ServerPath && bash restart-services.sh"

    if (Get-Command sshpass -ErrorAction SilentlyContinue) {
        & sshpass -p $Password ssh -p $Port `
            -o StrictHostKeyChecking=no `
            -o PubkeyAuthentication=no `
            "${Username}@${ServerHost}" $sshCmd
    } elseif (Get-Command plink -ErrorAction SilentlyContinue) {
        & plink -P $Port -pw $Password -batch "${Username}@${ServerHost}" $sshCmd
    } else {
        Write-Step "Neither sshpass nor plink found — cannot auto-run remote build."
        Write-Step "SSH in manually and run: cd $ServerPath && bash restart-services.sh"
    }
}

Write-Host ""
Write-Host "======================================================" -ForegroundColor Cyan
Write-Host "  Done!" -ForegroundColor Green
Write-Host "  Server: ${Username}@${ServerHost}:${ServerPath}" -ForegroundColor White
Write-Host "======================================================" -ForegroundColor Cyan
