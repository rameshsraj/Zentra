//! Scrape target discovery for Zentra.
//! Reference: prometheus-main/discovery/ — static config, DNS-SD, K8s SD.
//!
//! Supports:
//! - Static configuration (manual target list)
//! - DNS-based service discovery (SRV records)
//! - Kubernetes service discovery (via kube-apiserver)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug, warn};
use uuid::Uuid;

/// A scrape target endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeTarget {
    pub target_id: Uuid,
    pub org_id: Uuid,
    pub host: String,
    pub port: u16,
    pub scheme: Option<String>,
    pub metrics_path: Option<String>,
    pub job_name: Option<String>,
    pub scrape_interval_seconds: Option<u32>,
    pub labels: HashMap<String, String>,
    pub enabled: bool,
    pub discovered_by: String,
}

impl ScrapeTarget {
    pub fn new(org_id: Uuid, host: impl Into<String>, port: u16) -> Self {
        Self {
            target_id: Uuid::now_v7(),
            org_id,
            host: host.into(),
            port,
            scheme: None,
            metrics_path: None,
            job_name: None,
            scrape_interval_seconds: None,
            labels: HashMap::new(),
            enabled: true,
            discovered_by: "static".into(),
        }
    }

    pub fn with_job(mut self, job: impl Into<String>) -> Self {
        self.job_name = Some(job.into());
        self
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.metrics_path = Some(path.into());
        self
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// Discovery provider trait.
pub trait DiscoveryProvider: Send + Sync {
    fn name(&self) -> &str;
    fn discover(&self) -> Vec<ScrapeTarget>;
}

/// Static target configuration — user-defined list.
pub struct StaticDiscovery {
    targets: Vec<ScrapeTarget>,
}

impl StaticDiscovery {
    pub fn new(targets: Vec<ScrapeTarget>) -> Self {
        Self { targets }
    }
}

impl DiscoveryProvider for StaticDiscovery {
    fn name(&self) -> &str {
        "static"
    }

    fn discover(&self) -> Vec<ScrapeTarget> {
        self.targets.clone()
    }
}

/// DNS-based service discovery using SRV records.
pub struct DnsServiceDiscovery {
    org_id: Uuid,
    /// DNS names to query for SRV records (e.g., _prometheus._tcp.example.com)
    dns_names: Vec<String>,
    job_name: String,
}

impl DnsServiceDiscovery {
    pub fn new(org_id: Uuid, dns_names: Vec<String>, job_name: impl Into<String>) -> Self {
        Self {
            org_id,
            dns_names,
            job_name: job_name.into(),
        }
    }
}

impl DiscoveryProvider for DnsServiceDiscovery {
    fn name(&self) -> &str {
        "dns_sd"
    }

    fn discover(&self) -> Vec<ScrapeTarget> {
        let mut targets = Vec::new();

        for dns_name in &self.dns_names {
            // In production: resolve SRV records
            // let records = dns_lookup::lookup_srv(dns_name);
            // For each SRV record → ScrapeTarget { host: record.target, port: record.port }
            debug!(dns_name = %dns_name, "DNS-SD discovery (stub)");
        }

        info!(count = targets.len(), provider = "dns_sd", "Discovery complete");
        targets
    }
}

/// Kubernetes service discovery — discovers pods/services with
/// prometheus.io/scrape annotations.
pub struct KubernetesDiscovery {
    org_id: Uuid,
    api_server: String,
    namespace: Option<String>,
    bearer_token: Option<String>,
}

impl KubernetesDiscovery {
    pub fn new(org_id: Uuid, api_server: impl Into<String>) -> Self {
        Self {
            org_id,
            api_server: api_server.into(),
            namespace: None,
            bearer_token: None,
        }
    }

    pub fn with_namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }
}

impl DiscoveryProvider for KubernetesDiscovery {
    fn name(&self) -> &str {
        "kubernetes_sd"
    }

    fn discover(&self) -> Vec<ScrapeTarget> {
        let mut targets = Vec::new();

        // In production:
        // 1. Query K8s API: /api/v1/pods (or /api/v1/services)
        // 2. Filter by annotation: prometheus.io/scrape: "true"
        // 3. Extract:
        //    - host = pod IP
        //    - port = annotation prometheus.io/port (default: 9090)
        //    - path = annotation prometheus.io/path (default: /metrics)
        //    - labels = namespace, pod_name, service_name

        debug!(
            api_server = %self.api_server,
            namespace = ?self.namespace,
            "K8s discovery (stub)"
        );

        info!(count = targets.len(), provider = "kubernetes_sd", "Discovery complete");
        targets
    }
}

/// Unified target discovery manager.
pub struct TargetDiscovery {
    providers: Vec<Box<dyn DiscoveryProvider>>,
}

impl TargetDiscovery {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn DiscoveryProvider>) {
        info!(provider = provider.name(), "Discovery provider registered");
        self.providers.push(provider);
    }

    /// Run all discovery providers and return merged target list.
    pub fn discover_all(&self) -> Vec<ScrapeTarget> {
        let mut all_targets = Vec::new();
        for provider in &self.providers {
            let targets = provider.discover();
            debug!(
                provider = provider.name(),
                count = targets.len(),
                "Provider discovery result"
            );
            all_targets.extend(targets);
        }
        info!(total = all_targets.len(), "All discovery providers evaluated");
        all_targets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrape_target_builder() {
        let org = Uuid::new_v4();
        let target = ScrapeTarget::new(org, "localhost", 9090)
            .with_job("node_exporter")
            .with_path("/metrics")
            .with_label("env", "production");

        assert_eq!(target.host, "localhost");
        assert_eq!(target.port, 9090);
        assert_eq!(target.job_name, Some("node_exporter".into()));
        assert_eq!(target.labels.get("env"), Some(&"production".to_string()));
    }

    #[test]
    fn test_static_discovery() {
        let org = Uuid::new_v4();
        let targets = vec![
            ScrapeTarget::new(org, "host1", 9090),
            ScrapeTarget::new(org, "host2", 9090),
        ];
        let sd = StaticDiscovery::new(targets);
        assert_eq!(sd.discover().len(), 2);
    }

    #[test]
    fn test_target_discovery_manager() {
        let org = Uuid::new_v4();
        let mut discovery = TargetDiscovery::new();
        discovery.add_provider(Box::new(StaticDiscovery::new(vec![
            ScrapeTarget::new(org, "host1", 9090),
        ])));
        let targets = discovery.discover_all();
        assert_eq!(targets.len(), 1);
    }
}
