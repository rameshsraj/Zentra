//! SMART disk health collector — agentless via SSH exec (smartctl).
//! Reads all SMART attributes per physical disk without installing any agent.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tokio::process::Command;
use tracing::{debug, warn};

fn srv(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "server-plugin")
        .with_resource_type("disk")
}

pub struct SmartDiskHealthCollector;

impl SmartDiskHealthCollector {
    const COLLECTION_SCRIPT: &'static str = r#"
#!/bin/bash
# Enumerate physical block devices (exclude partitions and loop/ram/fd)
DEVS=$(lsblk -dno NAME,TYPE 2>/dev/null | awk '$2=="disk"{print "/dev/"$1}')
[ -z "$DEVS" ] && DEVS=$(ls /dev/sd? /dev/nvme?n? /dev/hd? 2>/dev/null | head -20)

for dev in $DEVS; do
    echo "===DEVICE=== $dev"
    # Use --json if smartctl >= 7.0, else fall back to text
    if smartctl --version 2>/dev/null | awk 'NR==1{split($2,v,"."); if(v[1]>=7) exit 0; exit 1}'; then
        smartctl -a --json "$dev" 2>/dev/null && continue
    fi
    # Text fallback
    smartctl -a "$dev" 2>/dev/null
done
echo "===SMART_END==="
"#;

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // When agent_id is set the agent runs on the target machine — collect locally,
        // no SSH needed (and no SSH credentials available).
        let output = if target.agent_id.is_some() {
            debug!(target = %target.name, "SMART collection: running locally (agent-mode)");
            Command::new("bash")
                .args(["-c", Self::COLLECTION_SCRIPT])
                .output()
                .await
        } else {
            let host = &target.host;
            let ssh_user = target.credentials.as_ref()
                .and_then(|c| c.username.clone())
                .unwrap_or_else(|| std::env::var("zentra_SSH_USER").unwrap_or_else(|_| "root".into()));
            Command::new("ssh")
                .args([
                    "-o", "StrictHostKeyChecking=no",
                    "-o", "ConnectTimeout=10",
                    "-o", "BatchMode=yes",
                    &format!("{}@{}", ssh_user, host),
                    Self::COLLECTION_SCRIPT,
                ])
                .output()
                .await
        };

        let output = match output {
            Ok(o) if !o.stdout.is_empty() => o,
            Ok(o) => {
                warn!(target = %target.name, "SMART collection: no output — {}", String::from_utf8_lossy(&o.stderr));
                return metrics;
            }
            Err(e) => {
                warn!(target = %target.name, err = %e, "SMART collection exec error");
                return metrics;
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current_dev = String::new();

        // Try JSON parse first for each device block
        let mut device_blocks: Vec<(String, String)> = Vec::new();
        let mut current_block = String::new();

        for line in stdout.lines() {
            if line.starts_with("===DEVICE=== ") {
                if !current_dev.is_empty() && !current_block.is_empty() {
                    device_blocks.push((current_dev.clone(), current_block.clone()));
                }
                current_dev = line.trim_start_matches("===DEVICE=== ").to_string();
                current_block.clear();
            } else if line == "===SMART_END===" {
                if !current_dev.is_empty() && !current_block.is_empty() {
                    device_blocks.push((current_dev.clone(), current_block.clone()));
                }
            } else {
                current_block.push_str(line);
                current_block.push('\n');
            }
        }

        for (dev, block) in &device_blocks {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(block) {
                Self::parse_json_smart(dev, &json, target, &mut metrics);
            } else {
                Self::parse_text_smart(dev, block, target, &mut metrics);
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "SMART collection complete");
        metrics
    }

    fn parse_json_smart(dev: &str, json: &serde_json::Value, target: &Target, metrics: &mut Vec<Metric>) {
        let short_dev = dev.trim_start_matches("/dev/");

        // Overall health
        if let Some(passed) = json["smart_status"]["passed"].as_bool() {
            metrics.push(srv("server.disk.smart_passed", if passed { 1.0 } else { 0.0 }, short_dev, target)
                .with_tag("device", short_dev));
        }

        // Temperature
        if let Some(temp) = json["temperature"]["current"].as_f64() {
            metrics.push(srv("server.disk.temperature_celsius", temp, short_dev, target)
                .with_tag("device", short_dev).with_unit("celsius"));
        }

        // Power-on hours
        if let Some(hours) = json["power_on_time"]["hours"].as_f64() {
            metrics.push(srv("server.disk.power_on_hours", hours, short_dev, target)
                .with_tag("device", short_dev).with_unit("hours"));
        }

        // Power cycle count
        if let Some(cycles) = json["power_cycle_count"].as_f64() {
            metrics.push(srv("server.disk.power_cycle_count", cycles, short_dev, target)
                .with_tag("device", short_dev));
        }

        // NVMe-specific
        if let Some(nvme) = json.get("nvme_smart_health_information_log") {
            let fields = [
                ("available_spare", "server.disk.nvme_available_spare_percent", "percent"),
                ("percentage_used", "server.disk.nvme_percentage_used", "percent"),
                ("media_errors", "server.disk.nvme_media_errors", "count"),
                ("unsafe_shutdowns", "server.disk.nvme_unsafe_shutdowns", "count"),
                ("controller_busy_time", "server.disk.nvme_controller_busy_minutes", "minutes"),
            ];
            for (key, metric, unit) in fields {
                if let Some(v) = nvme[key].as_f64() {
                    metrics.push(srv(metric, v, short_dev, target).with_tag("device", short_dev).with_unit(unit));
                }
            }
        }

        // ATA attributes (SCSI/SATA)
        if let Some(attrs) = json["ata_smart_attributes"]["table"].as_array() {
            for attr in attrs {
                let name = attr["name"].as_str().unwrap_or("").to_lowercase().replace('-', "_");
                let raw = attr["raw"]["value"].as_f64().unwrap_or(0.0);

                let metric = match name.as_str() {
                    "reallocated_sector_ct" => Some(("server.disk.smart_reallocated_sectors", "count")),
                    "current_pending_sector" => Some(("server.disk.smart_pending_sectors", "count")),
                    "offline_uncorrectable" => Some(("server.disk.smart_offline_uncorrectable", "count")),
                    "udma_crc_error_count" => Some(("server.disk.smart_crc_errors", "count")),
                    "wear_leveling_count" => Some(("server.disk.smart_wear_leveling", "count")),
                    "media_wearout_indicator" => Some(("server.disk.smart_media_wearout", "count")),
                    "spin_retry_count" => Some(("server.disk.smart_spin_retries", "count")),
                    _ => None,
                };

                if let Some((metric_name, unit)) = metric {
                    metrics.push(srv(metric_name, raw, short_dev, target)
                        .with_tag("device", short_dev).with_unit(unit));
                }
            }
        }
    }

    fn parse_text_smart(dev: &str, text: &str, target: &Target, metrics: &mut Vec<Metric>) {
        let short_dev = dev.trim_start_matches("/dev/");

        for line in text.lines() {
            let line = line.trim();

            if line.contains("SMART overall-health self-assessment") {
                let passed = line.contains("PASSED");
                metrics.push(srv("server.disk.smart_passed", if passed { 1.0 } else { 0.0 }, short_dev, target)
                    .with_tag("device", short_dev));
                continue;
            }

            if line.contains("Temperature_Celsius") || line.contains("Airflow_Temperature_Cel") {
                if let Some(raw) = line.split_whitespace().last() {
                    if let Ok(v) = raw.parse::<f64>() {
                        metrics.push(srv("server.disk.temperature_celsius", v, short_dev, target)
                            .with_tag("device", short_dev).with_unit("celsius"));
                    }
                }
                continue;
            }

            // Parse ATA attribute lines: ID# ATTRIBUTE_NAME FLAG VALUE WORST THRESH TYPE UPDATED WHEN_FAILED RAW_VALUE
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 10 {
                let attr_name = parts[1].to_lowercase();
                let raw_str = parts[parts.len() - 1];

                let known_attr = match attr_name.as_str() {
                    "reallocated_sector_ct" => Some("server.disk.smart_reallocated_sectors"),
                    "current_pending_sector" => Some("server.disk.smart_pending_sectors"),
                    "offline_uncorrectable" => Some("server.disk.smart_offline_uncorrectable"),
                    "udma_crc_error_count" => Some("server.disk.smart_crc_errors"),
                    "wear_leveling_count" => Some("server.disk.smart_wear_leveling"),
                    "power_on_hours" => Some("server.disk.power_on_hours"),
                    "power_cycle_count" => Some("server.disk.power_cycle_count"),
                    _ => None,
                };

                if let Some(metric_name) = known_attr {
                    if let Ok(v) = raw_str.parse::<f64>() {
                        metrics.push(srv(metric_name, v, short_dev, target).with_tag("device", short_dev));
                    }
                }
            }
        }
    }
}
