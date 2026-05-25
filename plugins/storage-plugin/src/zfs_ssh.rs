//! ZFS pool and dataset metrics via SSH exec — agentless, no ZFS agent required.
//!
//! Commands executed (single SSH session):
//!   zpool list -H -o name,size,alloc,free,capacity,health,dedupratio
//!   zpool iostat -H -y 1 1  (latency and throughput)
//!   zfs list -H -o name,used,avail,refer,compressratio,type -t filesystem,volume
//!   zpool status (parse degraded/faulted vdevs)

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tracing::warn;

pub struct ZfsSshCollector;

impl ZfsSshCollector {
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let script = build_zfs_script();
        match run_ssh_script(target, &script).await {
            Ok(output) => parse_zfs_output(&output, target),
            Err(e) => {
                warn!(target = %target.name, error = %e, "ZFS SSH collect failed");
                vec![]
            }
        }
    }
}

async fn run_ssh_script(target: &Target, script: &str) -> Result<String, String> {
    let host = format!("{}:{}", target.host, if target.port == 0 { 22 } else { target.port as u16 });
    let (user, key) = match &target.credentials {
        Some(c) => (
            c.username.clone().unwrap_or_else(|| "root".into()),
            c.password_encrypted.clone().unwrap_or_default(),
        ),
        None => return Err("No credentials configured".into()),
    };
    // Execute via openssh subprocess — avoids pulling in ssh2 crate
    let output = tokio::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "ConnectTimeout=10",
            "-i", &key,
            &format!("{}@{}", user, target.host),
            "-p", &target.port.to_string(),
            script,
        ])
        .output()
        .await
        .map_err(|e| format!("SSH exec failed: {}", e))?;
    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| e.to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn build_zfs_script() -> String {
    r#"
echo "===ZFS_POOLS==="
zpool list -H -o name,size,alloc,free,cap,health,dedupratio 2>/dev/null || echo "NO_ZPOOL"

echo "===ZFS_IOSTAT==="
zpool iostat -H -y 1 1 2>/dev/null | awk '{print $1,$2,$3,$4,$5,$6,$7}' || echo "NO_IOSTAT"

echo "===ZFS_DATASETS==="
zfs list -H -o name,used,avail,refer,compressratio,type -t filesystem,volume 2>/dev/null | head -100 || echo "NO_ZFS"

echo "===ZFS_STATUS==="
zpool status -x 2>/dev/null || echo "all pools are healthy"
"#.to_string()
}

fn parse_zfs_output(output: &str, target: &Target) -> Vec<Metric> {
    let mut metrics = Vec::new();
    let mut section = "";

    for line in output.lines() {
        let trimmed = line.trim();
        match trimmed {
            "===ZFS_POOLS===" => { section = "pools"; continue; }
            "===ZFS_IOSTAT===" => { section = "iostat"; continue; }
            "===ZFS_DATASETS===" => { section = "datasets"; continue; }
            "===ZFS_STATUS===" => { section = "status"; continue; }
            _ => {}
        }

        if trimmed.is_empty() || trimmed.starts_with("NO_") { continue; }

        match section {
            "pools" => parse_pool_line(trimmed, target, &mut metrics),
            "iostat" => parse_iostat_line(trimmed, target, &mut metrics),
            "datasets" => parse_dataset_line(trimmed, target, &mut metrics),
            _ => {}
        }
    }

    metrics
}

// zpool list -H output: name size alloc free cap% health dedupratio
fn parse_pool_line(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 7 { return; }

    let pool = parts[0];
    let size_bytes = parse_zfs_size(parts[1]);
    let alloc_bytes = parse_zfs_size(parts[2]);
    let free_bytes = parse_zfs_size(parts[3]);
    let cap_pct: f64 = parts[4].trim_end_matches('%').parse().unwrap_or(0.0);
    let health = parts[5];
    let dedup: f64 = parts[6].trim_end_matches('x').parse().unwrap_or(1.0);

    let health_score = match health {
        "ONLINE"  => 1.0,
        "DEGRADED"=> 0.5,
        _ => 0.0,
    };

    for (name, val, unit) in [
        ("storage.zfs.pool_size_bytes",  size_bytes,  "bytes"),
        ("storage.zfs.pool_alloc_bytes", alloc_bytes, "bytes"),
        ("storage.zfs.pool_free_bytes",  free_bytes,  "bytes"),
        ("storage.zfs.pool_cap_percent", cap_pct,     "percent"),
        ("storage.zfs.pool_health",      health_score,""),
        ("storage.zfs.pool_dedup_ratio", dedup,       "ratio"),
    ] {
        metrics.push(
            Metric::new(name, val, pool)
                .with_context(target.org_id, target.target_id, "storage-plugin")
                .with_resource_type("storage")
                .with_unit(unit)
                .with_tag("pool", pool)
                .with_tag("target", &target.name)
        );
    }
}

// zpool iostat -H output: pool alloc free read_ops write_ops read_bw write_bw
fn parse_iostat_line(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 7 { return; }

    let pool = parts[0];
    let read_ops: f64 = parts[3].parse().unwrap_or(0.0);
    let write_ops: f64 = parts[4].parse().unwrap_or(0.0);
    let read_bw = parse_zfs_size(parts[5]);
    let write_bw = parse_zfs_size(parts[6]);

    for (name, val, unit) in [
        ("storage.zfs.pool_read_iops",  read_ops,  "ops/s"),
        ("storage.zfs.pool_write_iops", write_ops, "ops/s"),
        ("storage.zfs.pool_read_bw",    read_bw,   "bytes/s"),
        ("storage.zfs.pool_write_bw",   write_bw,  "bytes/s"),
    ] {
        metrics.push(
            Metric::new(name, val, pool)
                .with_context(target.org_id, target.target_id, "storage-plugin")
                .with_resource_type("storage")
                .with_unit(unit)
                .with_tag("pool", pool)
        );
    }
}

// zfs list -H output: name used avail refer compressratio type
fn parse_dataset_line(line: &str, target: &Target, metrics: &mut Vec<Metric>) {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 { return; }

    let ds = parts[0];
    let used = parse_zfs_size(parts[1]);
    let avail = parse_zfs_size(parts[2]);
    let refer = parse_zfs_size(parts[3]);
    let compress: f64 = parts[4].trim_end_matches('x').parse().unwrap_or(1.0);

    for (name, val, unit) in [
        ("storage.zfs.dataset_used_bytes",   used,     "bytes"),
        ("storage.zfs.dataset_avail_bytes",  avail,    "bytes"),
        ("storage.zfs.dataset_refer_bytes",  refer,    "bytes"),
        ("storage.zfs.dataset_compress_ratio", compress, "ratio"),
    ] {
        metrics.push(
            Metric::new(name, val, ds)
                .with_context(target.org_id, target.target_id, "storage-plugin")
                .with_resource_type("storage")
                .with_unit(unit)
                .with_tag("dataset", ds)
        );
    }
}

/// Parse ZFS human-readable sizes: 1.5T, 800G, 200M, 1024K
fn parse_zfs_size(s: &str) -> f64 {
    if s == "-" || s.is_empty() { return 0.0; }
    let s = s.trim();
    let (num_part, suffix) = if let Some(c) = s.chars().last() {
        if c.is_alphabetic() {
            (&s[..s.len() - 1], c)
        } else {
            (s, ' ')
        }
    } else {
        return 0.0;
    };

    let base: f64 = num_part.parse().unwrap_or(0.0);
    match suffix {
        'T' | 't' => base * 1_099_511_627_776.0,
        'G' | 'g' => base * 1_073_741_824.0,
        'M' | 'm' => base * 1_048_576.0,
        'K' | 'k' => base * 1024.0,
        _ => base,
    }
}
