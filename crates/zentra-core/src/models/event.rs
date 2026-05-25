use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Typed events flowing through the stream engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    MetricsCollected {
        plugin_id: String,
        target_id: Uuid,
        count: usize,
        timestamp: DateTime<Utc>,
    },
    LogsIngested {
        source: String,
        target_id: Uuid,
        count: usize,
        timestamp: DateTime<Utc>,
    },
    AlertTriggered {
        alert_id: Uuid,
        rule_id: Uuid,
        severity: String,
        timestamp: DateTime<Utc>,
    },
    AlertResolved {
        alert_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    TargetStatusChanged {
        target_id: Uuid,
        old_status: String,
        new_status: String,
        timestamp: DateTime<Utc>,
    },
    PluginStarted {
        plugin_id: String,
        timestamp: DateTime<Utc>,
    },
    PluginStopped {
        plugin_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    HealthCheck {
        component: String,
        healthy: bool,
        timestamp: DateTime<Utc>,
    },
    ThreatDetected {
        threat_id: Uuid,
        category: String,
        severity: String,
        source_ip: Option<String>,
        description: String,
        timestamp: DateTime<Utc>,
    },
    CaptureStats {
        total_packets: u64,
        total_bytes: u64,
        active_connections: usize,
        packets_per_sec: f64,
        timestamp: DateTime<Utc>,
    },
    AnomalyDetected {
        metric_name: String,
        resource_id: String,
        target_id: Uuid,
        z_score: f64,
        severity: String,
        timestamp: DateTime<Utc>,
    },
    TrendAlertGenerated {
        metric_name: String,
        resource_id: String,
        target_id: Uuid,
        hours_to_threshold: Option<f64>,
        severity: String,
        description: String,
        timestamp: DateTime<Utc>,
    },
}

impl Event {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::MetricsCollected { .. } => "metrics_collected",
            Self::LogsIngested { .. } => "logs_ingested",
            Self::AlertTriggered { .. } => "alert_triggered",
            Self::AlertResolved { .. } => "alert_resolved",
            Self::TargetStatusChanged { .. } => "target_status_changed",
            Self::PluginStarted { .. } => "plugin_started",
            Self::PluginStopped { .. } => "plugin_stopped",
            Self::HealthCheck { .. } => "health_check",
            Self::ThreatDetected { .. } => "threat_detected",
            Self::CaptureStats { .. } => "capture_stats",
            Self::AnomalyDetected { .. } => "anomaly_detected",
            Self::TrendAlertGenerated { .. } => "trend_alert_generated",
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::MetricsCollected { timestamp, .. }
            | Self::LogsIngested { timestamp, .. }
            | Self::AlertTriggered { timestamp, .. }
            | Self::AlertResolved { timestamp, .. }
            | Self::TargetStatusChanged { timestamp, .. }
            | Self::PluginStarted { timestamp, .. }
            | Self::PluginStopped { timestamp, .. }
            | Self::HealthCheck { timestamp, .. }
            | Self::ThreatDetected { timestamp, .. }
            | Self::CaptureStats { timestamp, .. }
            | Self::AnomalyDetected { timestamp, .. }
            | Self::TrendAlertGenerated { timestamp, .. } => *timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let e = Event::MetricsCollected {
            plugin_id: "test".into(),
            target_id: Uuid::new_v4(),
            count: 10,
            timestamp: Utc::now(),
        };
        assert_eq!(e.event_type(), "metrics_collected");
    }
}
