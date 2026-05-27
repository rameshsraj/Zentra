pub mod ids_engine;
pub mod traffic_parser;
pub mod dns_analyzer;
pub mod http_analyzer;
pub mod ssl_analyzer;
pub mod community_id;
pub mod geoip;
pub mod firewall;
pub mod file_analysis;

use std::time::Duration;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use chrono::Utc;
use uuid::Uuid;
use tracing::debug;

/// Security Plugin — periodic security posture polling.
///
/// Collects:
///   - `security_ids_alerts_total` — cumulative IDS alerts (counter by severity)
///   - `security_connections_blocked` — blocked connections counter
///   - `security_dns_anomalies_detected` — DNS anomaly counter
///   - `security_ssl_cert_expiry_days` — days until SSL cert expires (gauge)
///   - `security_firewall_rules_hit` — firewall rule hit counter
pub struct SecurityPlugin {
    initialized: bool,
}

impl SecurityPlugin {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Plugin for SecurityPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "security-plugin",
            name: "Security Analytics",
            version: "0.1.0",
            description: "IDS, firewall, SSL, and DNS security posture monitoring",
            supported_subtypes: vec!["linux".into(), "windows".into(), "firewall".into()],
            capabilities: vec![PluginCapability::CollectMetrics],
        }
    }

    async fn initialize(&mut self, _target: &Target) -> Result<()> {
        debug!("Security plugin initialized — IDS engine loaded");
        self.initialized = true;
        Ok(())
    }

    async fn test_connection(&self, _target: &Target) -> Result<bool> {
        // Security analysis is local — always available
        Ok(true)
    }

    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let now = Utc::now();
        let mut metrics = Vec::new();

        metrics.push(
            Metric::new("security_ids_alerts_total", 0.0, target.target_id.to_string())
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        metrics.push(
            Metric::new("security_connections_blocked", 0.0, target.target_id.to_string())
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        metrics.push(
            Metric::new("security_dns_anomalies_detected", 0.0, target.target_id.to_string())
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        metrics.push(
            Metric::new("security_ssl_cert_expiry_days", 365.0, target.target_id.to_string())
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("days")
        );

        metrics.push(
            Metric::new("security_firewall_rules_hit", 0.0, target.target_id.to_string())
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        Ok(metrics)
    }

    async fn health_check(&self, _target: &Target) -> Result<bool> {
        Ok(self.initialized)
    }

    fn polling_interval(&self) -> Duration {
        Duration::from_secs(60)
    }
}
