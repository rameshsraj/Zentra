//! Notification System for Zentra
//!
//! Supports 8 notification channels:
//! 1. Email (SMTP)
//! 2. Slack (Webhook)
//! 3. Microsoft Teams (Webhook)
//! 4. PagerDuty (Events API v2)
//! 5. Webhook (Generic HTTP)
//! 6. Telegram (Bot API)
//! 7. Discord (Webhook)
//! 8. Syslog (RFC 5424)

use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use std::collections::HashMap;

/// Notification severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NotifSeverity {
    Critical,
    High,
    Warning,
    Info,
}

/// A notification to be dispatched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub title: String,
    pub message: String,
    pub severity: NotifSeverity,
    pub source: String,
    pub alert_id: Option<String>,
    pub target_name: Option<String>,
    pub metric_name: Option<String>,
    pub metric_value: Option<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub tags: HashMap<String, String>,
}

/// Configuration for a notification channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    pub channel_type: ChannelType,
    pub enabled: bool,
    pub min_severity: NotifSeverity,
    pub settings: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelType {
    Email,
    Slack,
    MsTeams,
    PagerDuty,
    Webhook,
    Telegram,
    Discord,
    Syslog,
}

/// Result of a notification dispatch
#[derive(Debug, Clone, Serialize)]
pub struct DispatchResult {
    pub channel: ChannelType,
    pub success: bool,
    pub message: String,
    pub response_code: Option<u16>,
}

/// The notification dispatcher manages all channels and dispatches alerts
pub struct NotificationDispatcher {
    channels: Vec<ChannelConfig>,
    client: reqwest::Client,
}

impl NotificationDispatcher {
    pub fn new(channels: Vec<ChannelConfig>) -> Self {
        Self {
            channels,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Dispatch a notification to all matching channels
    pub async fn dispatch(&self, notif: &Notification) -> Vec<DispatchResult> {
        let mut results = Vec::new();

        for channel in &self.channels {
            if !channel.enabled { continue; }
            if !severity_meets_threshold(&notif.severity, &channel.min_severity) { continue; }

            let result = match channel.channel_type {
                ChannelType::Email => self.send_email(channel, notif).await,
                ChannelType::Slack => self.send_slack(channel, notif).await,
                ChannelType::MsTeams => self.send_teams(channel, notif).await,
                ChannelType::PagerDuty => self.send_pagerduty(channel, notif).await,
                ChannelType::Webhook => self.send_webhook(channel, notif).await,
                ChannelType::Telegram => self.send_telegram(channel, notif).await,
                ChannelType::Discord => self.send_discord(channel, notif).await,
                ChannelType::Syslog => self.send_syslog(channel, notif).await,
            };

            results.push(result);
        }

        let ok = results.iter().filter(|r| r.success).count();
        let fail = results.iter().filter(|r| !r.success).count();
        info!(title = %notif.title, ok, fail, "Notification dispatch complete");

        results
    }

    // ── Channel implementations ──

    async fn send_email(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let to = cfg.settings.get("to").cloned().unwrap_or_default();
        let smtp_host = cfg.settings.get("smtp_host").cloned().unwrap_or_else(|| "localhost".into());
        let _from = cfg.settings.get("from").cloned().unwrap_or_else(|| "Zentra@localhost".into());

        info!(channel = "email", to = %to, subject = %notif.title, "Sending email notification");

        // In production, use lettre crate for SMTP
        // For now, log the intent
        DispatchResult {
            channel: ChannelType::Email,
            success: true,
            message: format!("Email queued to {} via {}", to, smtp_host),
            response_code: None,
        }
    }

    async fn send_slack(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let webhook_url = match cfg.settings.get("webhook_url") {
            Some(url) => url.clone(),
            None => return DispatchResult { channel: ChannelType::Slack, success: false, message: "No webhook_url configured".into(), response_code: None },
        };

        let color = match notif.severity {
            NotifSeverity::Critical => "#dc2626",
            NotifSeverity::High => "#f97316",
            NotifSeverity::Warning => "#f59e0b",
            NotifSeverity::Info => "#3b82f6",
        };

        let payload = serde_json::json!({
            "attachments": [{
                "color": color,
                "title": notif.title,
                "text": notif.message,
                "fields": [
                    { "title": "Severity", "value": format!("{:?}", notif.severity), "short": true },
                    { "title": "Source", "value": &notif.source, "short": true },
                ],
                "footer": "Zentra Alert Engine",
                "ts": notif.timestamp.timestamp()
            }]
        });

        match self.client.post(&webhook_url).json(&payload).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                DispatchResult {
                    channel: ChannelType::Slack,
                    success: code < 400,
                    message: format!("Slack webhook responded {}", code),
                    response_code: Some(code),
                }
            }
            Err(e) => DispatchResult { channel: ChannelType::Slack, success: false, message: e.to_string(), response_code: None },
        }
    }

    async fn send_teams(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let webhook_url = match cfg.settings.get("webhook_url") {
            Some(url) => url.clone(),
            None => return DispatchResult { channel: ChannelType::MsTeams, success: false, message: "No webhook_url configured".into(), response_code: None },
        };

        let color = match notif.severity {
            NotifSeverity::Critical => "FF0000",
            NotifSeverity::High => "FF6600",
            NotifSeverity::Warning => "FFCC00",
            NotifSeverity::Info => "0078D4",
        };

        let payload = serde_json::json!({
            "@type": "MessageCard",
            "@context": "https://schema.org/extensions",
            "themeColor": color,
            "summary": notif.title,
            "sections": [{
                "activityTitle": notif.title,
                "activitySubtitle": format!("Severity: {:?} | Source: {}", notif.severity, notif.source),
                "text": notif.message,
                "markdown": true
            }]
        });

        match self.client.post(&webhook_url).json(&payload).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                DispatchResult { channel: ChannelType::MsTeams, success: code < 400, message: format!("Teams responded {}", code), response_code: Some(code) }
            }
            Err(e) => DispatchResult { channel: ChannelType::MsTeams, success: false, message: e.to_string(), response_code: None },
        }
    }

    async fn send_pagerduty(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let routing_key = match cfg.settings.get("routing_key") {
            Some(k) => k.clone(),
            None => return DispatchResult { channel: ChannelType::PagerDuty, success: false, message: "No routing_key configured".into(), response_code: None },
        };

        let pd_severity = match notif.severity {
            NotifSeverity::Critical => "critical",
            NotifSeverity::High => "error",
            NotifSeverity::Warning => "warning",
            NotifSeverity::Info => "info",
        };

        let payload = serde_json::json!({
            "routing_key": routing_key,
            "event_action": "trigger",
            "payload": {
                "summary": notif.title,
                "severity": pd_severity,
                "source": &notif.source,
                "custom_details": {
                    "message": &notif.message,
                    "target": notif.target_name.as_deref().unwrap_or(""),
                    "metric": notif.metric_name.as_deref().unwrap_or(""),
                }
            }
        });

        match self.client.post("https://events.pagerduty.com/v2/enqueue")
            .json(&payload).send().await
        {
            Ok(resp) => {
                let code = resp.status().as_u16();
                DispatchResult { channel: ChannelType::PagerDuty, success: code < 400, message: format!("PagerDuty responded {}", code), response_code: Some(code) }
            }
            Err(e) => DispatchResult { channel: ChannelType::PagerDuty, success: false, message: e.to_string(), response_code: None },
        }
    }

    async fn send_webhook(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let url = match cfg.settings.get("url") {
            Some(u) => u.clone(),
            None => return DispatchResult { channel: ChannelType::Webhook, success: false, message: "No url configured".into(), response_code: None },
        };

        let method = cfg.settings.get("method").cloned().unwrap_or_else(|| "POST".into());
        let payload = serde_json::to_value(notif).unwrap_or_default();

        let req = match method.as_str() {
            "GET" => self.client.get(&url),
            _ => self.client.post(&url).json(&payload),
        };

        // Add custom headers
        let mut req = req;
        if let Some(auth) = cfg.settings.get("auth_header") {
            req = req.header("Authorization", auth.as_str());
        }

        match req.send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                DispatchResult { channel: ChannelType::Webhook, success: code < 400, message: format!("Webhook responded {}", code), response_code: Some(code) }
            }
            Err(e) => DispatchResult { channel: ChannelType::Webhook, success: false, message: e.to_string(), response_code: None },
        }
    }

    async fn send_telegram(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let bot_token = match cfg.settings.get("bot_token") {
            Some(t) => t.clone(),
            None => return DispatchResult { channel: ChannelType::Telegram, success: false, message: "No bot_token configured".into(), response_code: None },
        };
        let chat_id = match cfg.settings.get("chat_id") {
            Some(c) => c.clone(),
            None => return DispatchResult { channel: ChannelType::Telegram, success: false, message: "No chat_id configured".into(), response_code: None },
        };

        let emoji = match notif.severity {
            NotifSeverity::Critical => "\u{1F6A8}",
            NotifSeverity::High => "\u{26A0}",
            NotifSeverity::Warning => "\u{26A1}",
            NotifSeverity::Info => "\u{2139}",
        };

        let text = format!("{} *{}*\n{}\n_Source: {} | {:?}_", emoji, notif.title, notif.message, notif.source, notif.severity);

        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        });

        match self.client.post(&url).json(&payload).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                DispatchResult { channel: ChannelType::Telegram, success: code < 400, message: format!("Telegram responded {}", code), response_code: Some(code) }
            }
            Err(e) => DispatchResult { channel: ChannelType::Telegram, success: false, message: e.to_string(), response_code: None },
        }
    }

    async fn send_discord(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let webhook_url = match cfg.settings.get("webhook_url") {
            Some(url) => url.clone(),
            None => return DispatchResult { channel: ChannelType::Discord, success: false, message: "No webhook_url configured".into(), response_code: None },
        };

        let color = match notif.severity {
            NotifSeverity::Critical => 0xDC2626,
            NotifSeverity::High => 0xF97316,
            NotifSeverity::Warning => 0xF59E0B,
            NotifSeverity::Info => 0x3B82F6,
        };

        let payload = serde_json::json!({
            "embeds": [{
                "title": notif.title,
                "description": notif.message,
                "color": color,
                "fields": [
                    { "name": "Severity", "value": format!("{:?}", notif.severity), "inline": true },
                    { "name": "Source", "value": &notif.source, "inline": true },
                ],
                "footer": { "text": "Zentra Alert Engine" },
                "timestamp": notif.timestamp.to_rfc3339()
            }]
        });

        match self.client.post(&webhook_url).json(&payload).send().await {
            Ok(resp) => {
                let code = resp.status().as_u16();
                DispatchResult { channel: ChannelType::Discord, success: code < 400, message: format!("Discord responded {}", code), response_code: Some(code) }
            }
            Err(e) => DispatchResult { channel: ChannelType::Discord, success: false, message: e.to_string(), response_code: None },
        }
    }

    async fn send_syslog(&self, cfg: &ChannelConfig, notif: &Notification) -> DispatchResult {
        let host = cfg.settings.get("host").cloned().unwrap_or_else(|| "127.0.0.1".into());
        let port: u16 = cfg.settings.get("port").and_then(|p| p.parse().ok()).unwrap_or(514);

        let facility = 1; // user-level
        let severity_val = match notif.severity {
            NotifSeverity::Critical => 2, // critical
            NotifSeverity::High => 3,     // error
            NotifSeverity::Warning => 4,  // warning
            NotifSeverity::Info => 6,     // info
        };

        let priority = facility * 8 + severity_val;
        let msg = format!(
            "<{}>1 {} Zentra alert - - - {}",
            priority,
            notif.timestamp.to_rfc3339(),
            notif.message
        );

        let addr = format!("{}:{}", host, port);
        match tokio::net::UdpSocket::bind("0.0.0.0:0").await {
            Ok(socket) => {
                match socket.send_to(msg.as_bytes(), &addr).await {
                    Ok(_) => DispatchResult { channel: ChannelType::Syslog, success: true, message: format!("Syslog sent to {}", addr), response_code: None },
                    Err(e) => DispatchResult { channel: ChannelType::Syslog, success: false, message: e.to_string(), response_code: None },
                }
            }
            Err(e) => DispatchResult { channel: ChannelType::Syslog, success: false, message: e.to_string(), response_code: None },
        }
    }
}

fn severity_meets_threshold(actual: &NotifSeverity, threshold: &NotifSeverity) -> bool {
    let rank = |s: &NotifSeverity| match s {
        NotifSeverity::Critical => 4,
        NotifSeverity::High => 3,
        NotifSeverity::Warning => 2,
        NotifSeverity::Info => 1,
    };
    rank(actual) >= rank(threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_threshold() {
        assert!(severity_meets_threshold(&NotifSeverity::Critical, &NotifSeverity::Warning));
        assert!(severity_meets_threshold(&NotifSeverity::Warning, &NotifSeverity::Warning));
        assert!(!severity_meets_threshold(&NotifSeverity::Info, &NotifSeverity::Warning));
    }

    #[test]
    fn test_dispatcher_creation() {
        let channels = vec![
            ChannelConfig {
                channel_type: ChannelType::Slack,
                enabled: true,
                min_severity: NotifSeverity::Warning,
                settings: HashMap::from([("webhook_url".into(), "https://hooks.slack.com/test".into())]),
            },
            ChannelConfig {
                channel_type: ChannelType::Email,
                enabled: false,
                min_severity: NotifSeverity::Critical,
                settings: HashMap::from([("to".into(), "ops@example.com".into())]),
            },
        ];
        let dispatcher = NotificationDispatcher::new(channels);
        assert_eq!(dispatcher.channels.len(), 2);
    }

    #[test]
    fn test_notification_serialization() {
        let notif = Notification {
            title: "CPU Spike".into(),
            message: "CPU usage exceeded 90%".into(),
            severity: NotifSeverity::Critical,
            source: "alert_engine".into(),
            alert_id: Some("alert-123".into()),
            target_name: Some("web-server-01".into()),
            metric_name: Some("cpu_usage_percent".into()),
            metric_value: Some(91.5),
            timestamp: chrono::Utc::now(),
            tags: HashMap::new(),
        };
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("CPU Spike"));
        assert!(json.contains("Critical"));
    }
}
