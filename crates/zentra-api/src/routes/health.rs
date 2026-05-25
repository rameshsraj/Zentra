use axum::{Router, routing::get};
use crate::state::AppState;
use crate::handlers;
use crate::observability;

pub fn router() -> Router<AppState> {
    Router::new()
        // Prometheus self-monitoring metrics.
        // Restrict scraper access at the network/firewall layer; do not expose port 5006 externally.
        .route("/metrics", get(observability::metrics_handler))

        // Health
        .route("/api/health", get(handlers::health::health_check))

        // System
        .route("/api/system/status", get(handlers::system::system_status))
        .route("/api/system/db", get(handlers::system::db_health))

        // Global Demo/Live Mode
        .route("/api/config/mode", get(handlers::config::get_mode))
        .route("/api/config/mode", axum::routing::post(handlers::config::set_mode))

        // P11: Config/Status API
        .route("/api/v1/status/runtimeinfo", get(handlers::config::runtime_info))
        .route("/api/v1/status/buildinfo", get(handlers::config::build_info))
        .route("/api/v1/status/config", get(handlers::config::running_config))
        .route("/api/v1/status/flags", get(handlers::config::flags))
        .route("/api/v1/status/tsdb", get(handlers::config::tsdb_status))
        .route("/api/v1/admin/reload", axum::routing::post(handlers::config::reload_config))

        // P11: OpenAPI spec
        .route("/api/v1/openapi.json", get(handlers::config::openapi_spec))

        // API Docs — Swagger UI and Redoc
        .route("/api/docs", get(handlers::config::swagger_ui))
        .route("/api/docs/redoc", get(handlers::config::redoc_ui))
}
