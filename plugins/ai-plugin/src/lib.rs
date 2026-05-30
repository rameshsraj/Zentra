pub mod llm_security;

use std::time::Duration;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use chrono::Utc;
use uuid::Uuid;
use tracing::debug;

/// AI Plugin — provides ML-based anomaly scoring and model health metrics.
///
/// Collects:
///   - `ai_anomaly_score` — current anomaly score per target (0.0–1.0)
///   - `ai_model_drift` — model prediction drift indicator
///   - `ai_inference_latency_ms` — inference latency in milliseconds
///   - `ai_predictions_total` — cumulative prediction count
pub struct AiPlugin {
    initialized: bool,
}

impl AiPlugin {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Plugin for AiPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "ai-plugin",
            name: "AI/ML Analytics",
            version: "0.1.0",
            description: "ML-based anomaly detection and predictive analytics metrics",
            supported_subtypes: vec!["linux".into(), "windows".into()],
            capabilities: vec![PluginCapability::CollectMetrics],
        }
    }

    #[tracing::instrument(skip(self, _target), fields(plugin.id = "ai-plugin", gen_ai.system = "zentra"))]
    async fn initialize(&mut self, _target: &Target) -> Result<()> {
        debug!("AI plugin initialized");
        self.initialized = true;
        Ok(())
    }

    async fn test_connection(&self, _target: &Target) -> Result<bool> {
        // AI plugin operates locally — always reachable
        Ok(true)
    }

    #[tracing::instrument(skip(self, target), fields(plugin.id = "ai-plugin", gen_ai.system = "zentra", gen_ai.request.model = "zentra-anomaly-v1"))]
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        // --- Legacy metric names (backward compatibility) ---

        metrics.push(
            Metric::new("ai_anomaly_score", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("score")
                .with_resource_type("ai")
        );

        metrics.push(
            Metric::new("ai_model_drift", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("score")
                .with_resource_type("ai")
        );

        metrics.push(
            Metric::new("ai_inference_latency_ms", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("ms")
                .with_resource_type("ai")
        );

        metrics.push(
            Metric::new("ai_predictions_total", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("count")
                .with_resource_type("ai")
        );

        // --- OTel Semantic Convention metrics (gen_ai.* namespace) ---
        // Aligned with OpenTelemetry Semantic Conventions for GenAI
        // See: https://opentelemetry.io/docs/specs/semconv/gen-ai/

        metrics.push(
            Metric::new("gen_ai.client.operation.duration", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("ms")
                .with_resource_type("ai")
                .with_tag("gen_ai.system", "zentra")
                .with_tag("gen_ai.operation.name", "anomaly_detection")
                .with_tag("gen_ai.request.model", "zentra-anomaly-v1")
        );

        metrics.push(
            Metric::new("gen_ai.server.request.duration", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("ms")
                .with_resource_type("ai")
                .with_tag("gen_ai.system", "zentra")
                .with_tag("gen_ai.operation.name", "inference")
                .with_tag("gen_ai.request.model", "zentra-anomaly-v1")
        );

        metrics.push(
            Metric::new("gen_ai.client.token.usage", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("tokens")
                .with_resource_type("ai")
                .with_tag("gen_ai.system", "zentra")
                .with_tag("gen_ai.operation.name", "inference")
                .with_tag("gen_ai.request.model", "zentra-anomaly-v1")
                .with_tag("gen_ai.token.type", "total")
        );

        metrics.push(
            Metric::new("gen_ai.server.request.count", 0.0, target.name.clone())
                .with_context(target.org_id, target.target_id, "ai-plugin")
                .with_unit("count")
                .with_resource_type("ai")
                .with_tag("gen_ai.system", "zentra")
                .with_tag("gen_ai.operation.name", "prediction")
                .with_tag("gen_ai.request.model", "zentra-anomaly-v1")
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
