use axum::{Router, routing::{get, post, put, delete}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // PostgreSQL Monitoring
        .route("/api/postgres/overview", get(handlers::postgres::overview))
        .route("/api/postgres/connections", get(handlers::postgres::connections))
        .route("/api/postgres/slow-queries", get(handlers::postgres::slow_queries))
        .route("/api/postgres/tables", get(handlers::postgres::table_stats))
        .route("/api/postgres/indexes", get(handlers::postgres::index_stats))
        .route("/api/postgres/replication", get(handlers::postgres::replication))
        .route("/api/postgres/locks", get(handlers::postgres::locks))
        .route("/api/postgres/wait-events", get(handlers::postgres::wait_events))
        .route("/api/postgres/vacuum-progress", get(handlers::postgres::vacuum_progress))
        .route("/api/postgres/performance-history", get(handlers::postgres::performance_history))
        .route("/api/postgres/pgpool", get(handlers::postgres::pgpool_status))

        // MySQL Monitoring
        .route("/api/mysql/overview", get(handlers::mysql_mon::overview))
        .route("/api/mysql/connections", get(handlers::mysql_mon::connections))
        .route("/api/mysql/slow-queries", get(handlers::mysql_mon::slow_queries))
        .route("/api/mysql/innodb", get(handlers::mysql_mon::innodb_status))
        .route("/api/mysql/replication", get(handlers::mysql_mon::replication))
        .route("/api/mysql/tables", get(handlers::mysql_mon::table_stats))
        .route("/api/mysql/performance-history", get(handlers::mysql_mon::performance_history))

        // MongoDB Monitoring
        .route("/api/mongodb/overview", get(handlers::mongodb_mon::overview))
        .route("/api/mongodb/collections", get(handlers::mongodb_mon::collections))
        .route("/api/mongodb/replicaset", get(handlers::mongodb_mon::replicaset))
        .route("/api/mongodb/operations", get(handlers::mongodb_mon::operations))
        .route("/api/mongodb/indexes", get(handlers::mongodb_mon::indexes))
        .route("/api/mongodb/performance-history", get(handlers::mongodb_mon::performance_history))

        // SQL Server Monitoring
        .route("/api/sqlserver/overview", get(handlers::sqlserver_mon::overview))
        .route("/api/sqlserver/connections", get(handlers::sqlserver_mon::connections))
        .route("/api/sqlserver/queries", get(handlers::sqlserver_mon::top_queries))
        .route("/api/sqlserver/databases", get(handlers::sqlserver_mon::databases))
        .route("/api/sqlserver/jobs", get(handlers::sqlserver_mon::agent_jobs))
        .route("/api/sqlserver/wait-stats", get(handlers::sqlserver_mon::wait_stats))
        .route("/api/sqlserver/performance-history", get(handlers::sqlserver_mon::performance_history))

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
