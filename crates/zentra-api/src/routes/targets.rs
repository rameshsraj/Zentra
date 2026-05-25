use axum::{Router, routing::{get, post, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // Targets
        .route("/api/targets", get(handlers::targets::list_targets))
        .route("/api/targets", post(handlers::targets::create_target))
        .route("/api/targets/:id", get(handlers::targets::get_target))
        .route("/api/targets/:id", delete(handlers::targets::delete_target))
        .route("/api/targets/:id/test", post(handlers::targets::test_connection))
        .route("/api/targets/test-db-connection", post(handlers::targets::test_db_connection))
        .route("/api/targets/verify-ssh", post(handlers::targets::verify_ssh))
        .route("/api/targets/setup-ssh", post(handlers::targets::setup_ssh))
        .route("/api/targets/:id/stats", get(handlers::targets::get_target_stats))
        .route("/api/targets/:id/crawl", post(handlers::targets::crawl_server))
        .route("/api/targets/:id/metrics/history", get(handlers::metric_collector::get_target_metrics_history))
        .route("/api/targets/:id/metrics/latest", get(handlers::metric_collector::get_target_metrics_latest))
        .route("/api/metrics/catalog", get(handlers::metric_collector::get_metrics_catalog))

        // SW P05: Target Hierarchy
        .route("/api/targets/:id/hierarchy", get(handlers::targets::get_target_hierarchy))
        .route("/api/targets/:id/children", get(handlers::targets::get_target_children))
        .route("/api/targets/:id/parent", axum::routing::patch(handlers::targets::set_target_parent))

        // Scrape Targets (Prometheus-compatible)
        .route("/api/scrape/targets", get(handlers::scrape::list_scrape_targets))
        .route("/api/scrape/targets", post(handlers::scrape::create_scrape_target))
        .route("/api/scrape/targets/:id", delete(handlers::scrape::delete_scrape_target))
        .route("/api/scrape/health", get(handlers::scrape::scrape_health_summary))
        .route("/api/scrape/recording-rules", get(handlers::scrape::list_recording_rules))

        // Service Discovery
        .route("/api/service-discovery/providers", get(handlers::service_discovery::list_providers))
        .route("/api/service-discovery/providers", post(handlers::service_discovery::create_provider))
        .route("/api/service-discovery/providers/:id", delete(handlers::service_discovery::delete_provider))
        .route("/api/service-discovery/providers/:id/sync", post(handlers::service_discovery::sync_provider))
        .route("/api/service-discovery/targets", get(handlers::service_discovery::list_discovered_targets))

        // V5 Prompt 10: Advanced Discovery
        .route("/api/discovery/scan", post(handlers::service_discovery::trigger_scan))
        .route("/api/discovery/scan/:id/results", get(handlers::service_discovery::scan_results))
        .route("/api/discovery/auto-register", post(handlers::service_discovery::auto_register))
        .route("/api/discovery/lld/:target_id", get(handlers::service_discovery::low_level_discovery))
        .route("/api/discovery/snmp-walk", post(handlers::service_discovery::snmp_walk))

        // Collectors API
        .route("/api/collectors", get(handlers::collectors::list_collectors))
        .route("/api/collectors", post(handlers::collectors::create_collector))
        .route("/api/collectors/:name/metrics", get(handlers::collectors::get_collector_metrics))
        .route("/api/collectors/:name/charts", get(handlers::collectors::get_collector_charts))
        .route("/api/collectors/:name/check", post(handlers::collectors::check_collector))
        .route("/api/collectors/:name", delete(handlers::collectors::delete_collector))
}
