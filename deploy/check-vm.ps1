#!/usr/bin/env pwsh
# Simple script to check VM status and package artifacts

$ProjectId = "project-95cdeea7-e2b2-4480-848"
$Instance = "instance-20260623-092750"
$Zone = "us-central1-f"

# Check if gcloud is available
$UseWsl = -not (Get-Command "gcloud" -ErrorAction SilentlyContinue)

if ($UseWsl) {
    Write-Host "Using WSL gcloud..."
    # Try to list instances via WSL
    wsl -d Ubuntu -u root -- bash -c "export PAGER=cat; gcloud --version"
    
    Write-Host "`nChecking VM status..."
    wsl -d Ubuntu -u root -- bash -c "export PAGER=cat; gcloud compute instances list --format=json | head -50" 2>&1
} else {
    Write-Host "Using native gcloud..."
    $env:PAGER = "cat"
    gcloud --version
    Write-Host "`nChecking VM status..."
    gcloud compute instances list --format=json 2>&1 | Select-Object -First 50
}
