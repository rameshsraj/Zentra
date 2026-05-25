//! Plugin trait definition for Zentra monitoring plugins.
//! All plugins must implement this trait to integrate with the agent collection loop.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Metadata about a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub plugin_type: PluginType,
}

/// Plugin category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PluginType {
    Server,
    Network,
    Database,
    Storage,
    Security,
    Cloud,
    Custom(String),
}

/// Result of a single metric collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedMetric {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Result of a collection run
#[derive(Debug, Clone)]
pub struct CollectionResult {
    pub metrics: Vec<CollectedMetric>,
    pub logs: Vec<String>,
    pub errors: Vec<String>,
}

/// Plugin configuration passed to initialize()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub target_id: Uuid,
    pub target_host: String,
    pub target_port: u16,
    pub credentials: Option<HashMap<String, String>>,
    pub settings: HashMap<String, String>,
}

/// The core plugin trait. All monitoring plugins must implement this.
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    /// Return metadata about this plugin
    fn info(&self) -> PluginInfo;

    /// Initialize the plugin with configuration
    async fn initialize(&mut self, config: PluginConfig) -> Result<(), String>;

    /// Collect metrics from the target
    async fn collect(&self) -> Result<CollectionResult, String>;

    /// Health check — is the plugin connected and working?
    async fn health_check(&self) -> Result<bool, String>;

    /// Cleanup resources
    async fn shutdown(&self) -> Result<(), String> {
        Ok(()) // Default: no-op
    }
}
