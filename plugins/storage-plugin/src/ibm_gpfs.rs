use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tokio::process::Command;
use tracing::{debug, warn};

fn st_metric(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage")
}

pub struct IbmGpfsCollector;

impl IbmGpfsCollector {
    /// Collect IBM GPFS / Spectrum Scale metrics via SSH exec.
    /// Runs mmdf, mmlsfs, mmgetstate, mmstat on the GPFS management node.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let user = target.credentials.as_ref()
            .and_then(|c| c.username.clone())
            .unwrap_or_else(|| "root".into());
        let host = &target.host;
        let ssh_host = format!("{}@{}", user, host);

        // mmgetstate — cluster node state
        let state_out = run_ssh(&ssh_host, "/usr/lpp/mmfs/bin/mmgetstate -a").await;
        if let Some(out) = state_out {
            let active = out.lines().filter(|l| l.contains("active")).count();
            let total = out.lines().filter(|l| l.len() > 10 && !l.starts_with('-') && !l.starts_with("Node")).count();
            metrics.push(st_metric("storage.gpfs.nodes_active", active as f64, "cluster", target));
            metrics.push(st_metric("storage.gpfs.nodes_total", total as f64, "cluster", target));
        }

        // mmlsfs — list filesystems (get filesystem names)
        let fs_list = run_ssh(&ssh_host, "/usr/lpp/mmfs/bin/mmlsfs all -T").await;
        let filesystems: Vec<String> = if let Some(out) = &fs_list {
            out.lines()
                .filter(|l| l.starts_with('/'))
                .map(|l| l.split_whitespace().next().unwrap_or("").to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            vec![]
        };

        // mmdf — disk free per filesystem
        for fs in &filesystems {
            let cmd = format!("/usr/lpp/mmfs/bin/mmdf {} -Y", fs);
            if let Some(out) = run_ssh(&ssh_host, &cmd).await {
                parse_mmdf(&out, fs, target, &mut metrics);
            }
        }

        // mmlsfs detailed (block size, replication, quotas)
        for fs in &filesystems {
            let cmd = format!("/usr/lpp/mmfs/bin/mmlsfs {} -l", fs);
            if let Some(out) = run_ssh(&ssh_host, &cmd).await {
                parse_mmlsfs(&out, fs, target, &mut metrics);
            }
        }

        // mmstat — I/O performance stats (if available)
        let stat_out = run_ssh(&ssh_host, "/usr/lpp/mmfs/bin/mmstat -g io 1 1").await;
        if let Some(out) = stat_out {
            parse_mmstat(&out, target, &mut metrics);
        }

        // mmlsquota — quota usage for root filesystem
        if let Some(first_fs) = filesystems.first() {
            let cmd = format!("/usr/lpp/mmfs/bin/mmlsquota -j -Y {}", first_fs);
            if let Some(out) = run_ssh(&ssh_host, &cmd).await {
                parse_mmlsquota(&out, first_fs, target, &mut metrics);
            }
        }

        debug!(target = %target.name, count = metrics.len(), "IBM GPFS collection complete");
        metrics
    }
}

async fn run_ssh(ssh_host: &str, cmd: &str) -> Option<String> {
    let out = Command::new("ssh")
        .args(["-o", "StrictHostKeyChecking=no", "-o", "ConnectTimeout=10",
            ssh_host, cmd])
        .output()
        .await
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        warn!(ssh_host, cmd, "SSH command failed");
        None
    }
}

fn parse_mmdf(out: &str, fs: &str, target: &Target, metrics: &mut Vec<Metric>) {
    // mmdf -Y output: colon-separated fields
    // mmdf::poolTotal::PoolName:TotalKB:FreeKB:...
    for line in out.lines() {
        if !line.starts_with("mmdf::") { continue; }
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 6 { continue; }
        let row_type = fields.get(1).copied().unwrap_or("");
        match row_type {
            "poolTotal" | "data" => {
                let pool = fields.get(2).copied().unwrap_or("unknown");
                let total_kb: f64 = fields.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let free_kb: f64 = fields.get(4).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let used_kb = total_kb - free_kb;
                let pct = if total_kb > 0.0 { (used_kb / total_kb) * 100.0 } else { 0.0 };

                metrics.push(st_metric("storage.gpfs.fs_total_bytes", total_kb * 1024.0, fs, target)
                    .with_unit("bytes").with_tag("filesystem", fs).with_tag("pool", pool));
                metrics.push(st_metric("storage.gpfs.fs_free_bytes", free_kb * 1024.0, fs, target)
                    .with_unit("bytes").with_tag("filesystem", fs).with_tag("pool", pool));
                metrics.push(st_metric("storage.gpfs.fs_used_pct", pct, fs, target)
                    .with_unit("percent").with_tag("filesystem", fs).with_tag("pool", pool));
            }
            "inode" => {
                let total_inodes: f64 = fields.get(3).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                let free_inodes: f64 = fields.get(4).and_then(|v| v.parse().ok()).unwrap_or(0.0);
                metrics.push(st_metric("storage.gpfs.inode_total", total_inodes, fs, target)
                    .with_tag("filesystem", fs));
                metrics.push(st_metric("storage.gpfs.inode_free", free_inodes, fs, target)
                    .with_tag("filesystem", fs));
            }
            _ => {}
        }
    }
}

fn parse_mmlsfs(out: &str, fs: &str, target: &Target, metrics: &mut Vec<Metric>) {
    for line in out.lines() {
        if line.contains("Block size") {
            if let Some(v) = extract_value(line) {
                metrics.push(st_metric("storage.gpfs.block_size_bytes", v, fs, target)
                    .with_unit("bytes").with_tag("filesystem", fs));
            }
        } else if line.contains("Number of nodes") {
            if let Some(v) = extract_value(line) {
                metrics.push(st_metric("storage.gpfs.fs_nodes", v, fs, target)
                    .with_tag("filesystem", fs));
            }
        } else if line.contains("Replication factor") {
            if let Some(v) = extract_value(line) {
                metrics.push(st_metric("storage.gpfs.replication_factor", v, fs, target)
                    .with_tag("filesystem", fs));
            }
        }
    }
}

fn parse_mmstat(out: &str, target: &Target, metrics: &mut Vec<Metric>) {
    // mmstat -g io output: node:reads:writes:read_bytes:write_bytes:...
    let mut total_reads = 0.0f64;
    let mut total_writes = 0.0f64;
    let mut total_read_bytes = 0.0f64;
    let mut total_write_bytes = 0.0f64;

    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 || parts[0] == "Node" { continue; }
        total_reads += parts.get(1).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        total_writes += parts.get(2).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        total_read_bytes += parts.get(3).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        total_write_bytes += parts.get(4).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    }

    if total_reads > 0.0 || total_writes > 0.0 {
        metrics.push(st_metric("storage.gpfs.io_reads", total_reads, "io", target));
        metrics.push(st_metric("storage.gpfs.io_writes", total_writes, "io", target));
        metrics.push(st_metric("storage.gpfs.io_read_bytes", total_read_bytes, "io", target).with_unit("bytes"));
        metrics.push(st_metric("storage.gpfs.io_write_bytes", total_write_bytes, "io", target).with_unit("bytes"));
    }
}

fn parse_mmlsquota(out: &str, fs: &str, target: &Target, metrics: &mut Vec<Metric>) {
    // mmlsquota -Y: colon-separated
    // mmlsquota::quota:fileset:type:id:name:blockUsage:blockQuota:blockLimit:...
    for line in out.lines() {
        if !line.starts_with("mmlsquota::quota") { continue; }
        let fields: Vec<&str> = line.split(':').collect();
        let block_used: f64 = fields.get(6).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let block_quota: f64 = fields.get(7).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let name = fields.get(4).copied().unwrap_or("default");
        if block_used > 0.0 {
            metrics.push(st_metric("storage.gpfs.quota_used_kb", block_used, name, target)
                .with_unit("kibibytes").with_tag("filesystem", fs).with_tag("entity", name));
            if block_quota > 0.0 {
                metrics.push(st_metric("storage.gpfs.quota_limit_kb", block_quota, name, target)
                    .with_unit("kibibytes").with_tag("filesystem", fs).with_tag("entity", name));
                let pct = (block_used / block_quota) * 100.0;
                metrics.push(st_metric("storage.gpfs.quota_used_pct", pct, name, target)
                    .with_unit("percent").with_tag("filesystem", fs).with_tag("entity", name));
            }
        }
    }
}

fn extract_value(line: &str) -> Option<f64> {
    line.split_whitespace().rev().find(|s| s.parse::<f64>().is_ok()).and_then(|s| s.parse().ok())
}
