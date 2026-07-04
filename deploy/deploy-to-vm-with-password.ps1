param(
    [string]$VmHost = "136.114.64.243",
    [string]$VmUser = "zentra",
    [string]$VmPassword = "zentra",
    [string]$HostKey = "ssh-ed25519 255 SHA256:6qm14d9MrNxvCZEL9DlJvifO8YICQ3a1dZ67eZk9XB8",
    [string]$LocalArtifactDir = "C:\GitHub\Zentra\artifacts\vm-build\zentra-export",
    [string]$RemoteDir = "/store/Zentra",
    [string]$RemotePgDataDir = "/store/pgdata",
    [string]$SetupSource = "C:\GitHub\Zentra\deploy\setup_server.sh"
)

$ErrorActionPreference = "Stop"

$plink = "C:\GitHub\Zentra\deploy\plink.exe"
$pscp = "C:\GitHub\Zentra\deploy\pscp.exe"

if (-not (Test-Path $plink)) { throw "plink not found: $plink" }
if (-not (Test-Path $pscp)) { throw "pscp not found: $pscp" }
if (-not (Test-Path $LocalArtifactDir)) { throw "Local artifact dir not found: $LocalArtifactDir" }
if (-not (Test-Path $SetupSource)) { throw "setup source not found: $SetupSource" }

$buildDir = Join-Path $LocalArtifactDir "build"
$migDir = Join-Path $LocalArtifactDir "migrations"
$envExample = Join-Path $LocalArtifactDir ".env.example"

if (-not (Test-Path (Join-Path $buildDir "zentra-server"))) { throw "Missing binary at $buildDir\zentra-server" }
if (-not (Test-Path (Join-Path $buildDir "frontend-dist"))) { throw "Missing frontend-dist at $buildDir\frontend-dist" }
if (-not (Get-ChildItem -Path $migDir -Filter *.sql -ErrorAction SilentlyContinue)) { throw "No SQL migrations found in $migDir" }

function Invoke-Remote {
    param([string]$Command)

    & $plink -batch -hostkey $HostKey -ssh -pw $VmPassword "$VmUser@$VmHost" $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Remote command failed: $Command"
    }
}

function Copy-ToRemote {
    param(
        [string]$LocalPath,
        [string]$RemotePath,
        [switch]$Recurse
    )

    $args = @("-batch", "-hostkey", $HostKey, "-pw", $VmPassword)
    if ($Recurse) { $args += "-r" }
    $args += @($LocalPath, "${VmUser}@${VmHost}:$RemotePath")

    & $pscp @args
    if ($LASTEXITCODE -ne 0) {
        throw "SCP failed: $LocalPath -> $RemotePath"
    }
}

Write-Host "1) Preparing remote directories" -ForegroundColor Cyan
Invoke-Remote "echo '$VmPassword' | sudo -S mkdir -p '$RemoteDir' '$RemoteDir/build' '$RemoteDir/migrations' '$RemotePgDataDir'"

Write-Host "2) Uploading build artifacts" -ForegroundColor Cyan
Copy-ToRemote -LocalPath $buildDir -RemotePath "$RemoteDir/" -Recurse

Write-Host "3) Uploading migrations" -ForegroundColor Cyan
Copy-ToRemote -LocalPath $migDir -RemotePath "$RemoteDir/" -Recurse

if (Test-Path $envExample) {
    Write-Host "4) Uploading .env.example" -ForegroundColor Cyan
    Copy-ToRemote -LocalPath $envExample -RemotePath "$RemoteDir/.env.example"
}

Write-Host "5) Uploading setup.sh" -ForegroundColor Cyan
Copy-ToRemote -LocalPath $SetupSource -RemotePath "/tmp/setup.sh"
Invoke-Remote "echo '$VmPassword' | sudo -S mv /tmp/setup.sh '$RemoteDir/setup.sh' && echo '$VmPassword' | sudo -S chmod +x '$RemoteDir/setup.sh'"

Write-Host "6) Writing runtime .env with requested credentials" -ForegroundColor Cyan
$remoteEnvCmd = @"
cat << 'EOF' > /tmp/zentra.env
DATABASE_URL=postgresql://postgres:zantaz@127.0.0.1:5432/zentra
ZENTRA_API_PORT=5006
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=change_me_min_32_chars
ZENTRA_MASTER_KEY=change_me_64_hex_chars
RUST_LOG=info
ZENTRA_APP_USER=zentra
ZENTRA_APP_PASSWORD=zentra
EOF
"@
Invoke-Remote $remoteEnvCmd
Invoke-Remote "echo '$VmPassword' | sudo -S mv /tmp/zentra.env '$RemoteDir/.env' && echo '$VmPassword' | sudo -S chown root:root '$RemoteDir/.env' && echo '$VmPassword' | sudo -S chmod 640 '$RemoteDir/.env'"

Write-Host "7) Running setup.sh" -ForegroundColor Cyan
Invoke-Remote "echo '$VmPassword' | sudo -S env DATABASE_URL='postgresql://postgres:zantaz@127.0.0.1:5432/zentra' bash '$RemoteDir/setup.sh'"

Write-Host "8) Verifying deployment" -ForegroundColor Cyan
Invoke-Remote "echo '$VmPassword' | sudo -S ls -la '$RemoteDir' && echo '$VmPassword' | sudo -S ls -la '$RemoteDir/build' | head -n 30 && echo '$VmPassword' | sudo -S ls -la '$RemoteDir/migrations' | head -n 30 && echo '$VmPassword' | sudo -S ls -la '$RemotePgDataDir'"

Write-Host "Deployment complete." -ForegroundColor Green
Write-Host "Remote code path: $RemoteDir" -ForegroundColor Green
Write-Host "Remote pgdata path: $RemotePgDataDir" -ForegroundColor Green
