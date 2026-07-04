#!/usr/bin/env pwsh
param(
		[string]$ProjectId = "project-95cdeea7-e2b2-4480-848",
		[string]$Instance = "instance-20260629-111005",
		[string]$Zone = "us-central1-a",
		[string]$VmUser = "",
		[string]$RemoteRepoDir = "/store/Zentra",
		[string]$RemoteStageDir = "/tmp/zentra-export",
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

$UseWslGcloud = -not (Get-Command "gcloud" -ErrorAction SilentlyContinue)

function Convert-ToWslPath {
	param([string]$Path)
	$full = [System.IO.Path]::GetFullPath($Path)
	if ($full -match '^([A-Za-z]):\\(.*)$') {
		$drive = $matches[1].ToLower()
		$rest = $matches[2] -replace '\\','/'
		return "/mnt/$drive/$rest"
	}
	return ($full -replace '\\','/')
}

function Invoke-Gcloud {
	param([string[]]$CmdArgs)
	if ($UseWslGcloud) {
		$escaped = ($CmdArgs | ForEach-Object {
			"'" + ($_ -replace "'", "'\\''") + "'"
		}) -join " "
		& wsl -d Ubuntu -u root -- bash -lc "gcloud $escaped"
	} else {
		& gcloud @CmdArgs
	}
	if ($LASTEXITCODE -ne 0) {
		throw "gcloud command failed: gcloud $($CmdArgs -join ' ')"
	}
}

if ([string]::IsNullOrWhiteSpace($ProjectId) -or [string]::IsNullOrWhiteSpace($Instance) -or [string]::IsNullOrWhiteSpace($Zone)) {
		throw "ProjectId, Instance, and Zone are required."
}

$target = if ([string]::IsNullOrWhiteSpace($VmUser)) { $Instance } else { "$VmUser@$Instance" }

Write-Host "Setting gcloud project to $ProjectId" -ForegroundColor Cyan
Invoke-Gcloud @("config","set","project",$ProjectId)

New-Item -ItemType Directory -Path $LocalOutputRoot -Force | Out-Null
$localTar = Join-Path $LocalOutputRoot "zentra-export.tar.gz"
$localExtractDir = Join-Path $LocalOutputRoot "zentra-export"

if (Test-Path $localTar) { Remove-Item $localTar -Force }
if (Test-Path $localExtractDir) { Remove-Item $localExtractDir -Recurse -Force }

$remoteScript = @"
set -euo pipefail

REPO_DIR='$RemoteRepoDir'
STAGE_DIR='$RemoteStageDir'

mkdir -p "`$STAGE_DIR/build" "`$STAGE_DIR/migrations"
rm -rf "`$STAGE_DIR/build"/* "`$STAGE_DIR/migrations"/*

cd "`$REPO_DIR"

if command -v npm >/dev/null 2>&1; then
	cd frontend
	npm install
	node node_modules/vite/bin/vite.js build
	cd ..
else
	echo "npm not found on VM" >&2
	exit 1
fi

if command -v cargo >/dev/null 2>&1; then
	cargo build --release --bin zentra-server
else
	echo "cargo not found on VM" >&2
	exit 1
fi

cp target/release/zentra-server "`$STAGE_DIR/build/zentra-server"
chmod +x "`$STAGE_DIR/build/zentra-server"
mkdir -p "`$STAGE_DIR/build/frontend-dist"
cp -r frontend/dist/. "`$STAGE_DIR/build/frontend-dist/"
cp migrations/*.sql "`$STAGE_DIR/migrations/"

cat > "`$STAGE_DIR/.env.example" <<'ENVEOF'
DATABASE_URL=postgresql://postgres:zentra@127.0.0.1:55434/zentra
ZENTRA_API_PORT=5006
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=change_me_min_32_chars
ZENTRA_MASTER_KEY=change_me_64_hex_chars
RUST_LOG=info
ENVEOF

tar -czf /tmp/zentra-export.tar.gz -C /tmp zentra-export
"@

$remoteScriptBytes = [System.Text.Encoding]::UTF8.GetBytes($remoteScript)
$remoteScriptB64 = [Convert]::ToBase64String($remoteScriptBytes)
$remoteCommand = "echo $remoteScriptB64 | base64 -d > /tmp/zentra-build-export.sh && chmod +x /tmp/zentra-build-export.sh && bash /tmp/zentra-build-export.sh"

Write-Host "Building on VM and staging artifacts..." -ForegroundColor Cyan
Invoke-Gcloud @("compute","ssh",$target,"--zone",$Zone,"--command",$remoteCommand)

Write-Host "Pulling packaged artifacts to Windows..." -ForegroundColor Cyan
$scpTarget = $localTar
if ($UseWslGcloud) {
	$scpTarget = Convert-ToWslPath $localTar
}
Invoke-Gcloud @("compute","scp","--zone",$Zone,"${target}:/tmp/zentra-export.tar.gz",$scpTarget)

Write-Host "Extracting artifacts..." -ForegroundColor Cyan
tar -xzf $localTar -C $LocalOutputRoot

Write-Host "Artifact bundle ready at: $localExtractDir" -ForegroundColor Green
Write-Host "Contents:" -ForegroundColor Green
Get-ChildItem -Path $localExtractDir -Recurse | Select-Object FullName
