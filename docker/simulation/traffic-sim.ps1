# =============================================================================
# Zentra Datacenter Simulation - Traffic Generator (Windows PowerShell)
# Generates realistic monitoring traffic across all 12 simulated nodes.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File traffic-sim.ps1
#
# Press Ctrl+C to stop.
#
# Prerequisites:
#   - Simulation must be running (setup.ps1 completed)
#   - Optional: net-snmp (snmpwalk/snmpget) for SNMP traffic
#   - Optional: Oracle Instant Client (sqlplus) for DB traffic
# =============================================================================

$ErrorActionPreference = "Continue"

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
function Write-Traffic { Write-Host "[TRAFFIC] $args" -ForegroundColor Cyan }
function Write-Ok      { Write-Host "[OK]      $args" -ForegroundColor Green }
function Write-Warn    { Write-Host "[WARN]    $args" -ForegroundColor Yellow }
function Write-Err     { Write-Host "[ERROR]   $args" -ForegroundColor Red }
function Write-Dim     { param([string]$Name, [string]$Msg); Write-Host "[$Name] $Msg" -ForegroundColor DarkGray }

# ---------------------------------------------------------------------------
# Detect IPs from running containers
# ---------------------------------------------------------------------------
function Get-ContainerIp {
    param([string]$Name)
    try {
        $ip = docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $Name 2>&1
        if ($LASTEXITCODE -eq 0 -and $ip -match '^\d+\.\d+\.\d+\.\d+$') { return $ip }
    } catch {}
    return ""
}

Write-Host ""
Write-Host "======================================================" -ForegroundColor White
Write-Host "  Zentra Datacenter Simulation - Traffic Generator"     -ForegroundColor White
Write-Host "======================================================" -ForegroundColor White
Write-Host ""

Write-Traffic "Detecting simulation node IPs from running containers..."

$IpSrvLinux01  = Get-ContainerIp "srv-linux-01"
$IpSrvLinux02  = Get-ContainerIp "srv-linux-02"
$IpSrvWin01    = Get-ContainerIp "srv-win-01"
$IpSrvWin02    = Get-ContainerIp "srv-win-02"
$IpSwCore01    = Get-ContainerIp "sw-core-01"
$IpSwAccess01  = Get-ContainerIp "sw-access-01"
$IpFw01        = Get-ContainerIp "fw-01"
$IpStor01      = Get-ContainerIp "stor-01"
$IpMwNginx01   = Get-ContainerIp "mw-nginx-01"
$IpMwApache01  = Get-ContainerIp "mw-apache-01"
$IpMwTomcat01  = Get-ContainerIp "mw-tomcat-01"
$IpDbOracle01  = Get-ContainerIp "db-oracle-01"

# Verify at least some containers are running
$found = @($IpSrvLinux01, $IpSrvLinux02, $IpMwNginx01) | Where-Object { $_ -ne "" }
if ($found.Count -eq 0) {
    Write-Err "No simulation containers detected. Run setup.ps1 first."
    Read-Host "Press Enter to exit"
    exit 1
}

Write-Ok "Detected container IPs"

# ---------------------------------------------------------------------------
# Check available tools
# ---------------------------------------------------------------------------
$HasSnmpwalk = $null -ne (Get-Command "snmpwalk" -ErrorAction SilentlyContinue)
$HasSnmpget  = $null -ne (Get-Command "snmpget" -ErrorAction SilentlyContinue)
$HasSqlplus  = $null -ne (Get-Command "sqlplus" -ErrorAction SilentlyContinue)

Write-Traffic "Available tools:"
Write-Traffic "  snmpwalk: $HasSnmpwalk  snmpget: $HasSnmpget  curl: True (Invoke-WebRequest)"
Write-Traffic "  sqlplus:  $HasSqlplus"

# ---------------------------------------------------------------------------
# Traffic generation functions
# ---------------------------------------------------------------------------

function Poll-Snmp {
    param([string]$Name, [string]$Ip, [string]$Oid)
    if (-not $Ip -or -not $HasSnmpwalk) { return }
    Write-Dim -Name $Name -Msg "SNMP walk $Oid"
    try { snmpwalk -v2c -c public -t 2 -r 1 $Ip $Oid 2>&1 | Out-Null } catch {}
}

function Get-Snmp {
    param([string]$Name, [string]$Ip, [string]$Oid)
    if (-not $Ip -or -not $HasSnmpget) { return }
    Write-Dim -Name $Name -Msg "SNMP get $Oid"
    try { snmpget -v2c -c public -t 2 -r 1 $Ip $Oid 2>&1 | Out-Null } catch {}
}

function Poll-Http {
    param([string]$Name, [string]$Url)
    Write-Dim -Name $Name -Msg "HTTP GET $Url"
    try {
        Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 5 -ErrorAction SilentlyContinue | Out-Null
    } catch {}
}

function Poll-HttpAuth {
    param([string]$Name, [string]$Url, [string]$User, [string]$Pass)
    Write-Dim -Name $Name -Msg "HTTP GET $Url (auth)"
    try {
        $pair = "${User}:${Pass}"
        $bytes = [System.Text.Encoding]::ASCII.GetBytes($pair)
        $base64 = [System.Convert]::ToBase64String($bytes)
        $headers = @{ Authorization = "Basic $base64" }
        Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 5 -Headers $headers -ErrorAction SilentlyContinue | Out-Null
    } catch {}
}

function Poll-Oracle {
    param([string]$Name, [string]$Ip, [string]$Sql)
    if (-not $Ip) { return }
    if ($HasSqlplus) {
        Write-Dim -Name $Name -Msg "SQL: $Sql"
        try {
            $Sql | & sqlplus -S "zentra_mon/zentra123@${Ip}:1521/XE" 2>&1 | Out-Null
        } catch {}
    } else {
        # Fallback: TCP connectivity check to Oracle listener
        Write-Dim -Name $Name -Msg "TCP check :1521"
        try {
            $tcp = New-Object System.Net.Sockets.TcpClient
            $tcp.ConnectAsync($Ip, 1521).Wait(2000) | Out-Null
            $tcp.Close()
        } catch {}
    }
}

# ---------------------------------------------------------------------------
# Traffic simulation loop
# ---------------------------------------------------------------------------
$Cycle = 0
$Interval = 15

Write-Traffic "Starting traffic simulation (every ${Interval}s). Press Ctrl+C to stop."
Write-Host ""

while ($true) {
    $Cycle++
    $ts = Get-Date -Format "HH:mm:ss"
    Write-Host ""
    Write-Traffic "=== Cycle $Cycle [$ts] ==="

    # -------------------------------------------------------------------
    # Linux Servers - SNMP + SSH (SSH via docker exec as fallback)
    # -------------------------------------------------------------------
    Poll-Snmp -Name "srv-linux-01" -Ip $IpSrvLinux01 -Oid "system"
    Poll-Snmp -Name "srv-linux-01" -Ip $IpSrvLinux01 -Oid ".1.3.6.1.4.1.2021.11"
    Poll-Snmp -Name "srv-linux-01" -Ip $IpSrvLinux01 -Oid ".1.3.6.1.4.1.2021.4"
    Poll-Snmp -Name "srv-linux-01" -Ip $IpSrvLinux01 -Oid ".1.3.6.1.2.1.2.2"

    # SSH simulation via docker exec
    if ($IpSrvLinux01) {
        Write-Dim -Name "srv-linux-01" -Msg "SSH: uptime; df -h; free -m"
        try { docker exec srv-linux-01 bash -c "uptime; df -h; free -m" 2>&1 | Out-Null } catch {}
    }

    Poll-Snmp -Name "srv-linux-02" -Ip $IpSrvLinux02 -Oid "system"
    Poll-Snmp -Name "srv-linux-02" -Ip $IpSrvLinux02 -Oid ".1.3.6.1.4.1.2021.11"
    Poll-Snmp -Name "srv-linux-02" -Ip $IpSrvLinux02 -Oid ".1.3.6.1.4.1.2021.4"

    if ($IpSrvLinux02) {
        Write-Dim -Name "srv-linux-02" -Msg "SSH: uptime; df -h; free -m"
        try { docker exec srv-linux-02 bash -c "uptime; df -h; free -m" 2>&1 | Out-Null } catch {}
    }

    # -------------------------------------------------------------------
    # Windows Servers - SNMP
    # -------------------------------------------------------------------
    Poll-Snmp -Name "srv-win-01" -Ip $IpSrvWin01 -Oid "system"
    Poll-Snmp -Name "srv-win-01" -Ip $IpSrvWin01 -Oid ".1.3.6.1.2.1.25.2.3"
    Poll-Snmp -Name "srv-win-01" -Ip $IpSrvWin01 -Oid ".1.3.6.1.2.1.25.3.3"
    Get-Snmp  -Name "srv-win-01" -Ip $IpSrvWin01 -Oid ".1.3.6.1.2.1.25.1.1.0"

    Poll-Snmp -Name "srv-win-02" -Ip $IpSrvWin02 -Oid "system"
    Poll-Snmp -Name "srv-win-02" -Ip $IpSrvWin02 -Oid ".1.3.6.1.2.1.25.2.3"
    Poll-Snmp -Name "srv-win-02" -Ip $IpSrvWin02 -Oid ".1.3.6.1.2.1.25.3.3"
    Get-Snmp  -Name "srv-win-02" -Ip $IpSrvWin02 -Oid ".1.3.6.1.2.1.25.1.1.0"

    # -------------------------------------------------------------------
    # Network Switches - SNMP
    # -------------------------------------------------------------------
    Poll-Snmp -Name "sw-core-01" -Ip $IpSwCore01 -Oid "system"
    Poll-Snmp -Name "sw-core-01" -Ip $IpSwCore01 -Oid ".1.3.6.1.2.1.2.2"
    Poll-Snmp -Name "sw-core-01" -Ip $IpSwCore01 -Oid ".1.3.6.1.2.1.47.1.1.1"
    Get-Snmp  -Name "sw-core-01" -Ip $IpSwCore01 -Oid ".1.3.6.1.4.1.9.9.109.1.1.1.1.8.1"
    Get-Snmp  -Name "sw-core-01" -Ip $IpSwCore01 -Oid ".1.3.6.1.4.1.9.9.13.1.3.1.3.1"

    Poll-Snmp -Name "sw-access-01" -Ip $IpSwAccess01 -Oid "system"
    Poll-Snmp -Name "sw-access-01" -Ip $IpSwAccess01 -Oid ".1.3.6.1.2.1.2.2"
    Poll-Snmp -Name "sw-access-01" -Ip $IpSwAccess01 -Oid ".1.3.6.1.2.1.47.1.1.1"

    # -------------------------------------------------------------------
    # Firewall - SNMP
    # -------------------------------------------------------------------
    Poll-Snmp -Name "fw-01" -Ip $IpFw01 -Oid "system"
    Poll-Snmp -Name "fw-01" -Ip $IpFw01 -Oid ".1.3.6.1.2.1.2.2"
    Get-Snmp  -Name "fw-01" -Ip $IpFw01 -Oid ".1.3.6.1.4.1.25461.2.1.2.3.1.0"
    Get-Snmp  -Name "fw-01" -Ip $IpFw01 -Oid ".1.3.6.1.4.1.25461.2.1.2.3.3.0"
    Get-Snmp  -Name "fw-01" -Ip $IpFw01 -Oid ".1.3.6.1.4.1.25461.2.1.2.1.1.0"

    # -------------------------------------------------------------------
    # Storage Array - SNMP
    # -------------------------------------------------------------------
    Poll-Snmp -Name "stor-01" -Ip $IpStor01 -Oid "system"
    Poll-Snmp -Name "stor-01" -Ip $IpStor01 -Oid ".1.3.6.1.2.1.2.2"
    Poll-Snmp -Name "stor-01" -Ip $IpStor01 -Oid ".1.3.6.1.2.1.25.2.3"

    # -------------------------------------------------------------------
    # Middleware - HTTP
    # -------------------------------------------------------------------
    if ($IpMwNginx01) {
        Poll-Http -Name "mw-nginx-01" -Url "http://${IpMwNginx01}/"
        Poll-Http -Name "mw-nginx-01" -Url "http://${IpMwNginx01}:8080/nginx_status"
        Poll-Http -Name "mw-nginx-01" -Url "http://${IpMwNginx01}:8080/healthz"
    }

    if ($IpMwApache01) {
        Poll-Http -Name "mw-apache-01" -Url "http://${IpMwApache01}/"
        Poll-Http -Name "mw-apache-01" -Url "http://${IpMwApache01}:8080/server-status"
        Poll-Http -Name "mw-apache-01" -Url "http://${IpMwApache01}:8080/server-status?auto"
    }

    if ($IpMwTomcat01) {
        Poll-Http    -Name "mw-tomcat-01" -Url "http://${IpMwTomcat01}:8080/"
        Poll-HttpAuth -Name "mw-tomcat-01" -Url "http://${IpMwTomcat01}:8080/manager/status" -User "zentra" -Pass "zentra123"
        Poll-HttpAuth -Name "mw-tomcat-01" -Url "http://${IpMwTomcat01}:8080/manager/jmxproxy/?qry=java.lang:type=Memory" -User "zentra" -Pass "zentra123"
    }

    # -------------------------------------------------------------------
    # Oracle Database
    # -------------------------------------------------------------------
    Poll-Oracle -Name "db-oracle-01" -Ip $IpDbOracle01 -Sql "SELECT STATUS FROM V`$INSTANCE;"
    Poll-Oracle -Name "db-oracle-01" -Ip $IpDbOracle01 -Sql "SELECT TABLESPACE_NAME, STATUS FROM DBA_TABLESPACES;"
    Poll-Oracle -Name "db-oracle-01" -Ip $IpDbOracle01 -Sql "SELECT NAME, VALUE FROM V`$SYSSTAT WHERE NAME IN ('user commits','user rollbacks','db block gets','consistent gets','physical reads');"
    Poll-Oracle -Name "db-oracle-01" -Ip $IpDbOracle01 -Sql "SELECT COUNT(*) FROM V`$SESSION WHERE STATUS='ACTIVE';"

    # -------------------------------------------------------------------
    # Burst traffic (every 5th cycle, simulate heavier load)
    # -------------------------------------------------------------------
    if ($Cycle % 5 -eq 0) {
        Write-Traffic "--- Burst cycle: generating extra load ---"
        $jobs = @()
        for ($j = 1; $j -le 10; $j++) {
            if ($IpMwNginx01) {
                $jobs += Start-Job -ScriptBlock {
                    try { Invoke-WebRequest -Uri "http://$($args[0])/" -UseBasicParsing -TimeoutSec 3 -ErrorAction SilentlyContinue | Out-Null } catch {}
                } -ArgumentList $IpMwNginx01
            }
            if ($IpMwApache01) {
                $jobs += Start-Job -ScriptBlock {
                    try { Invoke-WebRequest -Uri "http://$($args[0])/" -UseBasicParsing -TimeoutSec 3 -ErrorAction SilentlyContinue | Out-Null } catch {}
                } -ArgumentList $IpMwApache01
            }
            if ($IpMwTomcat01) {
                $jobs += Start-Job -ScriptBlock {
                    try { Invoke-WebRequest -Uri "http://$($args[0]):8080/" -UseBasicParsing -TimeoutSec 3 -ErrorAction SilentlyContinue | Out-Null } catch {}
                } -ArgumentList $IpMwTomcat01
            }
        }
        if ($jobs.Count -gt 0) {
            $jobs | Wait-Job -Timeout 10 | Out-Null
            $jobs | Remove-Job -Force -ErrorAction SilentlyContinue
        }
        Poll-Snmp -Name "sw-core-01" -Ip $IpSwCore01 -Oid ".1.3.6.1.2.1.2.2.1.10"
        Poll-Snmp -Name "sw-core-01" -Ip $IpSwCore01 -Oid ".1.3.6.1.2.1.2.2.1.16"
    }

    Write-Ok "Cycle $Cycle complete"
    Write-Traffic "Sleeping ${Interval}s..."
    Start-Sleep -Seconds $Interval
}
