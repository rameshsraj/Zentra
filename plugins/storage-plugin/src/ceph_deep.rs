//! Deep Ceph cluster monitoring via Ceph Dashboard REST API (ceph-mgr module).
//! Agentless — connects to the Ceph Manager REST API endpoint.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use tracing::{debug, warn};

fn stor(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage")
}

pub struct CephDeepCollector;

impl CephDeepCollector {
    async fn get(client: &Client, url: &str) -> Option<serde_json::Value> {
        client.get(url).send().await.ok()?.json::<serde_json::Value>().await.ok()
    }

    async fn login(host: &str, port: u16, user: &str, pass: &str) -> Option<(Client, String)> {
        let base = format!("https://{}:{}/api", host, port);
        let login_client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .ok()?;

        let resp = login_client
            .post(format!("{}/auth", base))
            .json(&serde_json::json!({"username": user, "password": pass}))
            .send()
            .await
            .ok()?;

        let token = resp.json::<serde_json::Value>().await.ok()?;
        let token_str = token["token"].as_str()?.to_string();

        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert("Authorization", format!("Bearer {}", token_str).parse().ok()?);
                h
            })
            .build()
            .ok()?;

        Some((client, base))
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let port = if target.port > 0 { target.port as u16 } else { 8443 };
        let (user, pass) = target.credentials.as_ref()
            .map(|c| (
                c.username.clone().unwrap_or_else(|| "admin".into()),
                c.password_encrypted.clone().unwrap_or_default(),
            ))
            .unwrap_or_else(|| ("admin".into(), String::new()));

        let (client, base) = match Self::login(host, port, &user, &pass).await {
            Some(v) => v,
            None => {
                warn!(target = %target.name, "Ceph: authentication failed or mgr not reachable");
                return metrics;
            }
        };

        // ── Cluster health ────────────────────────────────────────────────
        if let Some(health) = Self::get(&client, &format!("{}/health/minimal", base)).await {
            let status = health["health"]["status"].as_str().unwrap_or("HEALTH_ERR");
            let health_val = match status {
                "HEALTH_OK" => 1.0,
                "HEALTH_WARN" => 0.5,
                _ => 0.0,
            };
            metrics.push(stor("storage.ceph.cluster_health", health_val, "cluster", target)
                .with_tag("status", status));
        }

        // ── Monitor quorum ────────────────────────────────────────────────
        if let Some(mons) = Self::get(&client, &format!("{}/monitor", base)).await {
            if let Some(in_quorum) = mons["in_quorum"].as_array() {
                metrics.push(stor("storage.ceph.monitors_in_quorum", in_quorum.len() as f64, "monitor", target));
            }
            if let Some(out_quorum) = mons["out_of_quorum"].as_array() {
                metrics.push(stor("storage.ceph.monitors_out_of_quorum", out_quorum.len() as f64, "monitor", target));
            }
        }

        // ── OSD status ────────────────────────────────────────────────────
        if let Some(osds) = Self::get(&client, &format!("{}/osd", base)).await {
            if let Some(osd_list) = osds.as_array() {
                let total = osd_list.len() as f64;
                let up = osd_list.iter().filter(|o| o["up"].as_i64() == Some(1)).count() as f64;
                let r#in = osd_list.iter().filter(|o| o["in"].as_i64() == Some(1)).count() as f64;
                metrics.push(stor("storage.ceph.osds_total", total, "osd", target));
                metrics.push(stor("storage.ceph.osds_up", up, "osd", target));
                metrics.push(stor("storage.ceph.osds_in", r#in, "osd", target));
                metrics.push(stor("storage.ceph.osds_down", total - up, "osd", target));

                // Per-OSD performance
                for osd in osd_list {
                    let osd_id = osd["osd"].as_i64().unwrap_or(-1).to_string();
                    let resource = format!("osd.{}", osd_id);

                    if let Some(stats) = osd["osd_stats"].as_object() {
                        if let Some(kb_used) = stats.get("kb_used").and_then(|v| v.as_f64()) {
                            metrics.push(stor("storage.ceph.osd_used_kb", kb_used, &resource, target)
                                .with_tag("osd", &osd_id).with_unit("kilobytes"));
                        }
                        if let Some(kb_avail) = stats.get("kb_avail").and_then(|v| v.as_f64()) {
                            metrics.push(stor("storage.ceph.osd_available_kb", kb_avail, &resource, target)
                                .with_tag("osd", &osd_id).with_unit("kilobytes"));
                        }
                    }

                    // Perf stats (latency, IOPS)
                    if let Some(perf) = osd["perf_stat"].as_object() {
                        if let Some(al) = perf.get("apply_latency_ms").and_then(|v| v.as_f64()) {
                            metrics.push(stor("storage.ceph.osd_apply_latency_ms", al, &resource, target)
                                .with_tag("osd", &osd_id).with_unit("milliseconds"));
                        }
                        if let Some(cl) = perf.get("commit_latency_ms").and_then(|v| v.as_f64()) {
                            metrics.push(stor("storage.ceph.osd_commit_latency_ms", cl, &resource, target)
                                .with_tag("osd", &osd_id).with_unit("milliseconds"));
                        }
                    }
                }
            }
        }

        // ── Pool statistics ───────────────────────────────────────────────
        if let Some(pools) = Self::get(&client, &format!("{}/pool?stats=true", base)).await {
            if let Some(pool_list) = pools.as_array() {
                for pool in pool_list {
                    let name = pool["pool_name"].as_str().unwrap_or("unknown");
                    let used = pool["stats"]["stored"].as_f64().unwrap_or(0.0);
                    let max = pool["stats"]["max_avail"].as_f64().unwrap_or(0.0);
                    let read_iops = pool["stats"]["rd"].as_f64().unwrap_or(0.0);
                    let write_iops = pool["stats"]["wr"].as_f64().unwrap_or(0.0);
                    let rd_bytes = pool["stats"]["rd_bytes"].as_f64().unwrap_or(0.0);
                    let wr_bytes = pool["stats"]["wr_bytes"].as_f64().unwrap_or(0.0);

                    metrics.push(stor("storage.ceph.pool_used_bytes", used, name, target)
                        .with_tag("pool", name).with_unit("bytes"));
                    metrics.push(stor("storage.ceph.pool_available_bytes", max, name, target)
                        .with_tag("pool", name).with_unit("bytes"));
                    metrics.push(stor("storage.ceph.pool_read_iops", read_iops, name, target)
                        .with_tag("pool", name));
                    metrics.push(stor("storage.ceph.pool_write_iops", write_iops, name, target)
                        .with_tag("pool", name));
                    metrics.push(stor("storage.ceph.pool_read_bytes_per_sec", rd_bytes, name, target)
                        .with_tag("pool", name).with_unit("bytes_per_second"));
                    metrics.push(stor("storage.ceph.pool_write_bytes_per_sec", wr_bytes, name, target)
                        .with_tag("pool", name).with_unit("bytes_per_second"));

                    let pg_count = pool["pg_num"].as_f64().unwrap_or(0.0);
                    metrics.push(stor("storage.ceph.pool_pg_count", pg_count, name, target).with_tag("pool", name));
                }
            }
        }

        // ── Placement Group summary ───────────────────────────────────────
        if let Some(pg_summary) = Self::get(&client, &format!("{}/pg_summary", base)).await {
            if let Some(by_state) = pg_summary["all"]["by_state"].as_array() {
                let mut active_clean = 0.0f64;
                let mut degraded = 0.0f64;
                let mut stuck = 0.0f64;

                for state_entry in by_state {
                    let state = state_entry["state_name"].as_str().unwrap_or("");
                    let count = state_entry["count"].as_f64().unwrap_or(0.0);
                    if state.contains("active+clean") { active_clean += count; }
                    if state.contains("degraded") { degraded += count; }
                    if state.contains("stuck") { stuck += count; }
                }
                metrics.push(stor("storage.ceph.pg_active_clean", active_clean, "cluster", target));
                metrics.push(stor("storage.ceph.pg_degraded", degraded, "cluster", target));
                metrics.push(stor("storage.ceph.pg_stuck", stuck, "cluster", target));
            }
        }

        // ── Cluster capacity ──────────────────────────────────────────────
        if let Some(df) = Self::get(&client, &format!("{}/df", base)).await {
            let total = df["stats"]["total_bytes"].as_f64().unwrap_or(0.0);
            let used = df["stats"]["total_used_raw_bytes"].as_f64().unwrap_or(0.0);
            let avail = df["stats"]["total_avail_bytes"].as_f64().unwrap_or(0.0);

            metrics.push(stor("storage.ceph.cluster_total_bytes", total, "cluster", target).with_unit("bytes"));
            metrics.push(stor("storage.ceph.cluster_used_bytes", used, "cluster", target).with_unit("bytes"));
            metrics.push(stor("storage.ceph.cluster_available_bytes", avail, "cluster", target).with_unit("bytes"));
            if total > 0.0 {
                metrics.push(stor("storage.ceph.cluster_used_percent", (used / total) * 100.0, "cluster", target).with_unit("percent"));
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "Ceph deep collection complete");
        metrics
    }
}
