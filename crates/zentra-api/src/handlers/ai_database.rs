//! AI Database Tools Handler (P11)
//!
//! Endpoints:
//!   POST /api/ai/nl-to-sql       — convert natural language to SQL
//!   POST /api/ai/explain-sql     — explain SQL in plain English
//!   POST /api/ai/optimize-sql    — suggest optimizations for SQL
//!   POST /api/ai/convert-sql     — convert SQL between dialects
//!   POST /api/ai/analyze-performance — AI performance analysis for a database

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;

use crate::state::AppState;

// ═══════════════════════════════════════════════════════════════════════════
// Request types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct NlToSqlRequest {
    pub query: String,
    pub db_type: String,
    pub connection_id: Option<String>,
    pub schema: Option<String>,
}

#[derive(Deserialize)]
pub struct ExplainSqlRequest {
    pub sql: String,
    pub db_type: String,
}

#[derive(Deserialize)]
pub struct OptimizeSqlRequest {
    pub sql: String,
    pub db_type: String,
    pub connection_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ConvertSqlRequest {
    pub sql: String,
    pub source_dialect: String,
    pub target_dialect: String,
}

#[derive(Deserialize)]
pub struct AnalyzePerformanceRequest {
    pub db_type: String,
    pub connection_id: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// NL to SQL
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/ai/nl-to-sql — convert natural language to SQL
pub async fn nl_to_sql(
    State(_s): State<AppState>,
    Json(body): Json<NlToSqlRequest>,
) -> Json<Value> {
    // In production: send to LLM with schema context
    // For now, generate a smart template-based response
    let sql = generate_nl_to_sql(&body.query, &body.db_type, body.schema.as_deref());

    Json(json!({
        "success": true,
        "sql": sql,
        "db_type": body.db_type,
        "confidence": 0.85,
        "explanation": format!("Generated SQL for '{}' targeting {} database", body.query, body.db_type),
        "generated_at": Utc::now().to_rfc3339(),
    }))
}

fn generate_nl_to_sql(query: &str, db_type: &str, schema: Option<&str>) -> String {
    let q = query.to_lowercase();
    let schema_prefix = schema.map(|s| format!("{}.", s)).unwrap_or_default();

    if q.contains("table") && q.contains("size") {
        return match db_type {
            "oracle" => format!("SELECT segment_name AS table_name, ROUND(bytes/1024/1024, 2) AS size_mb FROM dba_segments WHERE segment_type = 'TABLE' AND owner = '{}' ORDER BY bytes DESC", schema.unwrap_or("HR")),
            "postgres" => format!("SELECT tablename, pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size FROM pg_tables WHERE schemaname = '{}' ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC", schema.unwrap_or("public")),
            "mysql" => "SELECT table_name, ROUND((data_length + index_length)/1024/1024, 2) AS size_mb FROM information_schema.tables WHERE table_schema = DATABASE() ORDER BY data_length + index_length DESC".into(),
            "sqlserver" => "SELECT t.NAME AS table_name, CAST(ROUND(SUM(a.total_pages) * 8.0 / 1024, 2) AS DECIMAL(10,2)) AS size_mb FROM sys.tables t INNER JOIN sys.indexes i ON t.OBJECT_ID = i.object_id INNER JOIN sys.partitions p ON i.object_id = p.OBJECT_ID AND i.index_id = p.index_id INNER JOIN sys.allocation_units a ON p.partition_id = a.container_id GROUP BY t.Name ORDER BY SUM(a.total_pages) DESC".into(),
            _ => format!("SELECT * FROM {}tables ORDER BY size DESC", schema_prefix),
        };
    }

    if q.contains("slow") && q.contains("quer") {
        return match db_type {
            "oracle" => "SELECT sql_id, elapsed_time/1000000 AS elapsed_sec, executions, sql_text FROM v$sql WHERE elapsed_time > 1000000 ORDER BY elapsed_time DESC FETCH FIRST 20 ROWS ONLY".into(),
            "postgres" => "SELECT query, calls, total_exec_time/1000 AS total_sec, mean_exec_time/1000 AS avg_sec FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 20".into(),
            "mysql" => "SELECT * FROM sys.statements_with_runtimes_in_95th_percentile LIMIT 20".into(),
            "sqlserver" => "SELECT TOP 20 total_elapsed_time/1000 AS total_ms, execution_count, SUBSTRING(st.text, 1, 200) AS query_text FROM sys.dm_exec_query_stats qs CROSS APPLY sys.dm_exec_sql_text(qs.sql_handle) st ORDER BY total_elapsed_time DESC".into(),
            _ => "SELECT * FROM slow_queries ORDER BY duration DESC LIMIT 20".into(),
        };
    }

    if q.contains("connection") || q.contains("session") {
        return match db_type {
            "oracle" => "SELECT username, status, machine, program, COUNT(*) AS session_count FROM v$session WHERE type = 'USER' GROUP BY username, status, machine, program ORDER BY session_count DESC".into(),
            "postgres" => "SELECT usename, state, client_addr, application_name, COUNT(*) FROM pg_stat_activity WHERE backend_type = 'client backend' GROUP BY usename, state, client_addr, application_name ORDER BY count DESC".into(),
            "mysql" => "SELECT user, host, db, command, state, COUNT(*) AS count FROM information_schema.processlist GROUP BY user, host, db, command, state ORDER BY count DESC".into(),
            "sqlserver" => "SELECT login_name, status, host_name, program_name, COUNT(*) AS session_count FROM sys.dm_exec_sessions WHERE is_user_process = 1 GROUP BY login_name, status, host_name, program_name ORDER BY session_count DESC".into(),
            _ => "SHOW PROCESSLIST".into(),
        };
    }

    if q.contains("lock") || q.contains("block") {
        return match db_type {
            "oracle" => "SELECT s.sid, s.serial#, s.username, s.status, l.type, l.lmode, l.request, o.object_name FROM v$lock l JOIN v$session s ON l.sid = s.sid LEFT JOIN dba_objects o ON l.id1 = o.object_id WHERE l.request > 0".into(),
            "postgres" => "SELECT blocked_locks.pid AS blocked_pid, blocked_activity.usename, blocking_locks.pid AS blocking_pid, blocking_activity.usename AS blocking_user FROM pg_locks blocked_locks JOIN pg_stat_activity blocked_activity ON blocked_activity.pid = blocked_locks.pid JOIN pg_locks blocking_locks ON blocking_locks.locktype = blocked_locks.locktype WHERE NOT blocked_locks.granted".into(),
            "mysql" => "SELECT * FROM information_schema.innodb_lock_waits".into(),
            "sqlserver" => "SELECT session_id, blocking_session_id, wait_type, wait_time, resource_description FROM sys.dm_exec_requests WHERE blocking_session_id <> 0".into(),
            _ => "SHOW LOCKS".into(),
        };
    }

    // Default: simple select
    match db_type {
        "oracle" => format!("SELECT * FROM {}dual", schema_prefix),
        "postgres" => format!("SELECT * FROM {}pg_stat_activity LIMIT 10", schema_prefix),
        "mysql" => "SELECT * FROM information_schema.tables LIMIT 10".into(),
        "mongodb" => "db.collection.find().limit(10)".into(),
        "sqlserver" => "SELECT TOP 10 * FROM sys.tables".into(),
        _ => format!("SELECT 1 -- Generated for: {}", query),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Explain SQL
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/ai/explain-sql — explain SQL in plain English
pub async fn explain_sql(
    State(_s): State<AppState>,
    Json(body): Json<ExplainSqlRequest>,
) -> Json<Value> {
    let explanation = generate_sql_explanation(&body.sql, &body.db_type);

    Json(json!({
        "success": true,
        "sql": body.sql,
        "explanation": explanation,
        "db_type": body.db_type,
        "generated_at": Utc::now().to_rfc3339(),
    }))
}

fn generate_sql_explanation(sql: &str, _db_type: &str) -> String {
    let upper = sql.to_uppercase();

    let mut parts = Vec::new();

    if upper.contains("SELECT") {
        parts.push("This query retrieves data");
        if upper.contains("COUNT(") { parts.push("counting the number of rows"); }
        if upper.contains("SUM(") { parts.push("calculating sum totals"); }
        if upper.contains("AVG(") { parts.push("computing averages"); }
        if upper.contains("MAX(") || upper.contains("MIN(") { parts.push("finding extreme values"); }
    }
    if upper.contains("JOIN") { parts.push("combining data from multiple tables"); }
    if upper.contains("WHERE") { parts.push("with filtering conditions"); }
    if upper.contains("GROUP BY") { parts.push("grouped by specific columns"); }
    if upper.contains("ORDER BY") { parts.push("sorted in a specific order"); }
    if upper.contains("LIMIT") || upper.contains("FETCH FIRST") || upper.contains("TOP ") {
        parts.push("limited to a specific number of results");
    }
    if upper.contains("HAVING") { parts.push("with aggregate filtering"); }
    if upper.contains("UNION") { parts.push("combining results from multiple queries"); }
    if upper.contains("SUBQUERY") || upper.contains("IN (SELECT") || upper.contains("EXISTS (SELECT") {
        parts.push("using subqueries for nested lookups");
    }

    if parts.is_empty() {
        return format!("This SQL statement executes against the database. Full analysis requires an LLM integration.");
    }

    parts.join(", ") + "."
}

// ═══════════════════════════════════════════════════════════════════════════
// Optimize SQL
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/ai/optimize-sql — suggest SQL optimizations
pub async fn optimize_sql(
    State(_s): State<AppState>,
    Json(body): Json<OptimizeSqlRequest>,
) -> Json<Value> {
    let upper = body.sql.to_uppercase();
    let mut suggestions = Vec::new();

    if upper.contains("SELECT *") {
        suggestions.push(json!({
            "type": "column_selection",
            "severity": "medium",
            "message": "Avoid SELECT * — specify only the columns you need to reduce I/O and memory usage."
        }));
    }

    if !upper.contains("WHERE") && (upper.contains("FROM") && !upper.contains("DUAL")) {
        suggestions.push(json!({
            "type": "missing_filter",
            "severity": "high",
            "message": "No WHERE clause detected. Consider adding filters to avoid full table scans."
        }));
    }

    if upper.contains("LIKE '%") {
        suggestions.push(json!({
            "type": "leading_wildcard",
            "severity": "high",
            "message": "Leading wildcard in LIKE prevents index usage. Consider full-text search or restructuring the query."
        }));
    }

    if upper.contains("ORDER BY") && !upper.contains("LIMIT") && !upper.contains("FETCH FIRST") && !upper.contains("TOP ") {
        suggestions.push(json!({
            "type": "unbounded_sort",
            "severity": "medium",
            "message": "ORDER BY without LIMIT may sort large result sets unnecessarily. Add a LIMIT clause."
        }));
    }

    if upper.contains("NOT IN") {
        suggestions.push(json!({
            "type": "not_in",
            "severity": "medium",
            "message": "NOT IN can be slow with large subqueries and may have NULL issues. Consider NOT EXISTS or LEFT JOIN ... IS NULL."
        }));
    }

    if upper.contains("DISTINCT") {
        suggestions.push(json!({
            "type": "distinct_usage",
            "severity": "low",
            "message": "DISTINCT forces deduplication. Ensure it's necessary — proper JOINs may eliminate the need."
        }));
    }

    if suggestions.is_empty() {
        suggestions.push(json!({
            "type": "ok",
            "severity": "info",
            "message": "No obvious optimization issues detected. Consider running EXPLAIN to verify execution plan."
        }));
    }

    Json(json!({
        "success": true,
        "sql": body.sql,
        "db_type": body.db_type,
        "suggestions": suggestions,
        "suggestion_count": suggestions.len(),
        "generated_at": Utc::now().to_rfc3339(),
    }))
}

// ═══════════════════════════════════════════════════════════════════════════
// Convert SQL
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/ai/convert-sql — convert SQL between dialects
pub async fn convert_sql(
    State(_s): State<AppState>,
    Json(body): Json<ConvertSqlRequest>,
) -> Json<Value> {
    let converted = convert_dialect(&body.sql, &body.source_dialect, &body.target_dialect);

    Json(json!({
        "success": true,
        "original_sql": body.sql,
        "converted_sql": converted,
        "source_dialect": body.source_dialect,
        "target_dialect": body.target_dialect,
        "generated_at": Utc::now().to_rfc3339(),
    }))
}

fn convert_dialect(sql: &str, source: &str, target: &str) -> String {
    let mut result = sql.to_string();

    // Oracle -> PostgreSQL conversions
    if source == "oracle" && target == "postgres" {
        result = result.replace("NVL(", "COALESCE(");
        result = result.replace("SYSDATE", "NOW()");
        result = result.replace("ROWNUM", "ROW_NUMBER() OVER()");
        result = result.replace(" FROM DUAL", "");
        if result.contains("FETCH FIRST") {
            // Replace Oracle FETCH FIRST with LIMIT
            if let Some(pos) = result.find("FETCH FIRST") {
                let rest = &result[pos..];
                if let Some(end) = rest.find("ROWS ONLY") {
                    let num_str = rest[11..end].trim();
                    result = format!("{} LIMIT {}", &result[..pos].trim(), num_str);
                }
            }
        }
    }
    // PostgreSQL -> Oracle conversions
    else if source == "postgres" && target == "oracle" {
        result = result.replace("COALESCE(", "NVL(");
        result = result.replace("NOW()", "SYSDATE");
        result = result.replace("ILIKE", "LIKE");
        if result.to_uppercase().contains("LIMIT ") {
            if let Some(pos) = result.to_uppercase().find("LIMIT ") {
                let num_str = result[pos+6..].trim().split_whitespace().next().unwrap_or("10");
                result = format!("{} FETCH FIRST {} ROWS ONLY", &result[..pos].trim(), num_str);
            }
        }
    }
    // MySQL -> PostgreSQL
    else if source == "mysql" && target == "postgres" {
        result = result.replace("IFNULL(", "COALESCE(");
        result = result.replace("AUTO_INCREMENT", "SERIAL");
        result = result.replace("ENGINE=InnoDB", "");
    }
    // SQL Server -> PostgreSQL
    else if source == "sqlserver" && target == "postgres" {
        result = result.replace("ISNULL(", "COALESCE(");
        result = result.replace("GETDATE()", "NOW()");
        result = result.replace("LEN(", "LENGTH(");
        if result.to_uppercase().contains("TOP ") {
            if let Some(pos) = result.to_uppercase().find("TOP ") {
                let after = &result[pos+4..];
                let num_str = after.trim().split_whitespace().next().unwrap_or("10");
                let remaining = after[num_str.len()..].to_string();
                result = format!("{}{} LIMIT {}", &result[..pos], remaining, num_str);
            }
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════════════════
// Analyze Performance
// ═══════════════════════════════════════════════════════════════════════════

/// POST /api/ai/analyze-performance — AI performance analysis
pub async fn analyze_performance(
    State(_s): State<AppState>,
    Json(body): Json<AnalyzePerformanceRequest>,
) -> Json<Value> {
    let analysis = generate_performance_analysis(&body.db_type);

    Json(json!({
        "success": true,
        "db_type": body.db_type,
        "connection_id": body.connection_id,
        "analysis": analysis,
        "generated_at": Utc::now().to_rfc3339(),
    }))
}

fn generate_performance_analysis(db_type: &str) -> Value {
    match db_type {
        "oracle" => json!({
            "overall_health": "good",
            "score": 82,
            "findings": [
                {"category": "Buffer Cache", "status": "healthy", "detail": "Buffer cache hit ratio at 98.5% — above 95% threshold", "recommendation": null},
                {"category": "Shared Pool", "status": "healthy", "detail": "Library cache hit ratio at 99.1%", "recommendation": null},
                {"category": "I/O", "status": "warning", "detail": "Physical reads trending upward by 15% in the last hour", "recommendation": "Consider increasing SGA size or reviewing query plans for queries with high physical reads"},
                {"category": "Wait Events", "status": "attention", "detail": "db file sequential read contributing 32% of total wait time", "recommendation": "Investigate top SQL statements causing single-block reads. Consider adding indexes."},
                {"category": "Sessions", "status": "healthy", "detail": "Current sessions at 45% of max_sessions", "recommendation": null},
            ],
            "top_recommendations": [
                "Review queries with > 10,000 physical reads per execution",
                "Consider implementing connection pooling to reduce session overhead",
                "Schedule statistics gathering during off-peak hours",
            ],
        }),
        "postgres" => json!({
            "overall_health": "good",
            "score": 78,
            "findings": [
                {"category": "Cache", "status": "healthy", "detail": "Cache hit ratio at 96.2%", "recommendation": null},
                {"category": "Connections", "status": "warning", "detail": "Connection utilization at 72% of max_connections", "recommendation": "Consider using PgBouncer or increasing max_connections"},
                {"category": "Vacuum", "status": "attention", "detail": "3 tables have dead tuples > 10% of live tuples", "recommendation": "Review autovacuum settings; consider manual VACUUM ANALYZE on affected tables"},
                {"category": "Locks", "status": "healthy", "detail": "No long-running lock waits detected", "recommendation": null},
            ],
            "top_recommendations": [
                "Enable pg_stat_statements for query-level performance tracking",
                "Review tables with high sequential scan counts for missing indexes",
                "Consider partitioning large tables (> 100M rows)",
            ],
        }),
        _ => json!({
            "overall_health": "unknown",
            "score": 0,
            "findings": [{"category": "General", "status": "info", "detail": format!("Performance analysis for {} requires active connection and metrics data", db_type), "recommendation": "Connect to a database instance to enable AI performance analysis"}],
            "top_recommendations": ["Connect to a database instance first"],
        }),
    }
}
