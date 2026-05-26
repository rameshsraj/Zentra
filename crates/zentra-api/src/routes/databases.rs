use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // AI Database Tools
        .route("/api/ai/nl-to-sql", post(handlers::ai_database::nl_to_sql))
        .route("/api/ai/explain-sql", post(handlers::ai_database::explain_sql))
        .route("/api/ai/optimize-sql", post(handlers::ai_database::optimize_sql))
        .route("/api/ai/convert-sql", post(handlers::ai_database::convert_sql))
        .route("/api/ai/analyze-performance", post(handlers::ai_database::analyze_performance))

        // SQL Workspace — Query Execution & History
        .route("/api/db/execute", post(handlers::database_query::execute_query))
        .route("/api/db/explain", post(handlers::database_query::explain_query))
        .route("/api/db/history", get(handlers::database_query::list_history))
        .route("/api/db/history/:id", delete(handlers::database_query::delete_history))

        // Database Schema Metadata
        .route("/api/db/:db_type/schemas", get(handlers::database_metadata::list_schemas))
        .route("/api/db/:db_type/tables", get(handlers::database_metadata::list_tables))
        .route("/api/db/:db_type/columns", get(handlers::database_metadata::list_columns))
        .route("/api/db/:db_type/indexes", get(handlers::database_metadata::list_indexes))

        // Universal Metric Definitions (all DB types)
        .route("/api/db/:db_type/metric-definitions", get(handlers::database_metrics::list_definitions))
        .route("/api/db/:db_type/metric-definitions", post(handlers::database_metrics::create_definition))
        .route("/api/db/:db_type/metric-definitions/:id", get(handlers::database_metrics::get_definition))
        .route("/api/db/:db_type/metric-definitions/:id", put(handlers::database_metrics::update_definition))
        .route("/api/db/:db_type/metric-definitions/:id", delete(handlers::database_metrics::delete_definition))
        .route("/api/db/:db_type/metric-definitions/test", post(handlers::database_metrics::test_definition))
}
