//! NetApp ONTAP REST API collector — agentless via ONTAP REST API v1.
//! Collects aggregates, volumes, SnapMirror, SVM, shelves, and port metrics.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use tracing::{debug, warn};

fn stor(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage")
}

pub struct NetAppOntapCollector;

impl NetAppOntapCollector {
    async fn get(client: &Client, url: &str, user: &str, pass: &str) -> Option<serde_json::Value> {
        client
            .get(url)
            .basic_auth(user, Some(pass))
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let port = if target.port > 0 { target.port } else { 443 };
        let (user, pass) = target.credentials.as_ref()
            .map(|c| (
                c.username.clone().unwrap_or_else(|| "admin".into()),
                c.password_encrypted.clone().unwrap_or_default(),
            ))
            .unwrap_or_else(|| ("admin".into(), String::new()));

        let base = format!("https://{}:{}/api", host, port);

        let client = match Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!(target = %target.name, err = %e, "ONTAP client build error");
                return metrics;
            }
        };

        // ── Cluster summary ────────────────────────────────────────────────
        if let Some(cluster) = Self::get(&client, &format!("{}/cluster", base), &user, &pass).await {
            if let Some(health) = cluster["metric"]["status"].as_str() {
                metrics.push(stor("storage.ontap.cluster_status", if health == "ok" { 1.0 } else { 0.0 }, "cluster", target)
                    .with_tag("status", health));
            }
        }

        // ── Aggregates (physical storage pools) ───────────────────────────
        let fields = "name,state,space.block_storage.size,space.block_storage.used,space.efficiency.savings";
        if let Some(aggr_resp) = Self::get(&client, &format!("{}/storage/aggregates?fields={}&max_records=200", base, fields), &user, &pass).await {
            if let Some(records) = aggr_resp["records"].as_array() {
                for aggr in records {
                    let name = aggr["name"].as_str().unwrap_or("unknown");
                    let state = aggr["state"].as_str().unwrap_or("unknown");
                    let state_val = if state == "online" { 1.0 } else { 0.0 };
                    metrics.push(stor("storage.ontap.aggr_state", state_val, name, target)
                        .with_tag("aggregate", name).with_tag("state", state));

                    let size = aggr["space"]["block_storage"]["size"].as_f64().unwrap_or(0.0);
                    let used = aggr["space"]["block_storage"]["used"].as_f64().unwrap_or(0.0);
                    if size > 0.0 {
                        metrics.push(stor("storage.ontap.aggr_size_bytes", size, name, target)
                            .with_tag("aggregate", name).with_unit("bytes"));
                        metrics.push(stor("storage.ontap.aggr_used_bytes", used, name, target)
                            .with_tag("aggregate", name).with_unit("bytes"));
                        metrics.push(stor("storage.ontap.aggr_used_percent", (used / size) * 100.0, name, target)
                            .with_tag("aggregate", name).with_unit("percent"));
                    }
                    if let Some(savings) = aggr["space"]["efficiency"]["savings"].as_f64() {
                        metrics.push(stor("storage.ontap.aggr_efficiency_savings_bytes", savings, name, target)
                            .with_tag("aggregate", name).with_unit("bytes"));
                    }
                }
            }
        }

        // ── Volumes ───────────────────────────────────────────────────────
        let vol_fields = "name,state,svm.name,space.size,space.used,space.available,metric.iops.read,metric.iops.write,metric.latency.read,metric.latency.write,metric.throughput.read,metric.throughput.write,snapshot.used";
        if let Some(vol_resp) = Self::get(&client, &format!("{}/storage/volumes?fields={}&max_records=500", base, vol_fields), &user, &pass).await {
            if let Some(records) = vol_resp["records"].as_array() {
                for vol in records {
                    let name = vol["name"].as_str().unwrap_or("unknown");
                    let svm = vol["svm"]["name"].as_str().unwrap_or("unknown");
                    let resource = format!("{}/{}", svm, name);

                    let size = vol["space"]["size"].as_f64().unwrap_or(0.0);
                    let used = vol["space"]["used"].as_f64().unwrap_or(0.0);
                    if size > 0.0 {
                        metrics.push(stor("storage.ontap.vol_size_bytes", size, &resource, target)
                            .with_tag("volume", name).with_tag("svm", svm).with_unit("bytes"));
                        metrics.push(stor("storage.ontap.vol_used_bytes", used, &resource, target)
                            .with_tag("volume", name).with_tag("svm", svm).with_unit("bytes"));
                        metrics.push(stor("storage.ontap.vol_used_percent", (used / size) * 100.0, &resource, target)
                            .with_tag("volume", name).with_tag("svm", svm).with_unit("percent"));
                    }

                    macro_rules! metric_field {
                        ($mname:expr, $path:expr, $unit:expr) => {
                            if let Some(v) = vol[$path[0]][$path[1]][$path[2]].as_f64()
                                .or_else(|| vol[$path[0]][$path[1]].as_f64()) {
                                metrics.push(stor($mname, v, &resource, target)
                                    .with_tag("volume", name).with_tag("svm", svm).with_unit($unit));
                            }
                        };
                    }

                    if let Some(r) = vol["metric"]["iops"]["read"].as_f64() {
                        metrics.push(stor("storage.ontap.vol_read_iops", r, &resource, target).with_tag("volume", name));
                    }
                    if let Some(w) = vol["metric"]["iops"]["write"].as_f64() {
                        metrics.push(stor("storage.ontap.vol_write_iops", w, &resource, target).with_tag("volume", name));
                    }
                    if let Some(rl) = vol["metric"]["latency"]["read"].as_f64() {
                        metrics.push(stor("storage.ontap.vol_read_latency_us", rl, &resource, target).with_tag("volume", name).with_unit("microseconds"));
                    }
                    if let Some(wl) = vol["metric"]["latency"]["write"].as_f64() {
                        metrics.push(stor("storage.ontap.vol_write_latency_us", wl, &resource, target).with_tag("volume", name).with_unit("microseconds"));
                    }
                    if let Some(snap) = vol["snapshot"]["used"].as_f64() {
                        metrics.push(stor("storage.ontap.vol_snapshot_used_bytes", snap, &resource, target).with_tag("volume", name).with_unit("bytes"));
                    }
                }
            }
        }

        // ── SnapMirror relationships ───────────────────────────────────────
        if let Some(sm_resp) = Self::get(&client, &format!("{}/snapmirror/relationships?fields=destination.path,state,lag_time,healthy&max_records=200", base), &user, &pass).await {
            if let Some(records) = sm_resp["records"].as_array() {
                for rel in records {
                    let dest = rel["destination"]["path"].as_str().unwrap_or("unknown");
                    let state = rel["state"].as_str().unwrap_or("unknown");
                    let healthy = rel["healthy"].as_bool().unwrap_or(false);
                    metrics.push(stor("storage.ontap.snapmirror_healthy", if healthy { 1.0 } else { 0.0 }, dest, target)
                        .with_tag("destination", dest).with_tag("state", state));
                    if let Some(lag) = rel["lag_time"].as_str() {
                        // ISO 8601 duration like PT1H30M → convert to seconds
                        let lag_secs = Self::parse_iso_duration(lag);
                        metrics.push(stor("storage.ontap.snapmirror_lag_seconds", lag_secs, dest, target)
                            .with_tag("destination", dest).with_unit("seconds"));
                    }
                }
            }
        }

        // ── SVMs ──────────────────────────────────────────────────────────
        if let Some(svm_resp) = Self::get(&client, &format!("{}/svm/svms?fields=name,state,subtype&max_records=100", base), &user, &pass).await {
            if let Some(records) = svm_resp["records"].as_array() {
                for svm in records {
                    let name = svm["name"].as_str().unwrap_or("unknown");
                    let state = svm["state"].as_str().unwrap_or("unknown");
                    metrics.push(stor("storage.ontap.svm_state", if state == "running" { 1.0 } else { 0.0 }, name, target)
                        .with_tag("svm", name).with_tag("state", state));
                }
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "ONTAP collection complete");
        metrics
    }

    fn parse_iso_duration(dur: &str) -> f64 {
        // Parse PT1H30M15S format → seconds
        let mut secs = 0.0f64;
        let dur = dur.trim_start_matches('P').trim_start_matches('T');
        let mut num = String::new();
        for c in dur.chars() {
            if c.is_ascii_digit() || c == '.' {
                num.push(c);
            } else {
                let v: f64 = num.parse().unwrap_or(0.0);
                num.clear();
                match c {
                    'D' => secs += v * 86400.0,
                    'H' => secs += v * 3600.0,
                    'M' => secs += v * 60.0,
                    'S' => secs += v,
                    _ => {}
                }
            }
        }
        secs
    }
}
