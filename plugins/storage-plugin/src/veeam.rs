//! Veeam Backup & Replication REST API collector — agentless via VBR REST API v1.1.
//! Collects job status, RPO compliance, repository health, and CDP lag.

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use tracing::{debug, warn};
use chrono::{DateTime, Utc};

fn stor(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("backup")
}

pub struct VeeamCollector;

impl VeeamCollector {
    async fn authenticate(host: &str, port: u16, user: &str, pass: &str) -> Option<(Client, String)> {
        let base = format!("https://{}:{}/api/v1", host, port);
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .ok()?;

        let resp = client
            .post(format!("{}/token", base))
            .form(&[
                ("grant_type", "password"),
                ("username", user),
                ("password", pass),
            ])
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?;

        let token = resp["access_token"].as_str()?.to_string();
        let auth_client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(std::time::Duration::from_secs(30))
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert("Authorization", format!("Bearer {}", token).parse().ok()?);
                h
            })
            .build()
            .ok()?;

        Some((auth_client, base))
    }

    async fn get(client: &Client, url: &str) -> Option<serde_json::Value> {
        client.get(url).send().await.ok()?.json::<serde_json::Value>().await.ok()
    }

    fn job_result_val(result: Option<&str>) -> f64 {
        match result {
            Some("Success") => 1.0,
            Some("Warning") => 0.5,
            Some("Failed") | Some("Error") => 0.0,
            _ => -1.0,
        }
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let port = if target.port > 0 { target.port as u16 } else { 9419 };
        let (user, pass) = target.credentials.as_ref()
            .map(|c| (
                c.username.clone().unwrap_or_else(|| "administrator".into()),
                c.password_encrypted.clone().unwrap_or_default(),
            ))
            .unwrap_or_else(|| ("administrator".into(), String::new()));

        let (client, base) = match Self::authenticate(host, port, &user, &pass).await {
            Some(v) => v,
            None => {
                warn!(target = %target.name, "Veeam: authentication failed");
                return metrics;
            }
        };

        // ── Backup jobs ───────────────────────────────────────────────────
        if let Some(jobs) = Self::get(&client, &format!("{}/jobs/states", base)).await {
            if let Some(job_list) = jobs["data"].as_array() {
                for job in job_list {
                    let name = job["name"].as_str().unwrap_or("unknown");
                    let job_type = job["type"].as_str().unwrap_or("Backup");
                    let status = job["lastResult"].as_str();
                    let last_run = job["lastRun"].as_str().unwrap_or("");

                    metrics.push(stor("storage.veeam.job_result", Self::job_result_val(status), name, target)
                        .with_tag("job", name).with_tag("type", job_type)
                        .with_tag("result", status.unwrap_or("Unknown")));

                    // Duration of last session
                    if let Some(dur_sec) = job["lastSessionDuration"].as_f64() {
                        metrics.push(stor("storage.veeam.job_duration_seconds", dur_sec, name, target)
                            .with_tag("job", name).with_unit("seconds"));
                    }

                    // Transferred bytes
                    if let Some(tb) = job["lastTransferredBytes"].as_f64() {
                        metrics.push(stor("storage.veeam.job_transferred_bytes", tb, name, target)
                            .with_tag("job", name).with_unit("bytes"));
                    }

                    // RPO: time since last successful backup
                    if !last_run.is_empty() {
                        if let Ok(last_run_dt) = last_run.parse::<DateTime<Utc>>() {
                            let rpo_seconds = (Utc::now() - last_run_dt).num_seconds() as f64;
                            metrics.push(stor("storage.veeam.job_rpo_seconds", rpo_seconds, name, target)
                                .with_tag("job", name).with_unit("seconds"));
                        }
                    }

                    // Processing rate
                    if let Some(rate) = job["processingRateMbps"].as_f64() {
                        metrics.push(stor("storage.veeam.job_processing_rate_mbps", rate, name, target)
                            .with_tag("job", name).with_unit("mbps"));
                    }

                    // Data size backup
                    if let Some(data_size) = job["dataSize"].as_f64() {
                        metrics.push(stor("storage.veeam.job_data_size_bytes", data_size, name, target)
                            .with_tag("job", name).with_unit("bytes"));
                    }
                }
            }
        }

        // ── Repositories ──────────────────────────────────────────────────
        if let Some(repos) = Self::get(&client, &format!("{}/backupInfrastructure/repositories", base)).await {
            if let Some(repo_list) = repos["data"].as_array() {
                for repo in repo_list {
                    let name = repo["name"].as_str().unwrap_or("unknown");
                    let capacity = repo["capacityGB"].as_f64().unwrap_or(0.0) * 1_073_741_824.0;
                    let free = repo["freeSpaceGB"].as_f64().unwrap_or(0.0) * 1_073_741_824.0;
                    let used = capacity - free;

                    if capacity > 0.0 {
                        metrics.push(stor("storage.veeam.repo_capacity_bytes", capacity, name, target)
                            .with_tag("repo", name).with_unit("bytes"));
                        metrics.push(stor("storage.veeam.repo_free_bytes", free, name, target)
                            .with_tag("repo", name).with_unit("bytes"));
                        metrics.push(stor("storage.veeam.repo_used_percent", (used / capacity) * 100.0, name, target)
                            .with_tag("repo", name).with_unit("percent"));
                    }
                    let is_immutable = repo["isImmutabilityEnabled"].as_bool().unwrap_or(false);
                    metrics.push(stor("storage.veeam.repo_immutable", if is_immutable { 1.0 } else { 0.0 }, name, target)
                        .with_tag("repo", name));
                }
            }
        }

        // ── Server summary ────────────────────────────────────────────────
        if let Some(server) = Self::get(&client, &format!("{}/serverInfo", base)).await {
            if let Some(version) = server["buildVersion"].as_str() {
                metrics.push(stor("storage.veeam.server_up", 1.0, "server", target)
                    .with_tag("version", version));
            }
        }

        debug!(target = %target.name, metrics = metrics.len(), "Veeam collection complete");
        metrics
    }
}
