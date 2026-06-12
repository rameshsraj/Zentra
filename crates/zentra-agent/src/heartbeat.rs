//! Heartbeat loop — periodically POSTs agent status to the Zentra server.

use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

pub struct HeartbeatLoop {
    client: Client,
    api_endpoint: String,
    api_key: String,
    dc_name: String,
    active_targets: Arc<AtomicUsize>,
    cancel: CancellationToken,
}

impl HeartbeatLoop {
    pub fn new(
        api_endpoint: String,
        api_key: String,
        dc_name: String,
        active_targets: Arc<AtomicUsize>,
        cancel: CancellationToken,
    ) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to build heartbeat HTTP client");

        Self {
            client,
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_key,
            dc_name,
            active_targets,
            cancel,
        }
    }

    pub async fn run(self) {
        info!("Heartbeat loop started (30s interval)");
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        ticker.tick().await; // skip immediate

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Heartbeat loop stopping (shutdown)");
                    break;
                }
                _ = ticker.tick() => {}
            }

            let target_count = self.active_targets.load(Ordering::Relaxed);
            let body = json!({
                "agent_version": env!("CARGO_PKG_VERSION"),
                "active_targets": target_count as i32,
                "os_name": std::env::consts::OS,
                "dc_name": self.dc_name,
            });

            let url = format!("{}/api/agents/heartbeat", self.api_endpoint);
            match self
                .client
                .post(&url)
                .header("X-Zentra-Key", &self.api_key)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.as_u16() == 401 || status.as_u16() == 403 {
                        error!("Heartbeat rejected ({}): API key revoked or invalid. Stopping agent.", status);
                        std::process::exit(1);
                    }
                    if status.is_success() {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(ts) = json.get("server_timestamp_utc").and_then(|v| v.as_str()) {
                                if let Ok(server_time) = chrono::DateTime::parse_from_rfc3339(ts) {
                                    let drift = (chrono::Utc::now() - server_time.with_timezone(&chrono::Utc))
                                        .num_seconds()
                                        .abs();
                                    if drift > 60 {
                                        warn!(drift_seconds = drift, "Clock drift detected between agent and server");
                                    }
                                    debug!(server_time = %ts, drift_seconds = drift, "Heartbeat OK");
                                }
                            }
                        }
                    } else {
                        warn!(status = %status, "Heartbeat returned non-success status");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Heartbeat failed (network error) — continuing collection");
                }
            }
        }
    }
}
