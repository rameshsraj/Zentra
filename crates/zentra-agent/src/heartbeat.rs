//! Heartbeat loop — periodically POSTs agent status to the Zentra server.

use crate::log_capture::LogBuffer;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Returns the local IPv4 address used to reach the given destination host.
/// We connect to the Zentra API server IP rather than a public address so the
/// OS picks the interface that actually reaches the Zentra network.
fn local_ip_for_destination(host: &str) -> String {
    use std::net::UdpSocket;
    // Use port 80 or 443; traffic is never actually sent.
    let dest = format!("{}:80", host);
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| { s.connect(&dest)?; s.local_addr() })
        .map(|a| a.ip().to_string())
        .unwrap_or_default()
}

/// Extract the host from an http://host:port URL.
fn host_from_url(url: &str) -> String {
    url.trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Shared cell that holds the agent's own UUID once the first successful
/// heartbeat response arrives. Discovery tasks wait on this before reporting.
pub type AgentIdCell = Arc<OnceLock<Uuid>>;

pub fn new_agent_id_cell() -> AgentIdCell {
    Arc::new(OnceLock::new())
}

pub struct HeartbeatLoop {
    client: Client,
    api_endpoint: String,
    api_key: String,
    dc_name: String,
    active_targets: Arc<AtomicUsize>,
    cancel: CancellationToken,
    log_buffer: LogBuffer,
    agent_id_cell: AgentIdCell,
}

impl HeartbeatLoop {
    pub fn new(
        api_endpoint: String,
        api_key: String,
        dc_name: String,
        active_targets: Arc<AtomicUsize>,
        cancel: CancellationToken,
        log_buffer: LogBuffer,
        agent_id_cell: AgentIdCell,
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
            log_buffer,
            agent_id_cell,
        }
    }

    pub async fn run(self) {
        info!("Heartbeat loop started (30s interval)");
        // Fire immediately so the agent gets its agent_id on startup,
        // then continue every 30 seconds.
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Heartbeat loop stopping (shutdown)");
                    break;
                }
                _ = ticker.tick() => {}
            }

            let target_count = self.active_targets.load(Ordering::Relaxed);
            let recent_logs = self.log_buffer.drain();
            // Collect real hostname and IP so the server stores them correctly.
            // remote_addr is unreliable when the agent talks through nginx (127.0.0.1).
            let hostname = hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_default();
            let local_ip = local_ip_for_destination(&host_from_url(&self.api_endpoint));
            let body = json!({
                "agent_version": env!("CARGO_PKG_VERSION"),
                "active_targets": target_count as i32,
                "os_name": std::env::consts::OS,
                "dc_name": self.dc_name,
                "recent_logs": recent_logs,
                "hostname": hostname,
                "ip_address": local_ip,
            });

            let url = format!("{}/api/agents/heartbeat", self.api_endpoint);
            match self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", &self.api_key))
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
                            // Capture agent_id from server response on first heartbeat
                            if self.agent_id_cell.get().is_none() {
                                if let Some(id_str) = json.get("agent_id").and_then(|v| v.as_str()) {
                                    if let Ok(id) = id_str.parse::<Uuid>() {
                                        let _ = self.agent_id_cell.set(id);
                                        info!(agent_id = %id, "Agent ID received from server");
                                    }
                                }
                            }
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
