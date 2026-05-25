//! Pure Storage FlashArray REST API collector — agentless via Pure API v2.
//! Collects array performance, volumes, protection groups, and hardware health.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use tracing::{debug, warn};

fn stor(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage")
}

pub struct PureStorageCollector;

impl PureStorageCollector {
    async fn build_client(host: &str, api_token: &str) -> Option<(Client, String)> {
        let login_client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .ok()?;

        let base = format!("https://{}/api/2.18", host);

        // Authenticate: POST /api/2.18/login returns x-auth-token
        let resp = login_client
            .post(format!("{}/login", base))
            .header("api-token", api_token)
            .send()
            .await
            .ok()?;

        let token = resp
            .headers()
            .get("x-auth-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)?;

        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert("x-auth-token", token.parse().ok()?);
                h
            })
            .build()
            .ok()?;

        Some((client, base))
    }

    async fn get(client: &Client, url: &str) -> Option<serde_json::Value> {
        client.get(url).send().await.ok()?.json::<serde_json::Value>().await.ok()
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let api_token = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_default();

        if api_token.is_empty() {
            warn!(target = %target.name, "Pure Storage: no API token configured");
            return metrics;
        }

        let (client, base) = match Self::build_client(host, &api_token).await {
            Some(v) => v,
            None => {
                warn!(target = %target.name, "Pure Storage: authentication failed");
                return metrics;
            }
        };

        // ── Array capacity and performance ────────────────────────────────
        if let Some(arrays) = Self::get(&client, &format!("{}/arrays?space=true&performance=true", base)).await {
            if let Some(items) = arrays["items"].as_array() {
                for arr in items {
                    let name = arr["name"].as_str().unwrap_or("array");

                    // Capacity
                    let total = arr["space"]["total_physical"].as_f64().unwrap_or(0.0);
                    let used = arr["space"]["used_provisioned"].as_f64().unwrap_or(0.0);
                    let data_reduction = arr["space"]["data_reduction"].as_f64().unwrap_or(1.0);

                    if total > 0.0 {
                        metrics.push(stor("storage.pure.array_total_bytes", total, name, target)
                            .with_tag("array", name).with_unit("bytes"));
                        metrics.push(stor("storage.pure.array_used_bytes", used, name, target)
                            .with_tag("array", name).with_unit("bytes"));
                        metrics.push(stor("storage.pure.array_used_percent", (used / total) * 100.0, name, target)
                            .with_tag("array", name).with_unit("percent"));
                    }
                    metrics.push(stor("storage.pure.array_data_reduction_ratio", data_reduction, name, target)
                        .with_tag("array", name));

                    // Performance
                    if let Some(r) = arr["read_iops"].as_f64() {
                        metrics.push(stor("storage.pure.array_read_iops", r, name, target).with_tag("array", name));
                    }
                    if let Some(w) = arr["write_iops"].as_f64() {
                        metrics.push(stor("storage.pure.array_write_iops", w, name, target).with_tag("array", name));
                    }
                    if let Some(rl) = arr["usec_per_read_op"].as_f64() {
                        metrics.push(stor("storage.pure.array_read_latency_us", rl, name, target)
                            .with_tag("array", name).with_unit("microseconds"));
                    }
                    if let Some(wl) = arr["usec_per_write_op"].as_f64() {
                        metrics.push(stor("storage.pure.array_write_latency_us", wl, name, target)
                            .with_tag("array", name).with_unit("microseconds"));
                    }
                    if let Some(rb) = arr["read_bytes_per_sec"].as_f64() {
                        metrics.push(stor("storage.pure.array_read_bytes_per_sec", rb, name, target)
                            .with_tag("array", name).with_unit("bytes_per_second"));
                    }
                    if let Some(wb) = arr["write_bytes_per_sec"].as_f64() {
                        metrics.push(stor("storage.pure.array_write_bytes_per_sec", wb, name, target)
                            .with_tag("array", name).with_unit("bytes_per_second"));
                    }
                    if let Some(q) = arr["queue_depth"].as_f64() {
                        metrics.push(stor("storage.pure.array_queue_depth", q, name, target).with_tag("array", name));
                    }
                }
            }
        }

        // ── Volumes ───────────────────────────────────────────────────────
        if let Some(vols) = Self::get(&client, &format!("{}/volumes?space=true&performance=true&limit=500", base)).await {
            if let Some(items) = vols["items"].as_array() {
                for vol in items {
                    let name = vol["name"].as_str().unwrap_or("unknown");
                    let prov = vol["provisioned"].as_f64().unwrap_or(0.0);
                    let used = vol["space"]["total_physical"].as_f64().unwrap_or(0.0);

                    metrics.push(stor("storage.pure.vol_provisioned_bytes", prov, name, target)
                        .with_tag("volume", name).with_unit("bytes"));
                    metrics.push(stor("storage.pure.vol_used_bytes", used, name, target)
                        .with_tag("volume", name).with_unit("bytes"));

                    if let Some(dr) = vol["space"]["data_reduction"].as_f64() {
                        metrics.push(stor("storage.pure.vol_data_reduction_ratio", dr, name, target).with_tag("volume", name));
                    }
                    if let Some(r) = vol["read_iops"].as_f64() {
                        metrics.push(stor("storage.pure.vol_read_iops", r, name, target).with_tag("volume", name));
                    }
                    if let Some(w) = vol["write_iops"].as_f64() {
                        metrics.push(stor("storage.pure.vol_write_iops", w, name, target).with_tag("volume", name));
                    }
                    if let Some(rl) = vol["usec_per_read_op"].as_f64() {
                        metrics.push(stor("storage.pure.vol_read_latency_us", rl, name, target)
                            .with_tag("volume", name).with_unit("microseconds"));
                    }
                    if let Some(wl) = vol["usec_per_write_op"].as_f64() {
                        metrics.push(stor("storage.pure.vol_write_latency_us", wl, name, target)
                            .with_tag("volume", name).with_unit("microseconds"));
                    }
                }
            }
        }

        // ── Hardware health ───────────────────────────────────────────────
        if let Some(hw) = Self::get(&client, &format!("{}/hardware?limit=200", base)).await {
            if let Some(items) = hw["items"].as_array() {
                for component in items {
                    let name = component["name"].as_str().unwrap_or("unknown");
                    let status = component["status"].as_str().unwrap_or("unknown");
                    let health_val = match status {
                        "ok" | "healthy" => 1.0,
                        "warning" | "identifying" => 0.5,
                        _ => 0.0,
                    };
                    metrics.push(stor("storage.pure.hardware_health", health_val, name, target)
                        .with_tag("component", name).with_tag("status", status));

                    // Temperature for applicable components
                    if let Some(temp) = component["temperature"].as_f64() {
                        metrics.push(stor("storage.pure.component_temperature_celsius", temp, name, target)
                            .with_tag("component", name).with_unit("celsius"));
                    }
                    // Speed for fans
                    if let Some(speed) = component["speed"].as_f64() {
                        metrics.push(stor("storage.pure.fan_speed_rpm", speed, name, target)
                            .with_tag("component", name).with_unit("rpm"));
                    }
                    // Wattage for PSUs
                    if let Some(watts) = component["power"].as_f64() {
                        metrics.push(stor("storage.pure.psu_power_watts", watts, name, target)
                            .with_tag("component", name).with_unit("watts"));
                    }
                }
            }
        }

        // ── Protection groups & replication ──────────────────────────────
        if let Some(pgs) = Self::get(&client, &format!("{}/protection-groups/targets?limit=200", base)).await {
            if let Some(items) = pgs["items"].as_array() {
                for pg in items {
                    let name = pg["group"]["name"].as_str().unwrap_or("unknown");
                    if let Some(lag) = pg["lag"].as_f64() {
                        metrics.push(stor("storage.pure.pg_replication_lag_seconds", lag / 1_000_000.0, name, target)
                            .with_tag("protection_group", name).with_unit("seconds"));
                    }
                }
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "Pure Storage collection complete");
        metrics
    }
}
