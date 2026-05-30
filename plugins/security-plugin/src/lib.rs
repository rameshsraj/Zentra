pub mod ids_engine;
pub mod traffic_parser;
pub mod dns_analyzer;
pub mod http_analyzer;
pub mod ssl_analyzer;
pub mod community_id;
pub mod geoip;
pub mod firewall;
pub mod file_analysis;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use tracing::{debug, info, warn};

use crate::ids_engine::IdsEngine;
use crate::ssl_analyzer::SslAnalyzer;

/// Security Plugin — live security posture monitoring.
///
/// Wires the IDS engine, SSL analyzer, DNS analyzer, and firewall modules
/// into the metric collection pipeline for real data (P09 SEC-LIVE).
///
/// Collects:
///   - `security_ids_alerts_total` — cumulative IDS alerts from EVE JSON
///   - `security_ids_rules_loaded` — number of active IDS rules
///   - `security_connections_blocked` — blocked connections from firewall logs
///   - `security_dns_anomalies_detected` — DNS anomaly counter
///   - `security_ssl_cert_expiry_days` — minimum days until SSL cert expires
///   - `security_ssl_weak_ciphers` — count of weak cipher connections
///   - `security_firewall_rules_hit` — total firewall rule hit counter
pub struct SecurityPlugin {
    initialized: bool,
    ids_engine: IdsEngine,
    ssl_analyzer: SslAnalyzer,
    // Atomic counters updated by log watchers between collection cycles
    ids_alert_count: AtomicU64,
    firewall_blocked_count: AtomicU64,
    firewall_total_hits: AtomicU64,
    dns_anomaly_count: AtomicU64,
}

impl SecurityPlugin {
    pub fn new() -> Self {
        Self {
            initialized: false,
            ids_engine: IdsEngine::new(),
            ssl_analyzer: SslAnalyzer::new(),
            ids_alert_count: AtomicU64::new(0),
            firewall_blocked_count: AtomicU64::new(0),
            firewall_total_hits: AtomicU64::new(0),
            dns_anomaly_count: AtomicU64::new(0),
        }
    }

    /// Feed an EVE JSON line from Suricata into the IDS engine.
    /// Call this from a file watcher or log ingestion pipeline.
    pub fn ingest_eve_line(&mut self, line: &str) {
        match serde_json::from_str::<ids_engine::EveJsonEvent>(line) {
            Ok(event) => {
                if let Some(_alert) = self.ids_engine.process_eve_event(&event) {
                    self.ids_alert_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(e) => {
                debug!("EVE JSON parse skip: {}", e);
            }
        }
    }

    /// Feed a firewall log line (iptables format).
    pub fn ingest_firewall_line(&self, line: &str) {
        if let Some(event) = firewall::parse_iptables(line) {
            self.firewall_total_hits.fetch_add(1, Ordering::Relaxed);
            if event.action == firewall::FirewallAction::Drop
                || event.action == firewall::FirewallAction::Reject
            {
                self.firewall_blocked_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Feed an SSL log entry for certificate analysis.
    pub fn ingest_ssl_entry(&mut self, entry: &ssl_analyzer::SslLogEntry) {
        self.ssl_analyzer.process(entry);
    }

    /// Increment DNS anomaly counter (called by dns_analyzer).
    pub fn record_dns_anomaly(&self) {
        self.dns_anomaly_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a reference to the IDS engine for external rule loading.
    pub fn ids_engine_mut(&mut self) -> &mut IdsEngine {
        &mut self.ids_engine
    }
}

impl Plugin for SecurityPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "security-plugin",
            name: "Security Analytics",
            version: "0.2.0",
            description: "Live IDS, firewall, SSL, and DNS security posture monitoring",
            supported_subtypes: vec!["linux".into(), "windows".into(), "firewall".into()],
            capabilities: vec![PluginCapability::CollectMetrics],
        }
    }

    #[tracing::instrument(skip(self, _target), fields(plugin.id = "security-plugin"))]
    async fn initialize(&mut self, _target: &Target) -> Result<()> {
        info!(
            ids_rules = self.ids_engine.rule_count(),
            "Security plugin initialized — IDS engine loaded"
        );
        self.initialized = true;
        Ok(())
    }

    async fn test_connection(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    #[tracing::instrument(skip(self, target), fields(plugin.id = "security-plugin"))]
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();
        let tid = target.target_id.to_string();

        // IDS metrics — live counters from EVE JSON ingestion
        let ids_alerts = self.ids_alert_count.load(Ordering::Relaxed);
        metrics.push(
            Metric::new("security_ids_alerts_total", ids_alerts as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        metrics.push(
            Metric::new("security_ids_rules_loaded", self.ids_engine.rule_count() as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        // Firewall metrics — live counters from log ingestion
        let fw_blocked = self.firewall_blocked_count.load(Ordering::Relaxed);
        let fw_total = self.firewall_total_hits.load(Ordering::Relaxed);
        metrics.push(
            Metric::new("security_connections_blocked", fw_blocked as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );
        metrics.push(
            Metric::new("security_firewall_rules_hit", fw_total as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        // DNS anomaly metrics
        let dns_anomalies = self.dns_anomaly_count.load(Ordering::Relaxed);
        metrics.push(
            Metric::new("security_dns_anomalies_detected", dns_anomalies as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );

        // SSL certificate metrics — from live SSL analyzer
        let ssl = self.ssl_analyzer.analytics();
        let min_expiry_days = ssl.expiring_certs
            .iter()
            .map(|c| c.days_remaining)
            .min()
            .unwrap_or(365);
        metrics.push(
            Metric::new("security_ssl_cert_expiry_days", min_expiry_days as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("days")
        );
        metrics.push(
            Metric::new("security_ssl_weak_ciphers", ssl.weak_ciphers.len() as f64, &tid)
                .with_context(target.org_id, target.target_id, "security-plugin")
                .with_resource_type("security")
                .with_unit("count")
        );
        metrics.push(
            Metric::new("security_ssl_connections_total", ssl.total_connections as f64, &tid)
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
