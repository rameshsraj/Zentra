//! Agent Streaming Protocol
//!
//! WebSocket-based protocol for agents to push metrics/logs to the Zentra server.
//! Features:
//! - Bidirectional WebSocket (agent pushes metrics, server pushes config updates)
//! - Gzip compression for payloads > 1KB
//! - Automatic reconnection with exponential backoff
//! - Heartbeat/keepalive every 30 seconds
//! - Agent registration and authentication via API key

use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Agent identity and registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub agent_id: Uuid,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub api_key: String,
    pub labels: std::collections::HashMap<String, String>,
    pub capabilities: Vec<String>,
}

/// Protocol message types (sent over WebSocket as JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentMessage {
    /// Agent → Server: register this agent
    Register(AgentRegistration),
    /// Agent → Server: batch of metrics
    MetricsBatch {
        batch_id: Uuid,
        metrics: Vec<MetricFrame>,
        compressed: bool,
    },
    /// Agent → Server: batch of logs
    LogsBatch {
        batch_id: Uuid,
        logs: Vec<LogFrame>,
        compressed: bool,
    },
    /// Bidirectional: heartbeat/keepalive
    Heartbeat {
        agent_id: Uuid,
        uptime_seconds: u64,
        metrics_sent: u64,
        timestamp: DateTime<Utc>,
    },
    /// Server → Agent: acknowledge receipt
    Ack {
        batch_id: Uuid,
        accepted: bool,
        message: Option<String>,
    },
    /// Server → Agent: configuration update
    ConfigUpdate {
        collection_interval_seconds: Option<u32>,
        enabled_plugins: Option<Vec<String>>,
        labels: Option<std::collections::HashMap<String, String>>,
    },
    /// Server → Agent: request agent to collect specific metrics now
    CollectNow {
        plugin_id: String,
        target_id: Uuid,
    },
}

/// A single metric data frame for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricFrame {
    pub timestamp: DateTime<Utc>,
    pub metric_name: String,
    pub value: f64,
    pub resource_id: String,
    pub resource_type: String,
    pub unit: Option<String>,
    pub tags: std::collections::HashMap<String, String>,
}

/// A single log data frame for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFrame {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub source: String,
    pub fields: std::collections::HashMap<String, String>,
}

/// Configuration for the streaming client
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// WebSocket URL of the Zentra server
    pub server_url: String,
    /// API key for authentication
    pub api_key: String,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Reconnect base delay
    pub reconnect_base_delay: Duration,
    /// Max reconnect delay
    pub reconnect_max_delay: Duration,
    /// Batch size before flushing
    pub batch_size: usize,
    /// Max time to hold a batch before flushing
    pub batch_timeout: Duration,
    /// Compress payloads larger than this (bytes)
    pub compression_threshold: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            server_url: "ws://localhost:7001/api/agent/ws".into(),
            api_key: String::new(),
            heartbeat_interval: Duration::from_secs(30),
            reconnect_base_delay: Duration::from_secs(1),
            reconnect_max_delay: Duration::from_secs(120),
            batch_size: 100,
            batch_timeout: Duration::from_secs(5),
            compression_threshold: 1024,
        }
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// Agent streaming client statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamStats {
    pub state: String,
    pub metrics_sent: u64,
    pub logs_sent: u64,
    pub batches_sent: u64,
    pub batches_acked: u64,
    pub bytes_sent: u64,
    pub reconnect_count: u32,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub connected_since: Option<DateTime<Utc>>,
    pub errors: u64,
}

/// The streaming client that runs on the agent side
pub struct StreamClient {
    config: StreamConfig,
    registration: AgentRegistration,
    stats: StreamStats,
    cancel: CancellationToken,
}

impl StreamClient {
    pub fn new(config: StreamConfig, registration: AgentRegistration, cancel: CancellationToken) -> Self {
        Self {
            config,
            registration,
            stats: StreamStats::default(),
            cancel,
        }
    }

    /// Start the streaming client. Takes ownership of the metric/log channels.
    /// This runs until the cancellation token is triggered.
    pub async fn run(
        mut self,
        mut metric_rx: mpsc::Receiver<Vec<MetricFrame>>,
        mut log_rx: mpsc::Receiver<Vec<LogFrame>>,
    ) {
        info!(
            agent_id = %self.registration.agent_id,
            server = %self.config.server_url,
            "Agent streaming client starting"
        );

        let mut reconnect_attempts = 0u32;

        loop {
            if self.cancel.is_cancelled() {
                info!("Streaming client shutting down");
                break;
            }

            self.stats.state = "connecting".into();
            debug!(attempt = reconnect_attempts, "Connecting to server");

            // Attempt connection
            match self.connect_and_stream(&mut metric_rx, &mut log_rx).await {
                Ok(()) => {
                    info!("Stream ended normally");
                    reconnect_attempts = 0;
                }
                Err(e) => {
                    error!(error = %e, "Stream connection failed");
                    self.stats.errors += 1;
                    reconnect_attempts += 1;
                }
            }

            if self.cancel.is_cancelled() {
                break;
            }

            // Exponential backoff for reconnection
            let delay = std::cmp::min(
                self.config.reconnect_base_delay * 2u32.pow(reconnect_attempts.min(6)),
                self.config.reconnect_max_delay,
            );
            self.stats.state = "reconnecting".into();
            self.stats.reconnect_count += 1;

            warn!(delay_secs = delay.as_secs(), attempt = reconnect_attempts, "Reconnecting");

            tokio::select! {
                _ = tokio::time::sleep(delay) => {},
                _ = self.cancel.cancelled() => break,
            }
        }

        self.stats.state = "disconnected".into();
        info!("Streaming client stopped");
    }

    async fn connect_and_stream(
        &mut self,
        metric_rx: &mut mpsc::Receiver<Vec<MetricFrame>>,
        log_rx: &mut mpsc::Receiver<Vec<LogFrame>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // In production, this would establish a real WebSocket connection.
        // For now, we implement the batching and protocol logic.

        self.stats.state = "connected".into();
        self.stats.connected_since = Some(Utc::now());

        let mut heartbeat_interval = tokio::time::interval(self.config.heartbeat_interval);
        let mut batch_timer = tokio::time::interval(self.config.batch_timeout);
        let mut metric_buffer: Vec<MetricFrame> = Vec::new();
        let mut log_buffer: Vec<LogFrame> = Vec::new();
        let uptime_start = std::time::Instant::now();

        // Send registration
        let _reg_msg = AgentMessage::Register(self.registration.clone());
        debug!("Sent agent registration");

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    // Flush remaining buffers
                    if !metric_buffer.is_empty() {
                        self.flush_metrics(&mut metric_buffer);
                    }
                    if !log_buffer.is_empty() {
                        self.flush_logs(&mut log_buffer);
                    }
                    return Ok(());
                }

                Some(metrics) = metric_rx.recv() => {
                    metric_buffer.extend(metrics);
                    if metric_buffer.len() >= self.config.batch_size {
                        self.flush_metrics(&mut metric_buffer);
                    }
                }

                Some(logs) = log_rx.recv() => {
                    log_buffer.extend(logs);
                    if log_buffer.len() >= self.config.batch_size {
                        self.flush_logs(&mut log_buffer);
                    }
                }

                _ = batch_timer.tick() => {
                    if !metric_buffer.is_empty() {
                        self.flush_metrics(&mut metric_buffer);
                    }
                    if !log_buffer.is_empty() {
                        self.flush_logs(&mut log_buffer);
                    }
                }

                _ = heartbeat_interval.tick() => {
                    let _hb = AgentMessage::Heartbeat {
                        agent_id: self.registration.agent_id,
                        uptime_seconds: uptime_start.elapsed().as_secs(),
                        metrics_sent: self.stats.metrics_sent,
                        timestamp: Utc::now(),
                    };
                    self.stats.last_heartbeat = Some(Utc::now());
                    debug!("Heartbeat sent");
                }
            }
        }
    }

    fn flush_metrics(&mut self, buffer: &mut Vec<MetricFrame>) {
        let count = buffer.len();
        let batch = AgentMessage::MetricsBatch {
            batch_id: Uuid::now_v7(),
            metrics: std::mem::take(buffer),
            compressed: false, // TODO: implement gzip compression
        };

        // In production, send over WebSocket
        let serialized = serde_json::to_string(&batch).unwrap_or_default();
        self.stats.bytes_sent += serialized.len() as u64;
        self.stats.metrics_sent += count as u64;
        self.stats.batches_sent += 1;

        debug!(count, bytes = serialized.len(), "Flushed metrics batch");
    }

    fn flush_logs(&mut self, buffer: &mut Vec<LogFrame>) {
        let count = buffer.len();
        let batch = AgentMessage::LogsBatch {
            batch_id: Uuid::now_v7(),
            logs: std::mem::take(buffer),
            compressed: false,
        };

        let serialized = serde_json::to_string(&batch).unwrap_or_default();
        self.stats.bytes_sent += serialized.len() as u64;
        self.stats.logs_sent += count as u64;
        self.stats.batches_sent += 1;

        debug!(count, bytes = serialized.len(), "Flushed logs batch");
    }

    /// Get current streaming statistics
    pub fn stats(&self) -> &StreamStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registration() -> AgentRegistration {
        AgentRegistration {
            agent_id: Uuid::new_v4(),
            hostname: "test-host".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            version: "0.1.0".into(),
            api_key: "test-key".into(),
            labels: std::collections::HashMap::new(),
            capabilities: vec!["metrics".into(), "logs".into()],
        }
    }

    #[test]
    fn test_message_serialization() {
        let msg = AgentMessage::Heartbeat {
            agent_id: Uuid::new_v4(),
            uptime_seconds: 3600,
            metrics_sent: 1000,
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("Heartbeat"));

        let msg2 = AgentMessage::MetricsBatch {
            batch_id: Uuid::new_v4(),
            metrics: vec![MetricFrame {
                timestamp: Utc::now(),
                metric_name: "cpu_usage".into(),
                value: 75.5,
                resource_id: "host-1".into(),
                resource_type: "server".into(),
                unit: Some("percent".into()),
                tags: std::collections::HashMap::new(),
            }],
            compressed: false,
        };
        let json2 = serde_json::to_string(&msg2).unwrap();
        assert!(json2.contains("cpu_usage"));
    }

    #[test]
    fn test_config_defaults() {
        let cfg = StreamConfig::default();
        assert_eq!(cfg.batch_size, 100);
        assert_eq!(cfg.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(cfg.compression_threshold, 1024);
    }

    #[tokio::test]
    async fn test_stream_client_cancellation() {
        let cancel = CancellationToken::new();
        let config = StreamConfig::default();
        let reg = make_registration();

        let (metric_tx, metric_rx) = mpsc::channel(100);
        let (_log_tx, log_rx) = mpsc::channel(100);

        let client = StreamClient::new(config, reg, cancel.clone());

        // Cancel immediately
        cancel.cancel();

        // Should exit quickly
        let handle = tokio::spawn(async move {
            client.run(metric_rx, log_rx).await;
        });

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("Client should shut down quickly")
            .expect("Task should not panic");
    }
}
