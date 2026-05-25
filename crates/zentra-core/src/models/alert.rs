use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    Pending,
    Triggered,
    Acknowledged,
    Resolved,
    Suppressed,
}

/// Alert rule definition — what triggers an alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub rule_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub window_seconds: u32,
    pub severity: AlertSeverity,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Seconds the condition must hold before transitioning from Pending → Triggered
    #[serde(default)]
    pub for_duration_seconds: u32,
    /// Recovery threshold for hysteresis (prevents flapping).
    /// If set, the alert only resolves when the metric crosses back past this value.
    #[serde(default)]
    pub hysteresis_threshold: Option<f64>,
    /// Seconds between repeated notifications while alert remains triggered
    #[serde(default = "default_repeat_interval")]
    pub repeat_interval_seconds: u32,
    /// Optional notification policy ID for routing
    #[serde(default)]
    pub notification_policy_id: Option<Uuid>,
}

fn default_repeat_interval() -> u32 {
    3600
}

/// Condition for alert evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertCondition {
    pub operator: ComparisonOp,
    pub threshold: f64,
    pub aggregation: Aggregation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOp {
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    Avg,
    Max,
    Min,
    Sum,
    Count,
    Last,
    P95,
    P99,
}

/// An active or historical alert instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub alert_id: Uuid,
    pub rule_id: Uuid,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub resource_id: String,
    pub severity: AlertSeverity,
    pub status: AlertStatus,
    pub message: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub triggered_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
    /// When the alert first entered Pending state
    #[serde(default)]
    pub pending_since: Option<DateTime<Utc>>,
    /// Last time a notification was sent for this alert
    #[serde(default)]
    pub last_notified_at: Option<DateTime<Utc>>,
    /// How many notifications have been sent
    #[serde(default)]
    pub notification_count: u32,
}

impl Alert {
    /// Create a new alert in Pending state (if for_duration > 0) or Triggered (if 0)
    pub fn trigger(
        rule: &AlertRule,
        target_id: Uuid,
        resource_id: impl Into<String>,
        metric_value: f64,
        message: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let (status, pending_since) = if rule.for_duration_seconds > 0 {
            (AlertStatus::Pending, Some(now))
        } else {
            (AlertStatus::Triggered, None)
        };
        Self {
            alert_id: Uuid::now_v7(),
            rule_id: rule.rule_id,
            org_id: rule.org_id,
            target_id,
            resource_id: resource_id.into(),
            severity: rule.severity,
            status,
            message: message.into(),
            metric_value,
            threshold: rule.condition.threshold,
            triggered_at: now,
            acknowledged_at: None,
            acknowledged_by: None,
            resolved_at: None,
            metadata: None,
            pending_since,
            last_notified_at: None,
            notification_count: 0,
        }
    }

    /// Promote from Pending → Triggered (after for_duration has elapsed)
    pub fn promote_to_triggered(&mut self) {
        self.status = AlertStatus::Triggered;
        self.triggered_at = Utc::now();
    }

    /// Check if this alert's for_duration has elapsed and should be promoted
    pub fn should_promote(&self, for_duration_seconds: u32) -> bool {
        if self.status != AlertStatus::Pending {
            return false;
        }
        if let Some(pending) = self.pending_since {
            let elapsed = Utc::now().signed_duration_since(pending).num_seconds();
            elapsed >= for_duration_seconds as i64
        } else {
            false
        }
    }

    /// Check if a repeat notification should be sent
    pub fn should_repeat_notify(&self, repeat_interval_seconds: u32) -> bool {
        if self.status != AlertStatus::Triggered {
            return false;
        }
        if let Some(last) = self.last_notified_at {
            let elapsed = Utc::now().signed_duration_since(last).num_seconds();
            elapsed >= repeat_interval_seconds as i64
        } else {
            true // never notified yet
        }
    }

    /// Record that a notification was sent
    pub fn mark_notified(&mut self) {
        self.last_notified_at = Some(Utc::now());
        self.notification_count += 1;
    }

    pub fn acknowledge(&mut self, by: impl Into<String>) {
        self.status = AlertStatus::Acknowledged;
        self.acknowledged_at = Some(Utc::now());
        self.acknowledged_by = Some(by.into());
    }

    pub fn resolve(&mut self) {
        self.status = AlertStatus::Resolved;
        self.resolved_at = Some(Utc::now());
    }
}

/// Routing rule — where to send alerts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRoutingRule {
    pub routing_id: Uuid,
    pub rule_id: Uuid,
    pub org_id: Uuid,
    pub channel: AlertChannel,
    pub config: serde_json::Value,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlertChannel {
    Email,
    Slack,
    Webhook,
    PagerDuty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_lifecycle() {
        let rule = AlertRule {
            rule_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: "High CPU".into(),
            description: None,
            metric_name: "cpu_usage".into(),
            condition: AlertCondition {
                operator: ComparisonOp::GreaterThan,
                threshold: 90.0,
                aggregation: Aggregation::Avg,
            },
            window_seconds: 300,
            severity: AlertSeverity::Critical,
            enabled: true,
            tags: vec!["infra".into()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            for_duration_seconds: 0,
            hysteresis_threshold: None,
            repeat_interval_seconds: 3600,
            notification_policy_id: None,
        };

        let mut alert = Alert::trigger(&rule, Uuid::new_v4(), "host-1", 95.0, "CPU at 95%");
        assert_eq!(alert.status, AlertStatus::Triggered);

        alert.acknowledge("admin");
        assert_eq!(alert.status, AlertStatus::Acknowledged);
        assert!(alert.acknowledged_at.is_some());

        alert.resolve();
        assert_eq!(alert.status, AlertStatus::Resolved);
        assert!(alert.resolved_at.is_some());
    }
}
