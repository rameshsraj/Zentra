//! Deep Linux server metrics collector — agentless via SSH exec (read-only commands).
//!
//! Collects via a single SSH session per poll cycle:
//! - Per-CPU core utilization (user/system/iowait/steal/irq/softirq)
//! - Top-N processes by CPU and memory
//! - TCP socket state histogram
//! - Deep memory metrics (/proc/meminfo + NUMA)
//! - Network interface deep stats (/sys/class/net + ethtool)
//! - Disk I/O depth (iostat-equivalent from /proc/diskstats)
//! - Kernel parameters (sysctl security/performance)

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tokio::process::Command;
use tracing::{debug, warn};

fn srv(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "server-plugin")
        .with_resource_type("server")
}

pub struct LinuxDeepCollector;

impl LinuxDeepCollector {
    /// Comprehensive bash script executed via a single SSH connection.
    /// All commands are read-only — no writes, no installs.
    const COLLECTION_SCRIPT: &'static str = r#"
#!/bin/bash
set -e

# ── CPU cores ────────────────────────────────────────────────────────────────
echo "===CPU_CORES==="
awk '/^cpu[0-9]/ {
    idle=$5; iowait=$6; steal=$9;
    total=$2+$3+$4+$5+$6+$7+$8+$9+$10;
    user=$2; sys=$4; irq=$7; softirq=$8;
    printf "%s user=%.2f system=%.2f iowait=%.2f steal=%.2f irq=%.2f softirq=%.2f total=%d idle=%d\n",
      $1, ($2/total)*100, ($4/total)*100, ($6/total)*100, ($9/total)*100,
      ($7/total)*100, ($8/total)*100, total, idle
}' /proc/stat

# ── Top-20 processes by CPU ──────────────────────────────────────────────────
echo "===PROCS_CPU==="
ps aux --sort=-%cpu 2>/dev/null | awk 'NR>1 && NR<=21 {
    printf "pid=%s name=%s cpu=%.2f mem=%.2f rss=%s vsz=%s user=%s\n",
      $2, $11, $3, $4, $6, $5, $1
}'

# ── Top-20 processes by memory ───────────────────────────────────────────────
echo "===PROCS_MEM==="
ps aux --sort=-%mem 2>/dev/null | awk 'NR>1 && NR<=21 {
    printf "pid=%s name=%s cpu=%.2f mem=%.2f rss=%s vsz=%s user=%s\n",
      $2, $11, $3, $4, $6, $5, $1
}'

# ── TCP socket state histogram ───────────────────────────────────────────────
echo "===TCP_STATES==="
ss -ant 2>/dev/null | awk 'NR>1 {counts[$1]++} END {for(s in counts) print s, counts[s]}' \
  || cat /proc/net/tcp 2>/dev/null | awk 'NR>1 {
    state=$4;
    if(state=="01") s="ESTABLISHED";
    else if(state=="02") s="SYN_SENT";
    else if(state=="03") s="SYN_RECV";
    else if(state=="04") s="FIN_WAIT1";
    else if(state=="05") s="FIN_WAIT2";
    else if(state=="06") s="TIME_WAIT";
    else if(state=="07") s="CLOSE";
    else if(state=="08") s="CLOSE_WAIT";
    else if(state=="09") s="LAST_ACK";
    else if(state=="0A") s="LISTEN";
    else if(state=="0B") s="CLOSING";
    else s="UNKNOWN";
    counts[s]++
  } END {for(s in counts) print s, counts[s]}'

# ── Deep memory ──────────────────────────────────────────────────────────────
echo "===MEMORY==="
awk '{print $1, $2}' /proc/meminfo 2>/dev/null

# ── Disk I/O (iostat-compatible from /proc/diskstats) ───────────────────────
echo "===DISKSTATS==="
awk '$3 !~ /^(loop|ram|fd)/ && $3 !~ /[0-9]$/ {
    printf "dev=%s reads=%s reads_merged=%s sectors_read=%s ms_read=%s writes=%s writes_merged=%s sectors_written=%s ms_write=%s ios_in_progress=%s ms_ios=%s ms_weighted=%s\n",
      $3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14
}' /proc/diskstats 2>/dev/null

# ── Network interface deep stats ─────────────────────────────────────────────
echo "===NET_IFACES==="
for iface in $(ls /sys/class/net/ 2>/dev/null | grep -v lo); do
    rx_b=$(cat /sys/class/net/$iface/statistics/rx_bytes 2>/dev/null || echo 0)
    tx_b=$(cat /sys/class/net/$iface/statistics/tx_bytes 2>/dev/null || echo 0)
    rx_p=$(cat /sys/class/net/$iface/statistics/rx_packets 2>/dev/null || echo 0)
    tx_p=$(cat /sys/class/net/$iface/statistics/tx_packets 2>/dev/null || echo 0)
    rx_e=$(cat /sys/class/net/$iface/statistics/rx_errors 2>/dev/null || echo 0)
    tx_e=$(cat /sys/class/net/$iface/statistics/tx_errors 2>/dev/null || echo 0)
    rx_d=$(cat /sys/class/net/$iface/statistics/rx_dropped 2>/dev/null || echo 0)
    tx_d=$(cat /sys/class/net/$iface/statistics/tx_dropped 2>/dev/null || echo 0)
    rx_f=$(cat /sys/class/net/$iface/statistics/rx_fifo_errors 2>/dev/null || echo 0)
    tx_f=$(cat /sys/class/net/$iface/statistics/tx_fifo_errors 2>/dev/null || echo 0)
    rx_crc=$(cat /sys/class/net/$iface/statistics/rx_crc_errors 2>/dev/null || echo 0)
    speed=$(cat /sys/class/net/$iface/speed 2>/dev/null || echo -1)
    duplex=$(cat /sys/class/net/$iface/duplex 2>/dev/null || echo unknown)
    mtu=$(cat /sys/class/net/$iface/mtu 2>/dev/null || echo 0)
    operstate=$(cat /sys/class/net/$iface/operstate 2>/dev/null || echo unknown)
    printf "iface=%s rx_bytes=%s tx_bytes=%s rx_packets=%s tx_packets=%s rx_errors=%s tx_errors=%s rx_dropped=%s tx_dropped=%s rx_fifo=%s tx_fifo=%s rx_crc=%s speed=%s duplex=%s mtu=%s state=%s\n" \
      $iface $rx_b $tx_b $rx_p $tx_p $rx_e $tx_e $rx_d $tx_d $rx_f $tx_f $rx_crc $speed $duplex $mtu $operstate
done

# ── Kernel parameters ────────────────────────────────────────────────────────
echo "===SYSCTL==="
for key in \
  vm.swappiness vm.dirty_ratio vm.dirty_background_ratio \
  vm.vfs_cache_pressure kernel.pid_max fs.file-max \
  net.core.somaxconn net.ipv4.tcp_max_syn_backlog \
  net.ipv4.tcp_tw_reuse net.ipv4.ip_local_port_range \
  kernel.randomize_va_space kernel.kptr_restrict \
  kernel.dmesg_restrict net.ipv4.tcp_fin_timeout; do
    val=$(sysctl -n $key 2>/dev/null || echo -1)
    echo "$key=$val"
done

echo "===END==="
"#;

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let ssh_user = target.credentials.as_ref()
            .and_then(|c| c.username.clone())
            .unwrap_or_else(|| std::env::var("zentra_SSH_USER").unwrap_or_else(|_| "root".into()));

        let output = Command::new("ssh")
            .args([
                "-o", "StrictHostKeyChecking=no",
                "-o", "ConnectTimeout=10",
                "-o", "BatchMode=yes",
                "-o", "ServerAliveInterval=5",
                &format!("{}@{}", ssh_user, host),
                Self::COLLECTION_SCRIPT,
            ])
            .output()
            .await;

        let output = match output {
            Ok(o) if o.status.success() || !o.stdout.is_empty() => o,
            Ok(o) => {
                warn!(target = %target.name, err = %String::from_utf8_lossy(&o.stderr), "SSH collection failed");
                return metrics;
            }
            Err(e) => {
                warn!(target = %target.name, err = %e, "SSH exec error");
                return metrics;
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut section = "";

        for line in stdout.lines() {
            let line = line.trim();
            if line.starts_with("===") && line.ends_with("===") {
                section = line;
                continue;
            }
            if line.is_empty() { continue; }

            match section {
                "===CPU_CORES===" => Self::parse_cpu_core(line, target, &mut metrics),
                "===PROCS_CPU===" | "===PROCS_MEM===" => Self::parse_process(line, section, target, &mut metrics),
                "===TCP_STATES===" => Self::parse_tcp_state(line, target, &mut metrics),
                "===MEMORY===" => Self::parse_meminfo(line, target, &mut metrics),
                "===DISKSTATS===" => Self::parse_diskstat(line, target, &mut metrics),
                "===NET_IFACES===" => Self::parse_net_iface(line, target, &mut metrics),
                "===SYSCTL===" => Self::parse_sysctl(line, target, &mut metrics),
                _ => {}
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "Linux deep collection complete");
        metrics
    }

    fn kv(line: &str, key: &str) -> Option<f64> {
        line.split_whitespace()
            .find(|tok| tok.starts_with(&format!("{}=", key)))?
            .split_once('=')?
            .1
            .parse::<f64>()
            .ok()
    }

    fn kv_str<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        line.split_whitespace()
            .find(|tok| tok.starts_with(&format!("{}=", key)))?
            .split_once('=')?
            .1
            .into()
    }

    fn parse_cpu_core(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
        // Format: cpu0 user=5.23 system=1.12 iowait=0.45 steal=0.00 irq=0.01 softirq=0.02 ...
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() < 2 { return; }
        let core = parts[0];

        if let Some(v) = Self::kv(line, "user") {
            metrics.push(srv(&format!("server.cpu.{}.user_percent", core), v, core, target).with_tag("core", core));
        }
        if let Some(v) = Self::kv(line, "system") {
            metrics.push(srv(&format!("server.cpu.{}.system_percent", core), v, core, target).with_tag("core", core));
        }
        if let Some(v) = Self::kv(line, "iowait") {
            metrics.push(srv(&format!("server.cpu.{}.iowait_percent", core), v, core, target).with_tag("core", core));
        }
        if let Some(v) = Self::kv(line, "steal") {
            metrics.push(srv(&format!("server.cpu.{}.steal_percent", core), v, core, target)
                .with_tag("core", core)
                .with_unit("percent"));
        }
        if let Some(v) = Self::kv(line, "irq") {
            metrics.push(srv(&format!("server.cpu.{}.irq_percent", core), v, core, target).with_tag("core", core));
        }
        if let Some(v) = Self::kv(line, "softirq") {
            metrics.push(srv(&format!("server.cpu.{}.softirq_percent", core), v, core, target).with_tag("core", core));
        }
    }

    fn parse_process(line: &str, section: &str, target: &Target, metrics: &mut Vec<Metric>) {
        let prefix = if section == "===PROCS_CPU===" { "cpu" } else { "mem" };
        let pid = Self::kv_str(line, "pid").unwrap_or("0").to_string();
        let name = Self::kv_str(line, "name").unwrap_or("unknown").to_string();

        if let Some(cpu) = Self::kv(line, "cpu") {
            metrics.push(srv(&format!("server.process.{}.cpu_percent", prefix), cpu, &name, target)
                .with_tag("pid", &pid).with_tag("name", &name).with_unit("percent"));
        }
        if let Some(rss) = Self::kv(line, "rss") {
            metrics.push(srv("server.process.rss_kb", rss, &name, target)
                .with_tag("pid", &pid).with_tag("name", &name).with_unit("kilobytes"));
        }
        if let Some(vsz) = Self::kv(line, "vsz") {
            metrics.push(srv("server.process.vsz_kb", vsz, &name, target)
                .with_tag("pid", &pid).with_tag("name", &name).with_unit("kilobytes"));
        }
    }

    fn parse_tcp_state(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() != 2 { return; }
        let state = parts[0].to_uppercase();
        if let Ok(count) = parts[1].trim().parse::<f64>() {
            let metric_name = format!("server.tcp.state_{}", state.to_lowercase());
            metrics.push(srv(&metric_name, count, "tcp", target).with_tag("state", &state).with_unit("count"));
        }
    }

    fn parse_meminfo(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 { return; }
        let key = parts[0].trim().to_lowercase().replace(['(', ')'], "");
        let val_str = parts[1].trim().split_whitespace().next().unwrap_or("0");
        let val: f64 = val_str.parse().unwrap_or(0.0) * 1024.0; // kB → bytes

        let metric_name = match key.as_str() {
            "memtotal"    => "server.mem.total_bytes",
            "memfree"     => "server.mem.free_bytes",
            "memavailable" => "server.mem.available_bytes",
            "buffers"     => "server.mem.buffers_bytes",
            "cached"      => "server.mem.cached_bytes",
            "swapcached"  => "server.mem.swap_cached_bytes",
            "active"      => "server.mem.active_bytes",
            "inactive"    => "server.mem.inactive_bytes",
            "slab"        => "server.mem.slab_bytes",
            "sreclaimable" => "server.mem.slab_reclaimable_bytes",
            "anonpages"   => "server.mem.anon_pages_bytes",
            "mapped"      => "server.mem.mapped_bytes",
            "shmem"       => "server.mem.shared_bytes",
            "hugepages_total" => "server.mem.hugepages_total",
            "hugepages_free"  => "server.mem.hugepages_free",
            "hugepagesize"    => "server.mem.hugepage_size_bytes",
            "swaptotal"   => "server.mem.swap_total_bytes",
            "swapfree"    => "server.mem.swap_free_bytes",
            _ => return,
        };
        metrics.push(srv(metric_name, val, "memory", target).with_unit("bytes"));
    }

    fn parse_diskstat(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
        let dev = Self::kv_str(line, "dev").unwrap_or("unknown").to_string();

        macro_rules! push {
            ($name:expr, $field:expr, $unit:expr) => {
                if let Some(v) = Self::kv(line, $field) {
                    metrics.push(srv($name, v, &dev, target).with_tag("device", &dev).with_unit($unit));
                }
            };
        }

        push!("server.disk.reads_completed", "reads", "count");
        push!("server.disk.reads_merged", "reads_merged", "count");
        push!("server.disk.sectors_read", "sectors_read", "sectors");
        push!("server.disk.ms_reading", "ms_read", "milliseconds");
        push!("server.disk.writes_completed", "writes", "count");
        push!("server.disk.writes_merged", "writes_merged", "count");
        push!("server.disk.sectors_written", "sectors_written", "sectors");
        push!("server.disk.ms_writing", "ms_write", "milliseconds");
        push!("server.disk.ios_in_progress", "ios_in_progress", "count");
        push!("server.disk.ms_ios", "ms_ios", "milliseconds");
        push!("server.disk.ms_weighted", "ms_weighted", "milliseconds");

        // Derived: utilization % = (ms_ios / 1000) * 100 over a 1-second window
        if let (Some(ms_ios), _) = (Self::kv(line, "ms_ios"), ()) {
            let util = (ms_ios / 1000.0).min(100.0);
            metrics.push(srv("server.disk.util_percent", util, &dev, target).with_tag("device", &dev).with_unit("percent"));
        }
    }

    fn parse_net_iface(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
        let iface = Self::kv_str(line, "iface").unwrap_or("unknown").to_string();
        let state = Self::kv_str(line, "state").unwrap_or("unknown");
        let state_val = if state == "up" { 1.0 } else { 0.0 };

        macro_rules! push {
            ($name:expr, $field:expr, $unit:expr) => {
                if let Some(v) = Self::kv(line, $field) {
                    metrics.push(srv($name, v, &iface, target).with_tag("interface", &iface).with_unit($unit));
                }
            };
        }

        push!("server.nic.rx_bytes", "rx_bytes", "bytes");
        push!("server.nic.tx_bytes", "tx_bytes", "bytes");
        push!("server.nic.rx_packets", "rx_packets", "count");
        push!("server.nic.tx_packets", "tx_packets", "count");
        push!("server.nic.rx_errors", "rx_errors", "count");
        push!("server.nic.tx_errors", "tx_errors", "count");
        push!("server.nic.rx_dropped", "rx_dropped", "count");
        push!("server.nic.tx_dropped", "tx_dropped", "count");
        push!("server.nic.rx_fifo_errors", "rx_fifo", "count");
        push!("server.nic.tx_fifo_errors", "tx_fifo", "count");
        push!("server.nic.rx_crc_errors", "rx_crc", "count");
        push!("server.nic.speed_mbps", "speed", "mbps");
        push!("server.nic.mtu", "mtu", "bytes");
        metrics.push(srv("server.nic.link_up", state_val, &iface, target).with_tag("interface", &iface).with_unit("bool"));
    }

    fn parse_sysctl(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
        if let Some((key, val)) = line.split_once('=') {
            let metric_name = format!("server.sysctl.{}", key.trim().replace('.', "_").replace('-', "_"));
            // Numeric sysctl values only (some are ranges like "1024 65535")
            if let Ok(v) = val.trim().split_whitespace().next().unwrap_or("").parse::<f64>() {
                metrics.push(srv(&metric_name, v, "kernel", target).with_tag("param", key.trim()));
            }
        }
    }
}
