use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // P1: Oracle Database Monitoring
        .route("/api/oracle/overview", get(handlers::oracle::overview))
        .route("/api/oracle/rac", get(handlers::oracle::rac_overview))
        .route("/api/oracle/sysmetrics", get(handlers::oracle::sysmetrics))
        .route("/api/oracle/sysstat", get(handlers::oracle::sysstat))
        .route("/api/oracle/wait-classes", get(handlers::oracle::wait_classes))
        .route("/api/oracle/tablespaces", get(handlers::oracle::tablespaces))
        .route("/api/oracle/top-queries", get(handlers::oracle::top_queries))
        .route("/api/oracle/running-queries", get(handlers::oracle::running_queries))
        .route("/api/oracle/cache-hit-ratio", get(handlers::oracle::cache_hit_ratio))
        .route("/api/oracle/dataguard", get(handlers::oracle::dataguard))
        .route("/api/oracle/rman", get(handlers::oracle::rman))
        .route("/api/oracle/pdb", get(handlers::oracle::pdb_list))
        .route("/api/oracle/listeners", get(handlers::oracle::listeners))
        .route("/api/oracle/performance-history", get(handlers::oracle::performance_history))

        // P1: Oracle Chart Definitions
        .route("/api/oracle/chart-definitions", get(handlers::oracle::chart_definitions))

        // V5 Prompt 14: Oracle Connection Management
        .route("/api/oracle/connections", get(handlers::oracle::list_connections))
        .route("/api/oracle/connections", post(handlers::oracle::create_connection))
        .route("/api/oracle/connections/:id", put(handlers::oracle::update_connection))
        .route("/api/oracle/connections/:id", delete(handlers::oracle::delete_connection))
        .route("/api/oracle/connections/:id/test", post(handlers::oracle::test_connection))
        .route("/api/oracle/connections/:id/health", get(handlers::oracle::connection_health))

        // V5 Prompt 15: Oracle Metrics
        .route("/api/oracle/metrics", get(handlers::oracle_metrics::list_metrics))
        .route("/api/oracle/metrics", post(handlers::oracle_metrics::create_metric))
        .route("/api/oracle/metrics/upload", post(handlers::oracle_metrics::upload_metrics))
        .route("/api/oracle/metrics/reload", post(handlers::oracle_metrics::reload_metrics))
        .route("/api/oracle/metrics/:id", get(handlers::oracle_metrics::get_metric))
        .route("/api/oracle/metrics/:id", put(handlers::oracle_metrics::update_metric))
        .route("/api/oracle/metrics/:id", delete(handlers::oracle_metrics::delete_metric))
        .route("/api/oracle/metrics/:id/test", post(handlers::oracle_metrics::test_metric))

        // V5 Prompt 16: Oracle Alert Log
        .route("/api/oracle/alertlog", get(handlers::oracle::alert_log))
        .route("/api/oracle/alertlog/stats", get(handlers::oracle::alert_log_stats))

        // V5 Prompt 17 + P2: Oracle Scrape Management
        .route("/api/oracle/scrape/status", get(handlers::oracle::scrape_status))
        .route("/api/oracle/scrape/trigger", post(handlers::oracle::scrape_trigger))
        .route("/api/oracle/scrape/metrics", get(handlers::oracle::scrape_metrics))
        .route("/api/oracle/scheduler/start", post(handlers::oracle::scheduler_start))
        .route("/api/oracle/scheduler/stop", post(handlers::oracle::scheduler_stop))

        // V5 Prompt 18: TxEventQ
        .route("/api/oracle/txeventq", get(handlers::oracle::txeventq_list))
        .route("/api/oracle/txeventq/stats", get(handlers::oracle::txeventq_stats))
        .route("/api/oracle/txeventq/:queue_name", get(handlers::oracle::txeventq_detail))
}
