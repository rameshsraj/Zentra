<#
PowerShell script: check-containers.ps1
Purpose: Report status and basic network/port info for all Docker containers
Usage examples:
  # List status for all containers
  .\check-containers.ps1

  # Tail last 50 lines of logs for non-running containers
  .\check-containers.ps1 -TailLogs 50
#>
param(
    [int]
    $TailLogs = 0
)

function Ensure-DockerAvailable {
    docker version > $null 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Docker is not available or the daemon is not running."
        exit 2
    }
}

function Get-ContainerStatusObject {
    param([string]$Name)

    $raw = docker inspect $Name 2>$null
    if (-not $raw) { return $null }
    $info = $raw | ConvertFrom-Json | Select-Object -First 1

    $networks = @()
    if ($info.NetworkSettings -and $info.NetworkSettings.Networks -and ($info.NetworkSettings.Networks -isnot [System.Boolean])) {
        $nets = $info.NetworkSettings.Networks
        if ($nets -is [System.Collections.IDictionary]) {
            foreach ($k in $nets.Keys) {
                $v = $nets[$k]
                $networks += ("{0}={1}" -f $k, ($v.IPAddress -or "N/A"))
            }
        } else {
            foreach ($prop in $nets.PSObject.Properties) {
                $k = $prop.Name
                $v = $prop.Value
                $networks += ("{0}={1}" -f $k, ($v.IPAddress -or "N/A"))
            }
        }
    }

    $ports = @()
    $hostBindings = @()
    if ($info.NetworkSettings -and $info.NetworkSettings.Ports -and ($info.NetworkSettings.Ports -isnot [System.Boolean])) {
        $portsObj = $info.NetworkSettings.Ports
        foreach ($prop in $portsObj.PSObject.Properties) {
            $pkey = $prop.Name
            $binding = $prop.Value
            if ($null -ne $binding -and $binding.Count -gt 0) {
                foreach ($b in $binding) {
                    $hostIp = $b.HostIp -or ''
                    $hostPort = $b.HostPort -or ''
                    $ports += ("{0} -> {1}:{2}" -f $pkey, $hostIp, $hostPort)
                    if ($hostPort -ne '') { $hostBindings += ("{0}:{1}" -f ($hostIp -or '0.0.0.0'), $hostPort) }
                }
            } else {
                $ports += ("{0} -> (unbound)" -f $pkey)
            }
        }
    }

    $health = $null
    $failing = 0
    if ($info.State -and $info.State.Health) {
        if ($info.State.Health -is [System.Management.Automation.PSCustomObject]) {
            $health = $info.State.Health.Status
            try { $failing = $info.State.Health.FailingStreak } catch { $failing = 0 }
        } else {
            # Health might be a boolean/string in some inspect outputs; stringify it
            $health = [string]$info.State.Health
            $failing = 0
        }
    }

    [PSCustomObject]@{
        Name         = $info.Name.TrimStart('/')
        Image        = $info.Config.Image
        Status       = $info.State.Status
        Health       = [string](($health -ne $null) ? $health : '')
        FailingStreak= $failing
        Restarts     = $info.RestartCount
        IPs          = [string]($networks -join ', ')
        PrimaryIP    = [string]((($networks | Select-Object -First 1) -split '=' | Select-Object -Last 1) -replace '^(?:,\s*)*','')
        HostPorts    = [string](($hostBindings | Select-Object -Unique) -join ', ')
        Ports        = [string]($ports -join ', ')
        Created      = $info.Created
    }
}

# --- main ---
Ensure-DockerAvailable

$names = docker ps -a --format '{{.Names}}' 2>$null | Where-Object { $_ -ne '' }
if (-not $names) { Write-Host "No Docker containers found."; exit 0 }

$rows = @()
foreach ($n in $names) {
    $obj = Get-ContainerStatusObject -Name $n
    if ($obj) { $rows += $obj }
}

if ($rows.Count -eq 0) { Write-Host "No container data could be retrieved."; exit 1 }

# Print with colors and health info
$rows = $rows | Sort-Object -Property Name

# Header
Write-Host ("{0,-20} {1,-25} {2,-12} {3,-12} {4,-8} {5,-15} {6,-20} {7}" -f 'NAME','IMAGE','STATUS','HEALTH','RESTARTS','IP','HOSTPORTS','PORTS') -ForegroundColor Cyan
foreach ($row in $rows) {
    $color = 'White'
    if ($row.Status -ne 'running') { $color = 'Red' }
    elseif ($row.Health -eq 'healthy') { $color = 'Green' }
    elseif ($row.Health -eq 'unhealthy') { $color = 'Red' }
    else { $color = 'Yellow' }

    $healthText = $row.Health
    if ($row.FailingStreak -and $row.FailingStreak -gt 0) { $healthText = "${healthText} (fail:${($row.FailingStreak)})" }

    $ipOut = $row.PrimaryIP -or $row.IPs -or '-'
    $hostPortsOut = $row.HostPorts -or '-'
    $line = "{0,-20} {1,-25} {2,-12} {3,-12} {4,-8} {5,-15} {6,-20} {7}" -f $row.Name, $row.Image, $row.Status, ($healthText -or '-'), $row.Restarts, $ipOut, $hostPortsOut, ($row.Ports -or '-')
    Write-Host $line -ForegroundColor $color
}

# Summary
$total = $rows.Count
$running = ($rows | Where-Object { $_.Status -eq 'running' }).Count
$exited = ($rows | Where-Object { $_.Status -ne 'running' }).Count

Write-Host "`nSummary: Total=$total, Running=$running, NotRunning=$exited`n"

if ($TailLogs -gt 0) {
    Write-Host "Tailing last $TailLogs lines from non-running containers...`
"
    foreach ($r in ($rows | Where-Object { $_.Status -ne 'running' })) {
        Write-Host "--- Logs: $($r.Name) ---`n"
        docker logs --tail $TailLogs $($r.Name) 2>&1 | ForEach-Object { Write-Host $_ }
        Write-Host "`n"
    }
}
