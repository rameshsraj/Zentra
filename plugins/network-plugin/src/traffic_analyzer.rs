use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::capture::{CaptureState, CaptureSnapshot};

/// Well-known port classification
#[derive(Debug, Clone, Serialize)]
pub struct ServiceClassification {
    pub port: u16,
    pub service: &'static str,
    pub category: &'static str,
}

/// Classify a port to a known service
pub fn classify_port(port: u16) -> ServiceClassification {
    match port {
        22 => ServiceClassification { port, service: "SSH", category: "remote_access" },
        23 => ServiceClassification { port, service: "Telnet", category: "remote_access" },
        25 => ServiceClassification { port, service: "SMTP", category: "email" },
        53 => ServiceClassification { port, service: "DNS", category: "dns" },
        80 => ServiceClassification { port, service: "HTTP", category: "web" },
        110 => ServiceClassification { port, service: "POP3", category: "email" },
        143 => ServiceClassification { port, service: "IMAP", category: "email" },
        443 => ServiceClassification { port, service: "HTTPS", category: "web" },
        445 => ServiceClassification { port, service: "SMB", category: "file_sharing" },
        993 => ServiceClassification { port, service: "IMAPS", category: "email" },
        995 => ServiceClassification { port, service: "POP3S", category: "email" },
        1433 => ServiceClassification { port, service: "MSSQL", category: "database" },
        1521 => ServiceClassification { port, service: "Oracle", category: "database" },
        3306 => ServiceClassification { port, service: "MySQL", category: "database" },
        3389 => ServiceClassification { port, service: "RDP", category: "remote_access" },
        5432 => ServiceClassification { port, service: "PostgreSQL", category: "database" },
        5900..=5910 => ServiceClassification { port, service: "VNC", category: "remote_access" },
        6379 => ServiceClassification { port, service: "Redis", category: "database" },
        8080 => ServiceClassification { port, service: "HTTP-Alt", category: "web" },
        8443 => ServiceClassification { port, service: "HTTPS-Alt", category: "web" },
        9200 => ServiceClassification { port, service: "Elasticsearch", category: "search" },
        27017 => ServiceClassification { port, service: "MongoDB", category: "database" },
        _ => ServiceClassification { port, service: "Unknown", category: "other" },
    }
}

/// Traffic anomaly detected by the analyzer
#[derive(Debug, Clone, Serialize)]
pub struct TrafficAnomaly {
    pub anomaly_type: AnomalyType,
    pub severity: &'static str,
    pub description: String,
    pub source_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub port: Option<u16>,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub enum AnomalyType {
    PortScan,
    HighBandwidth,
    UnusualProtocol,
    SuspiciousPort,
    ConnectionSpike,
}

/// Thresholds for anomaly detection
#[derive(Debug, Clone)]
pub struct AnomalyThresholds {
    /// Max unique destination ports from a single source before flagging as port scan
    pub port_scan_threshold: usize,
    /// Bytes per second threshold for high bandwidth alert
    pub high_bandwidth_bps: u64,
    /// Max new connections per second
    pub connection_spike_threshold: usize,
    /// Ports considered suspicious
    pub suspicious_ports: Vec<u16>,
}

impl Default for AnomalyThresholds {
    fn default() -> Self {
        Self {
            port_scan_threshold: 50,
            high_bandwidth_bps: 100_000_000, // 100 MB/s
            connection_spike_threshold: 500,
            suspicious_ports: vec![23, 445, 1080, 3128, 4444, 5555, 6666, 8888, 9999],
        }
    }
}

/// Analyze traffic patterns from capture state
pub struct TrafficAnalyzer {
    thresholds: AnomalyThresholds,
    prev_snapshot: Option<CaptureSnapshot>,
    prev_time: Option<std::time::Instant>,
}

impl TrafficAnalyzer {
    pub fn new(thresholds: AnomalyThresholds) -> Self {
        Self {
            thresholds,
            prev_snapshot: None,
            prev_time: None,
        }
    }

    /// Analyze the current capture state and return detected anomalies
    pub fn analyze(&mut self, state: &CaptureState) -> Vec<TrafficAnomaly> {
        let snapshot = state.snapshot();
        let now = std::time::Instant::now();
        let mut anomalies = Vec::new();

        // High bandwidth detection
        if snapshot.bytes_per_second > self.thresholds.high_bandwidth_bps as f64 {
            anomalies.push(TrafficAnomaly {
                anomaly_type: AnomalyType::HighBandwidth,
                severity: "warning",
                description: format!(
                    "High bandwidth detected: {:.1} MB/s",
                    snapshot.bytes_per_second / 1_000_000.0
                ),
                source_ip: None,
                dest_ip: None,
                port: None,
                detected_at: Utc::now(),
            });
        }

        // Connection spike detection
        if let (Some(ref prev), Some(prev_t)) = (&self.prev_snapshot, self.prev_time) {
            let dt = now.duration_since(prev_t).as_secs_f64().max(0.001);
            let new_conns = snapshot
                .active_connections
                .saturating_sub(prev.active_connections);
            let conns_per_sec = new_conns as f64 / dt;

            if conns_per_sec > self.thresholds.connection_spike_threshold as f64 {
                anomalies.push(TrafficAnomaly {
                    anomaly_type: AnomalyType::ConnectionSpike,
                    severity: "warning",
                    description: format!(
                        "Connection spike: {:.0} new connections/sec",
                        conns_per_sec
                    ),
                    source_ip: None,
                    dest_ip: None,
                    port: None,
                    detected_at: Utc::now(),
                });
            }
        }

        self.prev_snapshot = Some(snapshot);
        self.prev_time = Some(now);

        anomalies
    }

    /// Generate traffic breakdown by service category
    pub fn service_breakdown(state: &CaptureState) -> HashMap<String, u64> {
        let mut breakdown: HashMap<String, u64> = HashMap::new();

        for entry in state.connections_iter() {
            let stats = entry.value();
            let svc = classify_port(stats.dest_port);
            *breakdown.entry(svc.category.to_string()).or_default() +=
                stats.bytes_sent + stats.bytes_received;
        }

        breakdown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_port() {
        assert_eq!(classify_port(443).service, "HTTPS");
        assert_eq!(classify_port(22).category, "remote_access");
        assert_eq!(classify_port(5432).service, "PostgreSQL");
        assert_eq!(classify_port(12345).service, "Unknown");
    }

    #[test]
    fn test_anomaly_thresholds_default() {
        let t = AnomalyThresholds::default();
        assert_eq!(t.port_scan_threshold, 50);
        assert!(t.suspicious_ports.contains(&23));
    }
}
