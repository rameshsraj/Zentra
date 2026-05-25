use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Logs
        .route("/api/logs", get(handlers::logs::search_logs))
        .route("/api/logs/live", get(handlers::logs::live_logs_ws))

        // Log Engine (P10)
        .route("/api/logs/query", get(handlers::log_engine::query_logs))
        .route("/api/logs/labels", get(handlers::log_engine::list_labels))
        .route("/api/logs/label/:name/values", get(handlers::log_engine::label_values))
        .route("/api/logs/streams", get(handlers::log_engine::list_streams))
        .route("/api/logs/volume", get(handlers::log_engine::log_volume))
        .route("/api/logs/context/:stream_id/:timestamp", get(handlers::log_engine::log_context))
        .route("/api/logs/ingest", post(handlers::log_engine::native_ingest))
        .route("/api/loki/api/v1/push", post(handlers::log_engine::loki_push))
        .route("/api/logs/collect", post(handlers::log_engine::collect_logs))
        .route("/api/logs/export", get(handlers::log_engine::export_logs))
        .route("/api/logs/tail", get(handlers::log_engine::live_tail))

        // Log Alerts (P11)
        .route("/api/log-alerts", get(handlers::log_engine::list_log_alerts))
        .route("/api/log-alerts", post(handlers::log_engine::create_log_alert))
        .route("/api/log-alerts/:id", put(handlers::log_engine::update_log_alert))
        .route("/api/log-alerts/:id", delete(handlers::log_engine::delete_log_alert))

        // Log Analytics (P11)
        .route("/api/logs/analytics", get(handlers::log_engine::log_analytics))

        // Discovery Rules (P12)
        .route("/api/discovery-rules", get(handlers::log_engine::list_discovery_rules))
        .route("/api/discovery-rules", post(handlers::log_engine::create_discovery_rule))
        .route("/api/discovery-rules/:id", put(handlers::log_engine::update_discovery_rule))
        .route("/api/discovery-rules/:id", delete(handlers::log_engine::delete_discovery_rule))
        .route("/api/discovery-rules/:id/run", post(handlers::log_engine::run_discovery_rule))
        .route("/api/discovery-rules/:id/items", get(handlers::log_engine::discovery_rule_items))

        // Log Sources
        .route("/api/log-sources", get(handlers::log_sources::list_log_sources))
        .route("/api/log-sources", post(handlers::log_sources::create_log_source))
        .route("/api/log-sources/:id", delete(handlers::log_sources::delete_log_source))
}
