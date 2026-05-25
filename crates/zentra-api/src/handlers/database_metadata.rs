//! Database Schema Metadata Handler (P10)
//!
//! Endpoints:
//!   GET /api/db/:db_type/schemas     — list schemas
//!   GET /api/db/:db_type/tables      — list tables
//!   GET /api/db/:db_type/columns     — list columns for a table
//!   GET /api/db/:db_type/indexes     — list indexes for a table

use axum::{extract::{Path, Query, State}, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct SchemaQuery {
    pub connection_id: Option<String>,
}

#[derive(Deserialize)]
pub struct TableQuery {
    pub connection_id: Option<String>,
    pub schema: Option<String>,
}

#[derive(Deserialize)]
pub struct ColumnQuery {
    pub connection_id: Option<String>,
    pub schema: Option<String>,
    pub table: Option<String>,
}

/// GET /api/db/:db_type/schemas — list schemas
pub async fn list_schemas(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<SchemaQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));

    // Try real Oracle query
    if db_type == "oracle" {
        if let Some(result) = query_oracle_metadata(&conn_id, "SELECT DISTINCT owner AS schema_name FROM all_tables ORDER BY owner") {
            return Json(json!({ "schemas": result, "db_type": db_type }));
        }
    }

    // No live connection available — return empty
    Json(json!({
        "db_type": db_type,
        "connection_id": conn_id,
        "schemas": [],
    }))
}

/// GET /api/db/:db_type/tables — list tables
pub async fn list_tables(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    // Try real Oracle query
    if db_type == "oracle" {
        let sql = format!("SELECT table_name, num_rows, last_analyzed FROM all_tables WHERE owner = '{}' ORDER BY table_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "tables": result, "schema": schema, "db_type": db_type }));
        }
    }

    // No live connection available — return empty
    Json(json!({
        "db_type": db_type,
        "connection_id": conn_id,
        "schema": schema,
        "tables": [],
    }))
}

/// GET /api/db/:db_type/columns — list columns for a table
pub async fn list_columns(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<ColumnQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));
    let table = params.table.unwrap_or_else(|| "UNKNOWN".into());

    // Try real Oracle query
    if db_type == "oracle" {
        let sql = format!(
            "SELECT column_name, data_type, data_length, nullable FROM all_tab_columns WHERE owner = '{}' AND table_name = '{}' ORDER BY column_id",
            schema.to_uppercase(), table.to_uppercase()
        );
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "columns": result, "table": table, "schema": schema, "db_type": db_type }));
        }
    }

    // No live connection available — return empty
    Json(json!({
        "db_type": db_type,
        "connection_id": conn_id,
        "schema": schema,
        "table": table,
        "columns": [],
    }))
}

/// GET /api/db/:db_type/indexes — list indexes
pub async fn list_indexes(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<ColumnQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));
    let table = params.table.unwrap_or_else(|| "UNKNOWN".into());

    Json(json!({
        "db_type": db_type,
        "connection_id": conn_id,
        "schema": schema,
        "table": table,
        "indexes": [],
    }))
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn default_schema(db_type: &str) -> String {
    match db_type {
        "oracle" => "HR".into(),
        "postgres" => "public".into(),
        "mysql" => "app_db".into(),
        "mongodb" => "app_db".into(),
        "sqlserver" => "dbo".into(),
        _ => "default".into(),
    }
}

fn query_oracle_metadata(conn_id: &str, sql: &str) -> Option<Vec<Value>> {
    use database_plugin::oracle_connection::{pool_is_connected, pool_query};

    if !pool_is_connected(conn_id) { return None; }

    match pool_query(conn_id, sql) {
        Ok(qr) if !qr.columns.is_empty() => {
            let cols = &qr.columns;
            let rows: Vec<Value> = qr.rows.iter().map(|row| {
                let mut obj = serde_json::Map::new();
                for (i, col) in cols.iter().enumerate() {
                    obj.insert(col.to_lowercase(), json!(row.get(i).cloned().unwrap_or_default()));
                }
                Value::Object(obj)
            }).collect();
            Some(rows)
        }
        _ => None,
    }
}

