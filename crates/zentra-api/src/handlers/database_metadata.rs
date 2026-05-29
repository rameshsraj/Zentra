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
    let table = params.table.clone().unwrap_or_default();

    if db_type == "oracle" {
        let sql = if table.is_empty() {
            format!("SELECT index_name, table_name, uniqueness, index_type, status FROM all_indexes WHERE owner = '{}' ORDER BY index_name", schema.to_uppercase())
        } else {
            format!("SELECT index_name, table_name, uniqueness, index_type, status FROM all_indexes WHERE owner = '{}' AND table_name = '{}' ORDER BY index_name", schema.to_uppercase(), table.to_uppercase())
        };
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "indexes": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({
        "db_type": db_type,
        "connection_id": conn_id,
        "schema": schema,
        "indexes": [],
    }))
}

/// GET /api/db/:db_type/views — list views
pub async fn list_views(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT view_name, text_length FROM all_views WHERE owner = '{}' ORDER BY view_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "views": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "views": [] }))
}

/// GET /api/db/:db_type/sequences — list sequences
pub async fn list_sequences(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT sequence_name, min_value, max_value, increment_by, last_number, cache_size FROM all_sequences WHERE sequence_owner = '{}' ORDER BY sequence_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "sequences": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "sequences": [] }))
}

/// GET /api/db/:db_type/triggers — list triggers
pub async fn list_triggers(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT trigger_name, trigger_type, triggering_event, table_name, status FROM all_triggers WHERE owner = '{}' ORDER BY trigger_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "triggers": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "triggers": [] }))
}

/// GET /api/db/:db_type/types — list types
pub async fn list_types(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT type_name, typecode, attributes, methods FROM all_types WHERE owner = '{}' ORDER BY type_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "types": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "types": [] }))
}

/// GET /api/db/:db_type/procedures — list procedures and functions
pub async fn list_procedures(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT object_name, object_type, status, created, last_ddl_time FROM all_objects WHERE owner = '{}' AND object_type IN ('PROCEDURE','FUNCTION','PACKAGE') ORDER BY object_type, object_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "procedures": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "procedures": [] }))
}

/// GET /api/db/:db_type/mviews — list materialized views
pub async fn list_mviews(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT mview_name, refresh_mode, refresh_method, last_refresh_date, staleness FROM all_mviews WHERE owner = '{}' ORDER BY mview_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "mviews": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "mviews": [] }))
}

/// GET /api/db/:db_type/synonyms — list synonyms
pub async fn list_synonyms(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT synonym_name, table_owner, table_name, db_link FROM all_synonyms WHERE owner = '{}' ORDER BY synonym_name", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "synonyms": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "synonyms": [] }))
}

/// GET /api/db/:db_type/schema-objects — summary counts of all object types in a schema
pub async fn schema_objects(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Query(params): Query<TableQuery>,
) -> Json<Value> {
    let conn_id = params.connection_id.unwrap_or_else(|| format!("{}-default", db_type));
    let schema = params.schema.unwrap_or_else(|| default_schema(&db_type));

    if db_type == "oracle" {
        let sql = format!("SELECT object_type, COUNT(*) AS cnt FROM all_objects WHERE owner = '{}' AND object_type IN ('TABLE','VIEW','INDEX','SEQUENCE','TRIGGER','TYPE','PROCEDURE','FUNCTION','PACKAGE','MATERIALIZED VIEW','SYNONYM') GROUP BY object_type ORDER BY object_type", schema.to_uppercase());
        if let Some(result) = query_oracle_metadata(&conn_id, &sql) {
            return Json(json!({ "object_counts": result, "schema": schema, "db_type": db_type }));
        }
    }

    Json(json!({ "db_type": db_type, "schema": schema, "object_counts": [] }))
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

