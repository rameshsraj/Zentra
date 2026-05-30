//! Notification Router — bridges AlertEngine events to NotificationDispatcher.
//!
//! Subscribes to the EventBus, matches events against notification policies,
//! and dispatches notifications through the appropriate channels.
//! Supports:
//! - Policy-based routing (match by severity, tags, target type)
//! - Group wait (debounce initial notification)
//! - Repeat interval tracking
//! - Mute timings (cron-based silence windows)

use std::collections::HashMap;
use chrono::{DateTime, Utc, Datelike, Timelike};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use zentra_core::models::event::Event;
use crate::notifications::{
    ChannelConfig, ChannelType, Notification, NotifSeverity, NotificationDispatcher,
};

/// A notification routing policy (mirrors the DB table)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPolicy {
    pub policy_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    /// Severities to match (empty = match all)
    pub match_severity: Vec<String>,
    /// Tags to match (empty = match all)
    pub match_tags: Vec<String>,
    /// Target type to match (None = match all)
    pub match_target_type: Option<String>,
    /// Channel to route to
    pub channel_type: String,
    /// Channel-specific configuration
    pub channel_config: serde_json::Value,
    pub enabled: bool,
    /// Seconds to wait before first notification in a group
    pub group_wait: u32,
    /// Seconds between notifications for the same alert group
    pub group_interval: u32,
    /// Seconds before repeating a notification for the same alert
    pub repeat_interval: u32,
    /// Mute window: days of week (0=Sun..6=Sat)
    pub mute_days: Vec<u8>,
    /// Mute window: start hour (0-23)
    pub mute_start_hour: Option<u8>,
    /// Mute window: end hour (0-23)
    pub mute_end_hour: Option<u8>,
}

impl NotificationPolicy {
    /// Check if a notification policy matches the given alert properties
    pub fn matches(&self, severity: &str, tags: &[String], target_type: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }

        // Check severity
        if !self.match_severity.is_empty()
            && !self.match_severity.iter().any(|s| s == severity)
        {
            return false;
        }

        // Check tags
        if !self.match_tags.is_empty()
            && !self.match_tags.iter().any(|t| tags.contains(t))
        {
            return false;
        }

        // Check target type
        if let Some(ref required) = self.match_target_type {
            if let Some(actual) = target_type {
                if required != actual {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }

    /// Check if currently inside a mute window
    pub fn is_muted(&self) -> bool {
        let now = Utc::now();
        let weekday = now.weekday().num_days_from_sunday() as u8;

        // Check if today is a muted day
        if !self.mute_days.is_empty() && !self.mute_days.contains(&weekday) {
            return false;
        }

        // Check hour range
        if let (Some(start), Some(end)) = (self.mute_start_hour, self.mute_end_hour) {
            let hour = now.hour() as u8;
            if start <= end {
                // Same-day range: e.g., 22:00 to 06:00 wraps
                return hour >= start && hour < end;
            } else {
                // Overnight range: e.g., 22:00 to 06:00
                return hour >= start || hour < end;
            }
        }

        // If mute_days set but no hours, mute the entire day
        !self.mute_days.is_empty()
    }

    /// Convert channel_type string to ChannelType enum
    pub fn to_channel_type(&self) -> Option<ChannelType> {
        match self.channel_type.to_lowercase().as_str() {
            "email" => Some(ChannelType::Email),
            "slack" => Some(ChannelType::Slack),
            "teams" | "msteams" => Some(ChannelType::MsTeams),
            "pagerduty" => Some(ChannelType::PagerDuty),
            "webhook" => Some(ChannelType::Webhook),
            "telegram" => Some(ChannelType::Telegram),
            "discord" => Some(ChannelType::Discord),
            "syslog" => Some(ChannelType::Syslog),
            _ => None,
        }
    }

    /// Build a ChannelConfig from this policy
    pub fn to_channel_config(&self) -> Option<ChannelConfig> {
        let ct = self.to_channel_type()?;
        let settings: HashMap<String, String> =
            serde_json::from_value(self.channel_config.clone()).unwrap_or_default();
        Some(ChannelConfig {
            channel_type: ct,
            enabled: true,
            min_severity: NotifSeverity::Info,
            settings,
        })
    }
}

/// Tracks per-alert group notification state
#[derive(Debug, Clone)]
struct GroupState {
    first_seen: DateTime<Utc>,
    last_notified: Option<DateTime<Utc>>,
    notification_count: u32,
}

/// The NotificationRouter subscribes to the EventBus and dispatches
/// notifications based on routing policies.
pub struct NotificationRouter {
    policies: Vec<NotificationPolicy>,
    dispatcher: NotificationDispatcher,
    /// Track per (alert_id, policy_id) notification state
    group_states: HashMap<(Uuid, Uuid), GroupState>,
}

impl NotificationRouter {
    pub fn new(policies: Vec<NotificationPolicy>) -> Self {
        // Build a dispatcher with no default channels — policies carry their own config
        let dispatcher = NotificationDispatcher::new(Vec::new());
        Self {
            policies,
            dispatcher,
            group_states: HashMap::new(),
        }
    }

    /// Update routing policies (hot-reload from DB)
    pub fn load_policies(&mut self, policies: Vec<NotificationPolicy>) {
        info!(count = policies.len(), "Notification policies reloaded");
        self.policies = policies;
    }

    /// Start listening to the event bus and routing notifications.
    /// This spawns a background task.
    pub fn start(mut self, mut rx: broadcast::Receiver<Event>) -> tokio::task::JoinHandle<()> {
        // Item 1: Propagate trace context into spawned task
        use tracing::Instrument;
        let span = tracing::info_span!("notification_router");
        tokio::spawn(async move {
            info!("Notification router started");
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        self.handle_event(&event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Notification router lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Notification router stopped (bus closed)");
                        break;
                    }
                }
            }
        }.instrument(span))
    }

    async fn handle_event(&mut self, event: &Event) {
        match event {
            Event::AlertTriggered {
                alert_id,
                rule_id,
                severity,
                timestamp,
            } => {
                self.route_alert_notification(
                    *alert_id,
                    *rule_id,
                    severity,
                    &format!("Alert triggered (rule {})", rule_id),
                    *timestamp,
                )
                .await;
            }
            Event::AnomalyDetected {
                metric_name,
                resource_id,
                target_id,
                z_score,
                severity,
                timestamp,
            } => {
                let notif = Notification {
                    title: format!("Anomaly: {} on {}", metric_name, resource_id),
                    message: format!(
                        "Anomaly detected on metric '{}' (z-score: {:.2}) for resource '{}'",
                        metric_name, z_score, resource_id
                    ),
                    severity: str_to_severity(severity),
                    source: "anomaly_detector".into(),
                    alert_id: None,
                    target_name: Some(target_id.to_string()),
                    metric_name: Some(metric_name.clone()),
                    metric_value: Some(*z_score),
                    timestamp: *timestamp,
                    tags: HashMap::new(),
                };

                // Route through matching policies
                for policy in &self.policies {
                    if !policy.enabled || policy.is_muted() {
                        continue;
                    }
                    if policy.matches(severity, &[], None) {
                        if let Some(cfg) = policy.to_channel_config() {
                            let temp_dispatcher =
                                NotificationDispatcher::new(vec![cfg]);
                            let results = temp_dispatcher.dispatch(&notif).await;
                            for r in &results {
                                if !r.success {
                                    warn!(
                                        channel = ?r.channel,
                                        error = %r.message,
                                        "Anomaly notification failed"
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Event::TrendAlertGenerated {
                metric_name,
                resource_id,
                severity,
                description,
                timestamp,
                ..
            } => {
                let notif = Notification {
                    title: format!("Trend Alert: {}", metric_name),
                    message: description.clone(),
                    severity: str_to_severity(severity),
                    source: "trend_alert_engine".into(),
                    alert_id: None,
                    target_name: None,
                    metric_name: Some(metric_name.clone()),
                    metric_value: None,
                    timestamp: *timestamp,
                    tags: HashMap::new(),
                };

                for policy in &self.policies {
                    if !policy.enabled || policy.is_muted() {
                        continue;
                    }
                    if policy.matches(severity, &[], None) {
                        if let Some(cfg) = policy.to_channel_config() {
                            let temp_dispatcher =
                                NotificationDispatcher::new(vec![cfg]);
                            temp_dispatcher.dispatch(&notif).await;
                        }
                    }
                }
            }
            _ => {
                // Other events don't trigger notifications
            }
        }
    }

    async fn route_alert_notification(
        &mut self,
        alert_id: Uuid,
        rule_id: Uuid,
        severity: &str,
        message: &str,
        timestamp: DateTime<Utc>,
    ) {
        let notif = Notification {
            title: format!("Alert: {}", alert_id),
            message: message.to_string(),
            severity: str_to_severity(severity),
            source: "alert_engine".into(),
            alert_id: Some(alert_id.to_string()),
            target_name: None,
            metric_name: None,
            metric_value: None,
            timestamp,
            tags: HashMap::new(),
        };

        let mut dispatched = false;

        for policy in &self.policies {
            if !policy.enabled || policy.is_muted() {
                continue;
            }

            if !policy.matches(severity, &[], None) {
                continue;
            }

            let key = (alert_id, policy.policy_id);
            let state = self.group_states.entry(key).or_insert(GroupState {
                first_seen: timestamp,
                last_notified: None,
                notification_count: 0,
            });

            // Group wait: don't notify until group_wait seconds after first seen
            let elapsed_since_first = Utc::now()
                .signed_duration_since(state.first_seen)
                .num_seconds();
            if elapsed_since_first < policy.group_wait as i64 {
                debug!(
                    policy = %policy.name,
                    alert = %alert_id,
                    "Waiting group_wait ({} secs, {} elapsed)",
                    policy.group_wait,
                    elapsed_since_first,
                );
                continue;
            }

            // Group interval: don't send more often than group_interval
            if let Some(last) = state.last_notified {
                let elapsed = Utc::now().signed_duration_since(last).num_seconds();
                if elapsed < policy.group_interval as i64 {
                    debug!(
                        policy = %policy.name,
                        "Throttled by group_interval ({} secs)",
                        policy.group_interval
                    );
                    continue;
                }
            }

            // Dispatch
            if let Some(cfg) = policy.to_channel_config() {
                let temp_dispatcher = NotificationDispatcher::new(vec![cfg]);
                let results = temp_dispatcher.dispatch(&notif).await;

                state.last_notified = Some(Utc::now());
                state.notification_count += 1;
                dispatched = true;

                for r in &results {
                    if r.success {
                        info!(
                            policy = %policy.name,
                            channel = ?r.channel,
                            alert = %alert_id,
                            "Notification dispatched"
                        );
                    } else {
                        error!(
                            policy = %policy.name,
                            channel = ?r.channel,
                            error = %r.message,
                            "Notification dispatch failed"
                        );
                    }
                }
            }
        }

        if !dispatched {
            debug!(alert = %alert_id, "No matching notification policies");
        }
    }
}

fn str_to_severity(s: &str) -> NotifSeverity {
    match s.to_lowercase().as_str() {
        "critical" => NotifSeverity::Critical,
        "high" => NotifSeverity::High,
        "warning" => NotifSeverity::Warning,
        _ => NotifSeverity::Info,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_policy(name: &str, channel: &str, severity: Vec<&str>) -> NotificationPolicy {
        NotificationPolicy {
            policy_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            match_severity: severity.into_iter().map(|s| s.into()).collect(),
            match_tags: vec![],
            match_target_type: None,
            channel_type: channel.into(),
            channel_config: serde_json::json!({"webhook_url": "https://hooks.slack.com/test"}),
            enabled: true,
            group_wait: 0,
            group_interval: 0,
            repeat_interval: 3600,
            mute_days: vec![],
            mute_start_hour: None,
            mute_end_hour: None,
        }
    }

    #[test]
    fn test_policy_matches_severity() {
        let policy = make_policy("critical-only", "slack", vec!["critical"]);
        assert!(policy.matches("critical", &[], None));
        assert!(!policy.matches("warning", &[], None));
        assert!(!policy.matches("info", &[], None));
    }

    #[test]
    fn test_policy_matches_all_severities() {
        let policy = make_policy("all-alerts", "slack", vec![]);
        assert!(policy.matches("critical", &[], None));
        assert!(policy.matches("warning", &[], None));
        assert!(policy.matches("info", &[], None));
    }

    #[test]
    fn test_mute_not_active_when_no_config() {
        let policy = make_policy("no-mute", "slack", vec![]);
        assert!(!policy.is_muted());
    }

    #[test]
    fn test_disabled_policy_no_match() {
        let mut policy = make_policy("disabled", "slack", vec![]);
        policy.enabled = false;
        assert!(!policy.matches("critical", &[], None));
    }

    #[test]
    fn test_channel_type_parsing() {
        let policy = make_policy("slack", "slack", vec![]);
        assert_eq!(policy.to_channel_type(), Some(ChannelType::Slack));

        let policy2 = make_policy("teams", "teams", vec![]);
        assert_eq!(policy2.to_channel_type(), Some(ChannelType::MsTeams));

        let policy3 = make_policy("bad", "unknown_channel", vec![]);
        assert_eq!(policy3.to_channel_type(), None);
    }
}
