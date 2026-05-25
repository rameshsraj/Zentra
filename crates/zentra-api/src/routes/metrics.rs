use axum::{Router, routing::{get, post}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Metrics
        .route("/api/metrics", get(handlers::metrics::query_metrics))
        .route("/api/metrics/aggregated", get(handlers::metrics::query_metrics_aggregated))

        // SW P03: MQE-Lite Metrics Query DSL
        .route("/api/query/mqe", post(handlers::metrics::mqe_query))
        .route("/api/query/mqe/validate", get(handlers::metrics::mqe_validate))
        .route("/api/query/metrics/names", get(handlers::metrics::metric_names))
        .route("/api/query/metrics/labels", get(handlers::metrics::metric_label_values))

        // Prometheus-Compatible Remote Write / Read
        .route("/api/v1/write", post(handlers::remote_write::remote_write))
        .route("/api/v1/read", post(handlers::remote_write::remote_read))

        // P12: Prometheus-Compatible Label/Metadata APIs
        .route("/api/v1/labels", get(handlers::metrics::list_labels))
        .route("/api/v1/label/:name/values", get(handlers::metrics::label_values))
        .route("/api/v1/metadata", get(handlers::metrics::metric_metadata))
        .route("/api/v1/series", get(handlers::metrics::query_series))
        .route("/api/v1/targets", get(handlers::metrics::prom_targets))

        // P5: Prometheus-compatible PromQL query endpoints
        .route("/api/v1/query", get(handlers::metrics::prom_query_instant))
        .route("/api/v1/query_range", get(handlers::metrics::prom_query_range))
}
