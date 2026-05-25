//! SQL Workspace — Query Execution & History Handler (P10)
//!
//! Endpoints:
//!   POST /api/db/execute   — execute SQL query
//!   POST /api/db/explain   — execute EXPLAIN/EXPLAIN PLAN
//!   GET  /api/db/history    — query history
//!   DELETE /api/db/history/:id — delete history entry

use axum::{extract::{Path, Query, State}, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::state::AppState;

// ═══════════════════════════════════════════════════════════════════════════
// Query History Store (in-memory, production would use DB)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: String,
    pub db_type: String,
    pub connection_id: String,
    pub sql_text: String,
    pub row_count: Option<u64>,
    pub execution_time_ms: u64,
    pub status: String,
    pub error_message: Option<String>,
    pub executed_by: String,
    pub executed_at: String,
}

fn query_history() -> &'static RwLock<Vec<QueryHistoryEntry>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<QueryHistoryEntry>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(Vec::new()))
}

// ═══════════════════════════════════════════════════════════════════════════
// Execute Query
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct ExecuteQueryRequest {
    pub db_type: String,
    pub connection_id: Option<String>,
    pub sql: String,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// POST /api/db/execute — execute SQL query
pub async fn execute_query(
    State(_s): State<AppState>,
    Json(body): Json<ExecuteQueryRequest>,
) -> Json<Value> {
    let start = std::time::Instant::now();
    let sql = body.sql.trim().to_string();
    let conn_id = body.connection_id.clone().unwrap_or_else(|| format!("{}-default", body.db_type));

    // Safety check: reject DDL unless it's a SELECT / WITH / EXPLAIN
    let upper = sql.to_uppercase();
    let is_readonly = upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("EXPLAIN")
        || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("DESC ");
    if !is_readonly {
        return Json(json!({
            "success": false,
            "error": "Only read-only queries (SELECT, WITH, EXPLAIN, SHOW, DESCRIBE) are allowed in SQL Workspace.",
            "sql": sql,
        }));
    }

    // Try real execution via Oracle pool if available, fallback to sqlplus
    let limit = body.limit.unwrap_or(100);
    let (columns, rows, row_count, exec_ms, error) = if body.db_type == "oracle" {
        let pool_result = execute_via_oracle(&conn_id, &sql, limit);
        if pool_result.4.as_deref() == Some("No live connection available for this database type") {
            // Native pool not available — fall back to sqlplus
            execute_via_sqlplus(&conn_id, &sql, limit).await
        } else {
            pool_result
        }
    } else {
        execute_demo(&body.db_type, &sql, limit)
    };

    let duration = start.elapsed().as_millis() as u64;

    // Record in history
    let entry = QueryHistoryEntry {
        id: Uuid::new_v4().to_string(),
        db_type: body.db_type.clone(),
        connection_id: conn_id.clone(),
        sql_text: sql.clone(),
        row_count: Some(row_count),
        execution_time_ms: exec_ms.unwrap_or(duration),
        status: if error.is_some() { "error".into() } else { "success".into() },
        error_message: error.clone(),
        executed_by: "current_user".into(),
        executed_at: Utc::now().to_rfc3339(),
    };
    {
        let mut history = query_history().write();
        history.insert(0, entry);
        if history.len() > 500 { history.truncate(500); }
    }

    if let Some(err) = error {
        Json(json!({
            "success": false,
            "error": err,
            "sql": sql,
            "execution_time_ms": duration,
        }))
    } else {
        Json(json!({
            "success": true,
            "columns": columns,
            "rows": rows,
            "row_count": row_count,
            "execution_time_ms": exec_ms.unwrap_or(duration),
            "db_type": body.db_type,
            "connection_id": conn_id,
        }))
    }
}

fn execute_via_oracle(conn_id: &str, sql: &str, limit: u32) -> (Vec<String>, Vec<Vec<String>>, u64, Option<u64>, Option<String>) {
    use database_plugin::oracle_connection::{pool_is_connected, pool_query};

    if !pool_is_connected(conn_id) {
        return execute_demo("oracle", sql, limit);
    }

    match pool_query(conn_id, sql) {
        Ok(qr) => {
            let row_count = qr.rows.len() as u64;
            let rows: Vec<Vec<String>> = qr.rows.into_iter().take(limit as usize).collect();
            (qr.columns, rows, row_count, Some(qr.duration_ms), None)
        }
        Err(e) => (vec![], vec![], 0, None, Some(e.message))
    }
}

fn execute_demo(_db_type: &str, _sql: &str, _limit: u32) -> (Vec<String>, Vec<Vec<String>>, u64, Option<u64>, Option<String>) {
    (vec![], vec![], 0, None, Some("No live connection available for this database type".into()))
}

/// Execute a query via sqlplus directly with CSV output formatting.
/// Bypasses oracle_local_query to avoid SET HEADING OFF conflicts.
async fn execute_via_sqlplus(conn_id: &str, sql: &str, limit: u32) -> (Vec<String>, Vec<Vec<String>>, u64, Option<u64>, Option<String>) {
    let start = std::time::Instant::now();

    // Look up connection details
    let (host, port, service_name, username, password, role_str) = {
        let store = crate::handlers::oracle::oracle_connections().read();
        match store.iter().find(|c| c.id == conn_id) {
            Some(conn) => (conn.host.clone(), conn.port, conn.service_name.clone(),
                conn.username.clone(), conn.password.clone(), conn.role.clone()),
            None => return (vec![], vec![], 0, None, Some(format!("Connection '{}' not found", conn_id))),
        }
    };

    let role_clause = if role_str.to_uppercase() == "SYSDBA" { " as sysdba" }
        else if role_str.to_uppercase() == "SYSOPER" { " as sysoper" }
        else { "" };
    let conn_str = format!("{}/{}@//{}:{}/{}{}", username, password, host, port, service_name, role_clause);

    let sql_input = format!(
        "SET MARKUP CSV ON\nSET PAGESIZE {}\nSET FEEDBACK OFF\nSET LINESIZE 32767\nSET HEADING ON\n{};\nEXIT;\n",
        limit + 1,
        sql.trim_end_matches(';'),
    );

    let spawn = tokio::process::Command::new("sqlplus")
        .args(["-S", &conn_str])
        .env("LD_LIBRARY_PATH", "/usr/lib/oracle/23/client64/lib")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    match spawn {
        Ok(mut child) => {
            use tokio::io::AsyncWriteExt;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(sql_input.as_bytes()).await;
                drop(stdin);
            }
            match tokio::time::timeout(std::time::Duration::from_secs(30), child.wait_with_output()).await {
                Ok(Ok(out)) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let combined = format!("{}\n{}", stdout, stderr);
                    // Check for connection-level errors
                    if combined.contains("ORA-01017") || combined.contains("ORA-28000")
                        || combined.contains("ORA-12541") || combined.contains("SP2-0157") {
                        let e = combined.lines().find(|l| l.contains("ORA-") || l.contains("SP2-"))
                            .unwrap_or("Oracle connection error").trim().to_string();
                        return (vec![], vec![], 0, Some(elapsed), Some(e));
                    }
                    // Check for query-level errors
                    if let Some(err_line) = stdout.lines().find(|l| l.contains("ORA-") || l.contains("SP2-")) {
                        return (vec![], vec![], 0, Some(elapsed), Some(err_line.trim().to_string()));
                    }
                    // Parse CSV output
                    let mut lines = stdout.lines().filter(|l| !l.trim().is_empty());
                    let columns: Vec<String> = match lines.next() {
                        Some(header) => parse_csv_line(header),
                        None => return (vec![], vec![], 0, Some(elapsed), None),
                    };
                    let rows: Vec<Vec<String>> = lines
                        .take(limit as usize)
                        .map(|line| parse_csv_line(line))
                        .collect();
                    let row_count = rows.len() as u64;
                    (columns, rows, row_count, Some(elapsed), None)
                }
                Ok(Err(e)) => (vec![], vec![], 0, None, Some(format!("sqlplus failed: {}", e))),
                Err(_) => (vec![], vec![], 0, None, Some("Query timed out after 30s".into())),
            }
        }
        Err(e) => (vec![], vec![], 0, None, Some(format!("sqlplus not found: {}", e))),
    }
}

/// Parse a CSV line, handling quoted fields with embedded commas
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ',' if !in_quotes => {
                fields.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// POST /api/db/explain — execute EXPLAIN
pub async fn explain_query(
    State(s): State<AppState>,
    Json(mut body): Json<ExecuteQueryRequest>,
) -> Json<Value> {
    let explain_prefix = match body.db_type.as_str() {
        "oracle" => "EXPLAIN PLAN FOR ",
        "postgres" => "EXPLAIN ANALYZE ",
        "mysql" => "EXPLAIN ",
        "sqlserver" => "SET SHOWPLAN_TEXT ON; ",
        _ => "EXPLAIN ",
    };
    body.sql = format!("{}{}", explain_prefix, body.sql);
    execute_query(State(s), Json(body)).await
}

// ═══════════════════════════════════════════════════════════════════════════
// Query History
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub db_type: Option<String>,
    pub limit: Option<usize>,
}

/// GET /api/db/history — query history
pub async fn list_history(
    State(_s): State<AppState>,
    Query(params): Query<HistoryQuery>,
) -> Json<Value> {
    let history = query_history().read();
    let limit = params.limit.unwrap_or(50);
    let filtered: Vec<_> = history.iter()
        .filter(|e| params.db_type.as_ref().map_or(true, |dt| &e.db_type == dt))
        .take(limit)
        .cloned()
        .collect();
    Json(json!({ "history": filtered, "total": filtered.len() }))
}

/// DELETE /api/db/history/:id — delete history entry
pub async fn delete_history(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut history = query_history().write();
    if let Some(pos) = history.iter().position(|e| e.id == id) {
        history.remove(pos);
        Json(json!({"deleted": true, "id": id}))
    } else {
        Json(json!({"error": "Not found", "id": id}))
    }
}
