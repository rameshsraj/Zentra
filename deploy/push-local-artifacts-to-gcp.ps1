#!/usr/bin/env pwsh
param(
    [string]$ProjectId = "project-95cdeea7-e2b2-4480-848",
    [string]$Instance = "instance-20260629-111005",
    [string]$Zone = "us-central1-a",
    [string]$VmUser = "",
    [string]$LocalArtifactDir = "C:\GitHub\Zentra\artifacts\vm-build\zentra-export",
    [string]$RemoteDir = "/store/Zentra"
)

$ErrorActionPreference = "Stop"

function Assert-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' not found in PATH."
    }
}

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

if (-not (Test-Path $LocalArtifactDir)) {
    throw "Local artifact directory not found: $LocalArtifactDir"
}

$buildDir = Join-Path $LocalArtifactDir "build"
$migDir = Join-Path $LocalArtifactDir "migrations"
$envExample = Join-Path $LocalArtifactDir ".env.example"

if (-not (Test-Path (Join-Path $buildDir "zentra-server"))) {
    throw "Missing binary: $buildDir\zentra-server"
}
if (-not (Test-Path (Join-Path $buildDir "frontend-dist"))) {
    throw "Missing frontend-dist directory under $buildDir"
}
if (-not (Get-ChildItem -Path $migDir -Filter *.sql -ErrorAction SilentlyContinue)) {
    throw "No SQL migrations found under $migDir"
}

$target = if ([string]::IsNullOrWhiteSpace($VmUser)) { $Instance } else { "$VmUser@$Instance" }

Write-Host "Setting gcloud project to $ProjectId" -ForegroundColor Cyan
Invoke-Gcloud @("config","set","project",$ProjectId)

Write-Host "Ensuring remote folders exist at $RemoteDir" -ForegroundColor Cyan
Invoke-Gcloud @("compute","ssh",$target,"--zone",$Zone,"--command","mkdir -p '$RemoteDir/build' '$RemoteDir/migrations'")

$scpBuild = $buildDir
$scpMig = $migDir
$scpEnv = $envExample
if ($UseWslGcloud) {
    $scpBuild = Convert-ToWslPath $buildDir
    $scpMig = Convert-ToWslPath $migDir
    $scpEnv = Convert-ToWslPath $envExample
}

Write-Host "Uploading build artifacts..." -ForegroundColor Cyan
Invoke-Gcloud @("compute","scp","--zone",$Zone,"--recurse",$scpBuild,"${target}:$RemoteDir/")

Write-Host "Uploading migrations..." -ForegroundColor Cyan
Invoke-Gcloud @("compute","scp","--zone",$Zone,"--recurse",$scpMig,"${target}:$RemoteDir/")

if (Test-Path $envExample) {
    Write-Host "Uploading .env.example..." -ForegroundColor Cyan
    Invoke-Gcloud @("compute","scp","--zone",$Zone,$scpEnv,"${target}:$RemoteDir/.env.example")
}

Write-Host "Remote verification:" -ForegroundColor Cyan
Invoke-Gcloud @("compute","ssh",$target,"--zone",$Zone,"--command","ls -la '$RemoteDir'; ls -la '$RemoteDir/build' | head -n 20; ls -la '$RemoteDir/migrations' | head -n 20")

Write-Host "Done. Uploaded local artifacts to ${target}:$RemoteDir" -ForegroundColor Green
