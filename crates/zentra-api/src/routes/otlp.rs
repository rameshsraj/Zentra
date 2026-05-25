use axum::{Router, routing::{get, post}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // OTel P03: OTLP Signal Ingest (JSON + Protobuf)
        .route("/v1/traces", post(handlers::otlp::ingest_traces))
        .route("/v1/metrics", post(handlers::otlp::ingest_metrics))
        .route("/v1/logs", post(handlers::otlp::ingest_logs))
        .route("/v1/profiles", post(handlers::otlp::ingest_profiles))

        // OTel P02: Collector Config & Status
        .route("/api/otel/status", get(handlers::otlp::otel_status))
        .route("/api/otel/collector-config", get(handlers::otlp::collector_config))

        // OTel P04: SDK Instrumentation Snippets
        .route("/api/otel/sdk-snippets", get(handlers::otlp::sdk_snippets))

        // OTel P05: Semantic Convention Attributes
        .route("/api/otel/attributes", get(handlers::otlp::otel_attributes))
        .route("/api/otel/attribute-values", get(handlers::otlp::otel_attribute_values))

        // OTel P06: Resource Detection
        .route("/api/otel/resources", get(handlers::otlp::otel_resources))
        .route("/api/otel/resources/:service_name/signals", get(handlers::otlp::otel_resource_signals))

        // OTel P07: Health Dashboard Stats
        .route("/api/otel/stats", get(handlers::otlp::otel_stats))
        .route("/api/otel/stats/timeseries", get(handlers::otlp::otel_stats_timeseries))

        // OTel P09: Span-to-Metrics Connector
        .route("/api/otel/connectors", get(handlers::otel_connectors::list_connectors).post(handlers::otel_connectors::configure_connector))
        .route("/api/otel/derived-metrics", get(handlers::otel_connectors::query_derived_metrics))

        // OTel P13: Collector Pipeline Topology
        .route("/api/otel/pipeline", get(handlers::otlp::otel_pipeline))
}
