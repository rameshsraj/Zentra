//! Webhook Event Dispatcher
//!
//! Broadcasts lifecycle events (alerts, license changes, machine heartbeats, etc.)
//! to configured external webhook endpoints.
//!
//! Reference: Keygen `app/services/broadcast_webhook_service.rb`

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use uuid::Uuid;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn compute_hmac_signature(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take any key size");
    mac.update(body);
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub endpoint_id: Uuid,
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,     // e.g. ["alert.triggered", "license.expired", "*"]
    pub enabled: bool,
    pub retry_count: u32,
    pub timeout_seconds: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub endpoint_id: Uuid,
    pub status: WebhookStatus,
    pub attempt: u32,
    pub response_code: Option<u16>,
    pub response_body: Option<String>,
    pub last_attempted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WebhookStatus {
    Pending,
    Delivered,
    Failed,
    Retrying,
}

/// Supported event types
pub const EVENT_TYPES: &[&str] = &[
    "alert.triggered",
    "alert.resolved",
    "alert.acknowledged",
    "license.created",
    "license.expired",
    "license.renewed",
    "license.seat.checkout",
    "license.seat.checkin",
    "machine.heartbeat.dead",
    "machine.heartbeat.stale",
    "target.up",
    "target.down",
    "anomaly.detected",
    "metric.threshold.exceeded",
];

// ═══════════════════════════════════════════════════════════════════════════
// Webhook Dispatcher
// ═══════════════════════════════════════════════════════════════════════════

pub struct WebhookDispatcher {
    endpoints: Arc<RwLock<Vec<WebhookEndpoint>>>,
    event_log: Arc<RwLock<Vec<WebhookEvent>>>,
    max_retries: u32,
}

impl WebhookDispatcher {
    pub fn new() -> Self {
        Self {
            endpoints: Arc::new(RwLock::new(Vec::new())),
            event_log: Arc::new(RwLock::new(Vec::new())),
            max_retries: 3,
        }
    }

    /// Register a webhook endpoint
    pub async fn register_endpoint(&self, endpoint: WebhookEndpoint) {
        info!(url = %endpoint.url, events = ?endpoint.events, "Registering webhook endpoint");
        self.endpoints.write().await.push(endpoint);
    }

    /// Broadcast an event to all matching endpoints
    pub async fn broadcast(&self, event_type: &str, payload: serde_json::Value) {
        let endpoints = self.endpoints.read().await;
        let matching: Vec<_> = endpoints.iter()
            .filter(|ep| ep.enabled && (ep.events.contains(&"*".to_string()) || ep.events.contains(&event_type.to_string())))
            .cloned()
            .collect();
        drop(endpoints);

        if matching.is_empty() {
            return;
        }

        info!(
            event_type = %event_type,
            endpoints = matching.len(),
            "Broadcasting webhook event"
        );

        for endpoint in matching {
            let event = WebhookEvent {
                event_id: Uuid::now_v7(),
                event_type: event_type.to_string(),
                payload: payload.clone(),
                endpoint_id: endpoint.endpoint_id,
                status: WebhookStatus::Pending,
                attempt: 0,
                response_code: None,
                response_body: None,
                last_attempted_at: None,
                created_at: Utc::now(),
            };

            self.deliver(endpoint, event).await;
        }
    }

    async fn deliver(&self, endpoint: WebhookEndpoint, mut event: WebhookEvent) {
        for attempt in 1..=self.max_retries {
            event.attempt = attempt;
            event.last_attempted_at = Some(Utc::now());
            event.status = if attempt > 1 { WebhookStatus::Retrying } else { WebhookStatus::Pending };

            // Build the webhook payload
            let body = serde_json::json!({
                "event_id": event.event_id,
                "event_type": event.event_type,
                "timestamp": event.created_at,
                "data": event.payload,
            });

            let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
            let signature = endpoint.secret.as_deref()
                .map(|s| compute_hmac_signature(s, &body_bytes));

            let mut req = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(endpoint.timeout_seconds))
                .build()
                .unwrap_or_default()
                .post(&endpoint.url)
                .header("Content-Type", "application/json")
                .header("X-Zentra-Event", &event.event_type)
                .header("X-Zentra-Delivery", event.event_id.to_string())
                .body(body_bytes.clone());

            if let Some(sig) = signature {
                req = req.header("X-Zentra-Signature", sig);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let resp_body = resp.text().await.ok();
                    event.response_code = Some(status);
                    event.response_body = resp_body;

                    if status < 300 {
                        event.status = WebhookStatus::Delivered;
                        info!(
                            event_id = %event.event_id,
                            endpoint = %endpoint.url,
                            attempt = attempt,
                            status = status,
                            "Webhook delivered"
                        );
                        break;
                    } else {
                        event.status = WebhookStatus::Failed;
                        warn!(
                            event_id = %event.event_id,
                            endpoint = %endpoint.url,
                            attempt = attempt,
                            status = status,
                            "Webhook returned non-2xx, will retry"
                        );
                    }
                }
                Err(e) => {
                    event.status = WebhookStatus::Failed;
                    warn!(
                        event_id = %event.event_id,
                        endpoint = %endpoint.url,
                        attempt = attempt,
                        error = %e,
                        "Webhook delivery error, will retry"
                    );
                }
            }

            // Exponential backoff: 2s, 4s, 8s, 16s, ...
            let delay = std::time::Duration::from_secs(2u64.pow(attempt.min(6)));
            tokio::time::sleep(delay).await;
        }

        // Log the event
        self.event_log.write().await.push(event);
    }

    /// List registered endpoints
    pub async fn list_endpoints(&self) -> Vec<WebhookEndpoint> {
        self.endpoints.read().await.clone()
    }

    /// Get recent event log
    pub async fn recent_events(&self, limit: usize) -> Vec<WebhookEvent> {
        let log = self.event_log.read().await;
        log.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_broadcast() {
        let dispatcher = WebhookDispatcher::new();

        dispatcher.register_endpoint(WebhookEndpoint {
            endpoint_id: Uuid::new_v4(),
            url: "https://hooks.example.com/Zentra".into(),
            secret: Some("webhook-secret".into()),
            events: vec!["alert.triggered".into(), "alert.resolved".into()],
            enabled: true,
            retry_count: 3,
            timeout_seconds: 10,
            created_at: Utc::now(),
        }).await;

        dispatcher.broadcast("alert.triggered", serde_json::json!({
            "alert_id": "abc",
            "severity": "critical",
            "message": "CPU > 95%"
        })).await;

        let events = dispatcher.recent_events(10).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "alert.triggered");
        assert_eq!(events[0].status, WebhookStatus::Delivered);
    }

    #[tokio::test]
    async fn test_wildcard_endpoint() {
        let dispatcher = WebhookDispatcher::new();

        dispatcher.register_endpoint(WebhookEndpoint {
            endpoint_id: Uuid::new_v4(),
            url: "https://audit.example.com".into(),
            secret: None,
            events: vec!["*".into()],
            enabled: true,
            retry_count: 1,
            timeout_seconds: 5,
            created_at: Utc::now(),
        }).await;

        dispatcher.broadcast("license.expired", serde_json::json!({"lic": "xyz"})).await;
        let events = dispatcher.recent_events(10).await;
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_disabled_endpoint_skipped() {
        let dispatcher = WebhookDispatcher::new();

        dispatcher.register_endpoint(WebhookEndpoint {
            endpoint_id: Uuid::new_v4(),
            url: "https://disabled.example.com".into(),
            secret: None,
            events: vec!["*".into()],
            enabled: false,
            retry_count: 1,
            timeout_seconds: 5,
            created_at: Utc::now(),
        }).await;

        dispatcher.broadcast("alert.triggered", serde_json::json!({})).await;
        let events = dispatcher.recent_events(10).await;
        assert_eq!(events.len(), 0);
    }
}
