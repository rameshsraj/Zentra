use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Core metric model for all time-series data in Zentra.
///
/// Every metric carries full context for drill-down:
/// org → target → plugin → resource → metric_name
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metric {
    pub timestamp: DateTime<Utc>,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub plugin_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub metric_name: String,
    pub value: f64,
    pub unit: String,
    pub tags: BTreeMap<String, String>,
    pub metadata: Option<serde_json::Value>,
}

impl Metric {
    pub fn new(
        metric_name: impl Into<String>,
        value: f64,
        resource_id: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            org_id: Uuid::nil(),
            target_id: Uuid::nil(),
            plugin_id: String::new(),
            resource_type: String::new(),
            resource_id: resource_id.into(),
            metric_name: metric_name.into(),
            value,
            unit: String::new(),
            tags: BTreeMap::new(),
            metadata: None,
        }
    }

    pub fn with_context(mut self, org_id: Uuid, target_id: Uuid, plugin_id: impl Into<String>) -> Self {
        self.org_id = org_id;
        self.target_id = target_id;
        self.plugin_id = plugin_id.into();
        self
    }

    pub fn with_resource_type(mut self, resource_type: impl Into<String>) -> Self {
        self.resource_type = resource_type.into();
        self
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    pub fn with_tags(mut self, tags: BTreeMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Batch of metrics for efficient ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricBatch {
    pub metrics: Vec<Metric>,
    pub source_plugin: String,
    pub collected_at: DateTime<Utc>,
}

impl MetricBatch {
    pub fn new(plugin_id: impl Into<String>, metrics: Vec<Metric>) -> Self {
        Self {
            source_plugin: plugin_id.into(),
            collected_at: Utc::now(),
            metrics,
        }
    }

    pub fn len(&self) -> usize {
        self.metrics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_builder() {
        let org = Uuid::new_v4();
        let target = Uuid::new_v4();
        let m = Metric::new("cpu_usage", 75.5, "host-1")
            .with_context(org, target, "server-plugin")
            .with_resource_type("server")
            .with_unit("percent")
            .with_tag("env", "production");

        assert_eq!(m.metric_name, "cpu_usage");
        assert_eq!(m.value, 75.5);
        assert_eq!(m.org_id, org);
        assert_eq!(m.plugin_id, "server-plugin");
        assert_eq!(m.tags.get("env"), Some(&"production".to_string()));
    }

    #[test]
    fn test_metric_batch() {
        let batch = MetricBatch::new("test-plugin", vec![
            Metric::new("m1", 1.0, "r1"),
            Metric::new("m2", 2.0, "r2"),
        ]);
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_metric_serialization() {
        let m = Metric::new("disk_usage", 80.0, "sda1").with_unit("percent");
        let json = serde_json::to_string(&m).unwrap();
        let deserialized: Metric = serde_json::from_str(&json).unwrap();
        assert_eq!(m.metric_name, deserialized.metric_name);
        assert_eq!(m.value, deserialized.value);
    }
}
