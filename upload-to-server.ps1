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

function New-FilteredTempCopy {
    param(
        [string]$InputFolder,
        [string[]]$ExcludePatterns
    )

    $tempDir = Join-Path $env:TEMP "zentra_upload_$(Get-Random)"
    Write-Step "Creating filtered temp copy at $tempDir"
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

    $robocopyExcludeDirs = $ExcludePatterns | Where-Object { $_ -notlike "*.*" }
    $robocopyExcludeFiles = $ExcludePatterns | Where-Object { $_ -like "*.*" }
    $robocopyArgs = @($InputFolder, $tempDir, "/MIR", "/NP", "/NFL", "/NDL", "/NJH")
    if ($robocopyExcludeDirs) { $robocopyArgs += "/XD"; $robocopyArgs += $robocopyExcludeDirs }
    if ($robocopyExcludeFiles) { $robocopyArgs += "/XF"; $robocopyArgs += $robocopyExcludeFiles }
    & robocopy @robocopyArgs | Out-Null

    return $tempDir
}

# ── Validate source folder ────────────────────────────────────────────────────
if (-not (Test-Path $SourceFolder)) {
    Write-Fail "Source folder not found: $SourceFolder"
    exit 1
}
$SourceFolder = (Resolve-Path $SourceFolder).Path.TrimEnd('\').TrimEnd('/')

# ── Detect upload tool ────────────────────────────────────────────────────────
# Priority when a password is supplied:
#   1. native rsync  (rare on Windows)
#   2. pscp (PuTTY)  — supports -pw natively, no sshpass needed
#   3. scp           — only usable non-interactively if sshpass is present
#   4. WSL rsync     — last resort; may not reach all network targets
$UseRsync = $false
$UseScp = $false
$UsePscp = $false
$UseWslRsync = $false

if (Get-Command rsync -ErrorAction SilentlyContinue) {
    $UseRsync = $true
} elseif (Get-Command pscp -ErrorAction SilentlyContinue) {
    $UsePscp = $true
} elseif (Get-Command scp -ErrorAction SilentlyContinue) {
    if (-not (Get-Command sshpass -ErrorAction SilentlyContinue)) {
        Write-Step "scp found but sshpass is not available — password cannot be passed non-interactively."
        Write-Step "Install PuTTY (pscp) for password-based uploads: https://www.putty.org/"
    }
    $UseScp = $true
} elseif (Get-Command wsl -ErrorAction SilentlyContinue) {
    $wslCheck = wsl which rsync 2>$null
    if ($wslCheck) { $UseWslRsync = $true }
}

if (-not $UseRsync -and -not $UseScp -and -not $UsePscp -and -not $UseWslRsync) {
    Write-Fail "No upload tool found. Install one of: rsync, OpenSSH (scp), PuTTY (pscp), or WSL rsync."
    Write-Step "Install OpenSSH: Settings → Apps → Optional Features → OpenSSH Client"
    Write-Step "Install PuTTY:   https://www.putty.org/"
    exit 1
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
    # scp doesn't support excludes natively — upload a filtered temp copy
    $TempDir = New-FilteredTempCopy -InputFolder $SourceFolder -ExcludePatterns $Excludes
    $TempItems = Get-ChildItem -LiteralPath $TempDir -Force
    if (-not $TempItems) {
        Write-Fail "Filtered temp copy is empty: $TempDir"
        Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
        exit 1
    }

    # Upload temp contents into the remote deployment root
    $Env:SSHPASS = $Password
    if (Get-Command sshpass -ErrorAction SilentlyContinue) {
        foreach ($item in $TempItems) {
            $scpArgs = @(
                "-P", $Port,
                "-o", "StrictHostKeyChecking=no",
                "-o", "PubkeyAuthentication=no",
                "-r",
                $item.FullName,
                "${Username}@${ServerHost}:${ServerPath}/"
            )
            & sshpass -p $Password scp @scpArgs
            if ($LASTEXITCODE -ne 0) { break }
        }
    } else {
        Write-Step "Tip: set SSH_ASKPASS or use PuTTY's pscp for password-based SCP without prompts"
        foreach ($item in $TempItems) {
            $scpArgs = @(
                "-P", $Port,
                "-o", "StrictHostKeyChecking=no",
                "-o", "PubkeyAuthentication=no",
                "-r",
                $item.FullName,
                "${Username}@${ServerHost}:${ServerPath}/"
            )
            & scp @scpArgs
            if ($LASTEXITCODE -ne 0) { break }
        }
    }

    # Cleanup temp
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue

} elseif ($UsePscp) {
    Write-Step "Using pscp (PuTTY)"

    # Try to get the host key fingerprint via ssh-keyscan for -hostkey flag.
    # If unavailable, pscp falls back to the PuTTY registry (populated by a
    # prior plink/pscp connection). If neither works, the transfer will fail
    # and the user is prompted to run plink once to cache the key.
    $hostKeyArgs = @()
    if (Get-Command ssh-keyscan -ErrorAction SilentlyContinue) {
        $tmpKeyFile = [System.IO.Path]::GetTempFileName()
        try {
            ssh-keyscan -t ed25519 -p $Port $ServerHost 2>$null | Out-File -Encoding ascii $tmpKeyFile
            if ((Get-Item $tmpKeyFile).Length -gt 0) {
                $fpLine = & ssh-keygen -lf $tmpKeyFile 2>$null
                $fp = ($fpLine -split '\s+') | Where-Object { $_ -match '^SHA256:' } | Select-Object -First 1
                if ($fp) {
                    Write-Step "Host key fingerprint: $fp"
                    $hostKeyArgs = @("-hostkey", $fp)
                }
            }
        } finally {
            Remove-Item $tmpKeyFile -ErrorAction SilentlyContinue
        }
    }

    if (-not $hostKeyArgs) {
        Write-Step "Using PuTTY registry for host key (run 'plink -P $Port ${Username}@${ServerHost} exit' once if this fails)"
    }

    $TempDir = New-FilteredTempCopy -InputFolder $SourceFolder -ExcludePatterns $Excludes
    $TempItems = Get-ChildItem -LiteralPath $TempDir -Force
    if (-not $TempItems) {
        Write-Fail "Filtered temp copy is empty: $TempDir"
        Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
        exit 1
    }

    foreach ($item in $TempItems) {
        $pscpArgs = @(
            "-r",
            "-P", $Port,
            "-pw", $Password,
            "-batch"
        ) + $hostKeyArgs + @(
            $item.FullName,
            "${Username}@${ServerHost}:${ServerPath}/"
        )
        & pscp @pscpArgs
        if ($LASTEXITCODE -ne 0) { break }
    }

    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
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
        # Fetch fingerprint for plink batch mode
        $plinkFp = $null
        if (Get-Command ssh-keyscan -ErrorAction SilentlyContinue) {
            $tmpKey = [System.IO.Path]::GetTempFileName()
            try {
                ssh-keyscan -t ed25519 -p $Port $ServerHost 2>$null | Out-File -Encoding ascii $tmpKey
                if ((Get-Item $tmpKey).Length -gt 0) {
                    $plinkFp = (& ssh-keygen -lf $tmpKey 2>$null) -split '\s+' | Where-Object { $_ -match '^SHA256:' } | Select-Object -First 1
                }
            } finally { Remove-Item $tmpKey -ErrorAction SilentlyContinue }
        }
        $plinkHostKey = if ($plinkFp) { @("-hostkey", $plinkFp) } else { @() }
        & plink -P $Port -pw $Password -batch @plinkHostKey "${Username}@${ServerHost}" $sshCmd
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
