//! RemoteDeliveryService — replaces IngestionService in remote mode.
//!
//! Receives MetricBatch/LogBatch from the Orchestrator via mpsc channels,
//! serialises them to OTLP JSON, and POSTs to the Zentra server over HTTPS.
//! Failed batches are written to a local offline buffer for later retry.

use crate::offline_buffer::OfflineBuffer;
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use zentra_core::models::log_entry::LogBatch;
use zentra_core::models::metric::{Metric, MetricBatch};

pub struct RemoteDeliveryService {
    client: Client,
    api_endpoint: String,
    api_key: String,
    dc_name: String,
    hostname: String,
    buffer: OfflineBuffer,
}

impl RemoteDeliveryService {
    pub fn new(api_endpoint: String, api_key: String, dc_name: String, buffer_dir: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .user_agent("zentra-agent/1.0")
            .build()
            .expect("Failed to build HTTP client");

        let buffer = OfflineBuffer::new(buffer_dir);
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| dc_name.clone());

        Self {
            client,
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_key,
            dc_name,
            hostname,
            buffer,
        }
    }

    /// Run the metric delivery loop — consumes from the mpsc channel.
    pub async fn run_metric_delivery(mut self, mut rx: mpsc::Receiver<MetricBatch>) {
        info!("RemoteDeliveryService: metric delivery loop started");

        loop {
            // Try to flush buffered batches first
            self.flush_buffer().await;

            match rx.recv().await {
                Some(batch) => {
                    let payload = self.metrics_to_otlp(&batch);
                    let url = format!("{}/api/agent/ingest/metrics", self.api_endpoint);
                    if let Err(fatal) = self.post_otlp(&url, &payload).await {
                        if fatal {
                            error!("RemoteDeliveryService: fatal auth error, stopping agent");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    info!("RemoteDeliveryService: metric channel closed, shutting down");
                    break;
                }
            }
        }
    }

    /// Run the log delivery loop — consumes from the mpsc channel.
    pub async fn run_log_delivery(mut self, mut rx: mpsc::Receiver<LogBatch>) {
        info!("RemoteDeliveryService: log delivery loop started");

        loop {
            match rx.recv().await {
                Some(batch) => {
                    let payload = self.logs_to_otlp(&batch);
                    let url = format!("{}/api/agent/ingest/logs", self.api_endpoint);
                    if let Err(fatal) = self.post_otlp(&url, &payload).await {
                        if fatal {
                            error!("RemoteDeliveryService: fatal auth error, stopping agent");
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    info!("RemoteDeliveryService: log channel closed, shutting down");
                    break;
                }
            }
        }
    }

    /// POST an OTLP JSON payload to the server. Returns Ok(()) on success,
    /// Err(true) for fatal auth errors (401/403), Err(false) for transient failures.
    async fn post_otlp(&mut self, url: &str, payload: &Value) -> Result<(), bool> {
        let result = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(payload)
            .send()
            .await;

        match result {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    debug!("OTLP push succeeded: {}", status);
                    Ok(())
                } else if status.as_u16() == 401 || status.as_u16() == 403 {
                    error!(
                        "OTLP push rejected with {}: API key is invalid or revoked. Stopping.",
                        status
                    );
                    Err(true) // fatal
                } else {
                    warn!("OTLP push failed with {}, buffering for retry", status);
                    self.buffer.write_batch(payload);
                    Err(false)
                }
            }
            Err(e) => {
                warn!("OTLP push network error: {}, buffering for retry", e);
                self.buffer.write_batch(payload);
                Err(false)
            }
        }
    }

    /// Flush up to 50 buffered batches, oldest first.
    async fn flush_buffer(&mut self) {
        let batches = self.buffer.read_pending(50);
        if batches.is_empty() {
            return;
        }
        info!(count = batches.len(), "Flushing offline buffer");

        let mut flushed = 0usize;
        for batch_json in &batches {
            let payload: Value = match serde_json::from_str(batch_json) {
                Ok(v) => v,
                Err(_) => {
                    flushed += 1; // skip corrupt entries
                    continue;
                }
            };
            let url = format!("{}/api/agent/ingest/metrics", self.api_endpoint);
            let result = self
                .client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", &self.api_key))
                .json(&payload)
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    flushed += 1;
                }
                Ok(resp) if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 => {
                    error!("Buffer flush: auth rejected, stopping agent");
                    std::process::exit(1);
                }
                _ => {
                    // Server still down — stop flushing, try again next cycle
                    break;
                }
            }
        }

        if flushed > 0 {
            self.buffer.acknowledge(flushed);
            info!(flushed, "Buffer flush complete");
        }
    }

    /// Flush all remaining buffered entries (called during shutdown).
    pub async fn flush_all(&mut self) {
        loop {
            let batches = self.buffer.read_pending(50);
            if batches.is_empty() {
                break;
            }
            for batch_json in &batches {
                let payload: Value = match serde_json::from_str(batch_json) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let url = format!("{}/api/agent/ingest/metrics", self.api_endpoint);
                let _ = self
                    .client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("Authorization", format!("Bearer {}", &self.api_key))
                    .json(&payload)
                    .send()
                    .await;
            }
            self.buffer.acknowledge(batches.len());
        }
    }

    /// Convert a MetricBatch into OTLP JSON format, grouping by (org_id, target_id, plugin_id).
    fn metrics_to_otlp(&self, batch: &MetricBatch) -> Value {
        // Group metrics by (org_id, target_id)
        let mut groups: BTreeMap<(String, String), Vec<&Metric>> = BTreeMap::new();
        for m in &batch.metrics {
            let key = (m.org_id.to_string(), m.target_id.to_string());
            groups.entry(key).or_default().push(m);
        }

        let resource_metrics: Vec<Value> = groups
            .into_iter()
            .map(|((org_id, target_id), metrics)| {
                let host = metrics
                    .first()
                    .and_then(|m| m.tags.get("host"))
                    .cloned()
                    .unwrap_or_default();

                let scope_metrics: Vec<Value> = metrics
                    .iter()
                    .map(|m| {
                        let mut attributes: Vec<Value> = vec![
                            json!({"key": "resource_type", "value": {"stringValue": m.resource_type}}),
                            json!({"key": "resource_id", "value": {"stringValue": m.resource_id}}),
                        ];
                        for (k, v) in &m.tags {
                            attributes.push(json!({"key": k, "value": {"stringValue": v}}));
                        }
                        json!({
                            "name": m.metric_name,
                            "unit": m.unit,
                            "gauge": {
                                "dataPoints": [{
                                    "timeUnixNano": format!("{}", m.timestamp.timestamp_nanos_opt().unwrap_or(0)),
                                    "asDouble": m.value,
                                    "attributes": attributes,
                                }]
                            }
                        })
                    })
                    .collect();

                json!({
                    "resource": {
                        "attributes": [
                            {"key": "service.name", "value": {"stringValue": "zentra-agent"}},
                            {"key": "zentra.org_id", "value": {"stringValue": org_id}},
                            {"key": "zentra.target_id", "value": {"stringValue": target_id}},
                            {"key": "host.name", "value": {"stringValue": host}},
                            {"key": "zentra.dc_name", "value": {"stringValue": self.dc_name}},
                        ]
                    },
                    "scopeMetrics": [{
                        "scope": {"name": batch.source_plugin, "version": "1.0.0"},
                        "metrics": scope_metrics,
                    }]
                })
            })
            .collect();

        json!({ "resourceMetrics": resource_metrics })
    }

    /// Convert a LogBatch into OTLP JSON resourceLogs format.
    fn logs_to_otlp(&self, batch: &LogBatch) -> Value {
        let mut groups: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
        for entry in &batch.entries {
            let key = (entry.org_id.to_string(), entry.target_id.to_string());
            groups.entry(key).or_default().push(json!({
                "timeUnixNano": format!("{}", entry.timestamp.timestamp_nanos_opt().unwrap_or(0)),
                "severityText": entry.severity.as_str(),
                "body": {"stringValue": entry.message},
                "attributes": [
                    {"key": "source", "value": {"stringValue": entry.source}},
                    {"key": "component", "value": {"stringValue": entry.component}},
                    {"key": "component_type", "value": {"stringValue": entry.component_type}},
                ]
            }));
        }

        let resource_logs: Vec<Value> = groups
            .into_iter()
            .map(|((org_id, target_id), log_records)| {
                json!({
                    "resource": {
                        "attributes": [
                            {"key": "service.name", "value": {"stringValue": "zentra-agent"}},
                            {"key": "zentra.org_id", "value": {"stringValue": org_id}},
                            {"key": "zentra.agent_id", "value": {"stringValue": target_id}},
                            {"key": "zentra.target_id", "value": {"stringValue": target_id}},
                            {"key": "host.name", "value": {"stringValue": self.hostname}},
                            {"key": "zentra.dc_name", "value": {"stringValue": self.dc_name}},
                        ]
                    },
                    "scopeLogs": [{
                        "scope": {"name": batch.source, "version": "1.0.0"},
                        "logRecords": log_records,
                    }]
                })
            })
            .collect();

        json!({ "resourceLogs": resource_logs })
    }
}
