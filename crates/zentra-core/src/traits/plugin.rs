use crate::models::metric::Metric;
use crate::models::log_entry::LogEntry;
use crate::models::target::Target;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Metadata describing a plugin's capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub supported_subtypes: Vec<String>,
    pub capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    CollectMetrics,
    CollectLogs,
    NetworkCapture,
    ConnectionTest,
    ResourceDiscovery,
}

/// Core plugin trait. Every monitoring plugin (network, database, storage, server) implements this.
///
/// Plugins are decoupled from the stream engine — they produce `Metric` and `LogEntry` items
/// that are pushed into the ingestion pipeline.
#[allow(async_fn_in_trait)]
pub trait Plugin: Send + Sync {
    /// Return plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Initialize the plugin (allocate resources, open connections)
    async fn initialize(&mut self, target: &Target) -> Result<()>;

    /// Test connectivity to the target
    async fn test_connection(&self, target: &Target) -> Result<bool>;

    /// Collect metrics from the target
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>>;

    /// Collect logs from the target (optional — default returns empty)
    async fn collect_logs(&self, target: &Target) -> Result<Vec<LogEntry>> {
        let _ = target;
        Ok(vec![])
    }

    /// Health check — is the plugin + target healthy?
    async fn health_check(&self, target: &Target) -> Result<bool>;

    /// How often should this plugin collect data?
    fn polling_interval(&self) -> Duration {
        Duration::from_secs(60)
    }

    /// Shutdown and release resources
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata() {
        let meta = PluginMetadata {
            id: "test-plugin",
            name: "Test Plugin",
            version: "0.1.0",
            description: "A test plugin",
            supported_subtypes: vec!["linux".into()],
            capabilities: vec![PluginCapability::CollectMetrics],
        };
        assert_eq!(meta.id, "test-plugin");
        assert!(meta.capabilities.contains(&PluginCapability::CollectMetrics));
    }
}
