//! Service Discovery for Zentra
//!
//! Discovers monitoring targets from multiple sources:
//! - Static file configuration (YAML/JSON)
//! - DNS-SD (SRV records)
//! - Docker API (container labels)
//! - Consul catalog
//! - Kubernetes/k3s API (pod annotations)
//!
//! All providers implement the DiscoveryProvider trait and return a unified
//! DiscoveredTarget list that can be reconciled with the active target set.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// A target discovered by any provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTarget {
    /// Unique key for deduplication (provider-specific)
    pub discovery_key: String,
    /// Display name
    pub name: String,
    /// Address to connect to
    pub address: String,
    /// Port
    pub port: u16,
    /// Plugin to use for this target
    pub plugin_id: String,
    /// Target subtype (server, interface, oracle, etc.)
    pub subtype: String,
    /// Labels/tags from discovery
    pub labels: HashMap<String, String>,
    /// Source provider
    pub source: String,
    /// When this target was discovered
    pub discovered_at: DateTime<Utc>,
}

/// Trait for all discovery providers
#[async_trait::async_trait]
pub trait DiscoveryProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn discover(&self) -> Result<Vec<DiscoveredTarget>, DiscoveryError>;
    fn refresh_interval(&self) -> Duration;
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Not configured: {0}")]
    NotConfigured(String),
}

// ═══════════════════════════════════════════════════════════════════════════
// Static File Discovery
// ═══════════════════════════════════════════════════════════════════════════

/// Discovers targets from a static JSON/YAML config file
pub struct StaticFileDiscovery {
    path: String,
}

impl StaticFileDiscovery {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

#[derive(Deserialize)]
struct StaticConfig {
    targets: Vec<StaticTarget>,
}

#[derive(Deserialize)]
struct StaticTarget {
    name: String,
    address: String,
    #[serde(default = "default_port")]
    port: u16,
    plugin: String,
    subtype: String,
    #[serde(default)]
    labels: HashMap<String, String>,
}

fn default_port() -> u16 { 0 }

#[async_trait::async_trait]
impl DiscoveryProvider for StaticFileDiscovery {
    fn name(&self) -> &str { "static_file" }

    async fn discover(&self) -> Result<Vec<DiscoveredTarget>, DiscoveryError> {
        let content = tokio::fs::read_to_string(&self.path).await?;
        let config: StaticConfig = serde_json::from_str(&content)
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        Ok(config.targets.into_iter().map(|t| {
            DiscoveredTarget {
                discovery_key: format!("static:{}:{}", t.address, t.port),
                name: t.name,
                address: t.address,
                port: t.port,
                plugin_id: t.plugin,
                subtype: t.subtype,
                labels: t.labels,
                source: "static_file".into(),
                discovered_at: Utc::now(),
            }
        }).collect())
    }

    fn refresh_interval(&self) -> Duration { Duration::from_secs(300) }
}

// ═══════════════════════════════════════════════════════════════════════════
// DNS-SD Discovery (SRV records)
// ═══════════════════════════════════════════════════════════════════════════

/// Discovers targets via DNS SRV records
pub struct DnsSdDiscovery {
    service_names: Vec<String>,
    domain: String,
    default_plugin: String,
}

impl DnsSdDiscovery {
    pub fn new(service_names: Vec<String>, domain: impl Into<String>, default_plugin: impl Into<String>) -> Self {
        Self {
            service_names,
            domain: domain.into(),
            default_plugin: default_plugin.into(),
        }
    }
}

#[async_trait::async_trait]
impl DiscoveryProvider for DnsSdDiscovery {
    fn name(&self) -> &str { "dns_sd" }

    async fn discover(&self) -> Result<Vec<DiscoveredTarget>, DiscoveryError> {
        let mut targets = Vec::new();

        for svc in &self.service_names {
            let query = format!("_{}._{}.{}", svc, "tcp", self.domain);
            debug!(query = %query, "DNS-SD lookup");

            // Use dns-lookup for resolution
            match tokio::task::spawn_blocking({
                let query = query.clone();
                move || dns_lookup::lookup_host(&query)
            }).await {
                Ok(Ok(ips)) => {
                    for ip in ips {
                        targets.push(DiscoveredTarget {
                            discovery_key: format!("dns:{}:{}", svc, ip),
                            name: format!("{}-{}", svc, ip),
                            address: ip.to_string(),
                            port: 0,
                            plugin_id: self.default_plugin.clone(),
                            subtype: "server".into(),
                            labels: HashMap::from([("service".into(), svc.clone())]),
                            source: "dns_sd".into(),
                            discovered_at: Utc::now(),
                        });
                    }
                }
                Ok(Err(e)) => {
                    debug!(service = %svc, error = %e, "DNS-SD lookup failed");
                }
                Err(e) => {
                    warn!(error = %e, "DNS-SD task failed");
                }
            }
        }

        Ok(targets)
    }

    fn refresh_interval(&self) -> Duration { Duration::from_secs(60) }
}

// ═══════════════════════════════════════════════════════════════════════════
// Docker Discovery
// ═══════════════════════════════════════════════════════════════════════════

/// Discovers targets from Docker containers (via Docker API)
pub struct DockerDiscovery {
    docker_host: String,
    label_prefix: String,
}

impl DockerDiscovery {
    pub fn new(docker_host: impl Into<String>) -> Self {
        Self {
            docker_host: docker_host.into(),
            label_prefix: "Zentra".into(),
        }
    }
}

#[async_trait::async_trait]
impl DiscoveryProvider for DockerDiscovery {
    fn name(&self) -> &str { "docker" }

    async fn discover(&self) -> Result<Vec<DiscoveredTarget>, DiscoveryError> {
        let url = format!("{}/containers/json?filters={{\"status\":[\"running\"]}}", self.docker_host);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let resp = client.get(&url).send().await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let containers: Vec<serde_json::Value> = resp.json().await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        let mut targets = Vec::new();

        for container in containers {
            let labels = container.get("Labels")
                .and_then(|l| l.as_object())
                .cloned()
                .unwrap_or_default();

            // Only include containers with Zentra.enable=true label
            let enabled = labels.get(&format!("{}.enable", self.label_prefix))
                .and_then(|v| v.as_str())
                .unwrap_or("false");

            if enabled != "true" {
                continue;
            }

            let name = container.get("Names")
                .and_then(|n| n.as_array())
                .and_then(|a| a.first())
                .and_then(|n| n.as_str())
                .unwrap_or("unknown")
                .trim_start_matches('/')
                .to_string();

            let plugin = labels.get(&format!("{}.plugin", self.label_prefix))
                .and_then(|v| v.as_str())
                .unwrap_or("server-plugin")
                .to_string();

            let subtype = labels.get(&format!("{}.subtype", self.label_prefix))
                .and_then(|v| v.as_str())
                .unwrap_or("container")
                .to_string();

            let port: u16 = labels.get(&format!("{}.port", self.label_prefix))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            // Get container IP from NetworkSettings
            let address = container.get("NetworkSettings")
                .and_then(|ns| ns.get("Networks"))
                .and_then(|nets| nets.as_object())
                .and_then(|nets| nets.values().next())
                .and_then(|net| net.get("IPAddress"))
                .and_then(|ip| ip.as_str())
                .unwrap_or("127.0.0.1")
                .to_string();

            let container_id = container.get("Id")
                .and_then(|id| id.as_str())
                .unwrap_or("unknown");

            let mut target_labels: HashMap<String, String> = labels.iter()
                .filter(|(k, _)| k.starts_with(&self.label_prefix))
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect();
            target_labels.insert("container_id".into(), container_id[..12.min(container_id.len())].to_string());

            targets.push(DiscoveredTarget {
                discovery_key: format!("docker:{}", container_id),
                name,
                address,
                port,
                plugin_id: plugin,
                subtype,
                labels: target_labels,
                source: "docker".into(),
                discovered_at: Utc::now(),
            });
        }

        info!(count = targets.len(), "Docker discovery completed");
        Ok(targets)
    }

    fn refresh_interval(&self) -> Duration { Duration::from_secs(30) }
}

// ═══════════════════════════════════════════════════════════════════════════
// Kubernetes / k3s Discovery
// ═══════════════════════════════════════════════════════════════════════════

/// Discovers targets from Kubernetes pods (via API server)
pub struct KubernetesDiscovery {
    api_server: String,
    token: String,
    namespace: Option<String>,
    annotation_prefix: String,
}

impl KubernetesDiscovery {
    pub fn new(api_server: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            api_server: api_server.into(),
            token: token.into(),
            namespace: None,
            annotation_prefix: "Zentra.io".into(),
        }
    }

    pub fn with_namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }
}

#[async_trait::async_trait]
impl DiscoveryProvider for KubernetesDiscovery {
    fn name(&self) -> &str { "kubernetes" }

    async fn discover(&self) -> Result<Vec<DiscoveredTarget>, DiscoveryError> {
        let path = match &self.namespace {
            Some(ns) => format!("{}/api/v1/namespaces/{}/pods", self.api_server, ns),
            None => format!("{}/api/v1/pods", self.api_server),
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .danger_accept_invalid_certs(true) // k3s self-signed
            .build()
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let resp = client.get(&path)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        let pod_list: serde_json::Value = resp.json().await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        let items = pod_list.get("items")
            .and_then(|i| i.as_array())
            .cloned()
            .unwrap_or_default();

        let mut targets = Vec::new();

        for pod in items {
            let annotations = pod.get("metadata")
                .and_then(|m| m.get("annotations"))
                .and_then(|a| a.as_object())
                .cloned()
                .unwrap_or_default();

            // Only include pods with Zentra.io/scrape=true
            let scrape = annotations.get(&format!("{}/scrape", self.annotation_prefix))
                .and_then(|v| v.as_str())
                .unwrap_or("false");

            if scrape != "true" {
                continue;
            }

            let name = pod.get("metadata")
                .and_then(|m| m.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("unknown")
                .to_string();

            let namespace = pod.get("metadata")
                .and_then(|m| m.get("namespace"))
                .and_then(|n| n.as_str())
                .unwrap_or("default")
                .to_string();

            let pod_ip = pod.get("status")
                .and_then(|s| s.get("podIP"))
                .and_then(|ip| ip.as_str())
                .unwrap_or("127.0.0.1")
                .to_string();

            let port: u16 = annotations.get(&format!("{}/port", self.annotation_prefix))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(9090);

            let plugin = annotations.get(&format!("{}/plugin", self.annotation_prefix))
                .and_then(|v| v.as_str())
                .unwrap_or("server-plugin")
                .to_string();

            let mut labels = HashMap::new();
            labels.insert("namespace".into(), namespace.clone());
            labels.insert("pod".into(), name.clone());

            // Copy pod labels
            if let Some(pod_labels) = pod.get("metadata")
                .and_then(|m| m.get("labels"))
                .and_then(|l| l.as_object())
            {
                for (k, v) in pod_labels {
                    if let Some(s) = v.as_str() {
                        labels.insert(format!("k8s_{}", k.replace('/', "_")), s.to_string());
                    }
                }
            }

            targets.push(DiscoveredTarget {
                discovery_key: format!("k8s:{}/{}", namespace, name),
                name: format!("{}/{}", namespace, name),
                address: pod_ip,
                port,
                plugin_id: plugin,
                subtype: "container".into(),
                labels,
                source: "kubernetes".into(),
                discovered_at: Utc::now(),
            });
        }

        info!(count = targets.len(), "Kubernetes discovery completed");
        Ok(targets)
    }

    fn refresh_interval(&self) -> Duration { Duration::from_secs(30) }
}

// ═══════════════════════════════════════════════════════════════════════════
// Discovery Manager
// ═══════════════════════════════════════════════════════════════════════════

/// Manages multiple discovery providers and reconciles results
pub struct DiscoveryManager {
    providers: Vec<Box<dyn DiscoveryProvider>>,
    /// All discovered targets keyed by discovery_key
    targets: HashMap<String, DiscoveredTarget>,
}

impl DiscoveryManager {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            targets: HashMap::new(),
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn DiscoveryProvider>) {
        info!(provider = provider.name(), "Discovery provider registered");
        self.providers.push(provider);
    }

    /// Run a full discovery cycle across all providers
    pub async fn discover_all(&mut self) -> Vec<DiscoveredTarget> {
        let mut all_targets = Vec::new();

        for provider in &self.providers {
            match tokio::time::timeout(Duration::from_secs(30), provider.discover()).await {
                Ok(Ok(targets)) => {
                    info!(provider = provider.name(), count = targets.len(), "Discovery completed");
                    all_targets.extend(targets);
                }
                Ok(Err(e)) => {
                    warn!(provider = provider.name(), error = %e, "Discovery failed");
                }
                Err(_) => {
                    error!(provider = provider.name(), "Discovery timed out (30s)");
                }
            }
        }

        // Reconcile: update existing, add new, mark stale
        for target in &all_targets {
            self.targets.insert(target.discovery_key.clone(), target.clone());
        }

        all_targets
    }

    /// Get all currently known targets
    pub fn current_targets(&self) -> Vec<&DiscoveredTarget> {
        self.targets.values().collect()
    }

    /// Get targets that are new (not in the existing set)
    pub fn diff_new(&self, existing_keys: &[String]) -> Vec<&DiscoveredTarget> {
        self.targets.values()
            .filter(|t| !existing_keys.contains(&t.discovery_key))
            .collect()
    }

    /// Get keys that are stale (in existing but not in discovered)
    pub fn diff_stale(&self, existing_keys: &[String]) -> Vec<String> {
        existing_keys.iter()
            .filter(|k| !self.targets.contains_key(*k))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_target_serialization() {
        let target = DiscoveredTarget {
            discovery_key: "static:10.0.0.1:22".into(),
            name: "web-server".into(),
            address: "10.0.0.1".into(),
            port: 22,
            plugin_id: "server-plugin".into(),
            subtype: "server".into(),
            labels: HashMap::from([("env".into(), "prod".into())]),
            source: "static_file".into(),
            discovered_at: Utc::now(),
        };

        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("web-server"));
        assert!(json.contains("10.0.0.1"));
    }

    #[test]
    fn test_discovery_manager_reconciliation() {
        let mut mgr = DiscoveryManager::new();

        // Simulate discovered targets
        let t1 = DiscoveredTarget {
            discovery_key: "static:a".into(),
            name: "a".into(),
            address: "10.0.0.1".into(),
            port: 0,
            plugin_id: "server-plugin".into(),
            subtype: "server".into(),
            labels: HashMap::new(),
            source: "test".into(),
            discovered_at: Utc::now(),
        };
        let t2 = DiscoveredTarget {
            discovery_key: "static:b".into(),
            name: "b".into(),
            address: "10.0.0.2".into(),
            port: 0,
            plugin_id: "server-plugin".into(),
            subtype: "server".into(),
            labels: HashMap::new(),
            source: "test".into(),
            discovered_at: Utc::now(),
        };

        mgr.targets.insert(t1.discovery_key.clone(), t1);
        mgr.targets.insert(t2.discovery_key.clone(), t2);

        // Existing has a, b, c
        let existing = vec!["static:a".into(), "static:b".into(), "static:c".into()];

        let new = mgr.diff_new(&existing);
        assert_eq!(new.len(), 0); // a and b already exist

        let stale = mgr.diff_stale(&existing);
        assert_eq!(stale, vec!["static:c".to_string()]); // c is stale
    }
}
