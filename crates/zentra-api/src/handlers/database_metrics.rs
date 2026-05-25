//! Universal Database Metric Definitions Handler (P9)
//!
//! CRUD for metric definitions across all database types:
//! PostgreSQL, MySQL, MongoDB, SQL Server.
//! Each DB type loads its own default TOML and stores custom metrics in-memory.

use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

use crate::state::AppState;

// ═══════════════════════════════════════════════════════════════════════════
// Metric Definition Model (shared across all DB types)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDef {
    pub id: String,
    pub db_type: String,
    pub context: String,
    pub labels: Vec<String>,
    pub metrics_desc: HashMap<String, String>,
    pub metrics_type: HashMap<String, String>,
    pub request: String,
    pub ignore_zero_result: bool,
    pub query_timeout: Option<u32>,
    pub scrape_interval: Option<u32>,
    pub is_default: bool,
    pub is_active: bool,
    pub source_file: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// In-memory Store per DB type
// ═══════════════════════════════════════════════════════════════════════════

fn db_metric_defs() -> &'static RwLock<HashMap<String, Vec<MetricDef>>> {
    static INSTANCE: std::sync::OnceLock<RwLock<HashMap<String, Vec<MetricDef>>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert("postgres".into(), load_toml_defaults("postgres", include_str!("../../../../plugins/database-plugin/src/postgres_default_metrics.toml")));
        map.insert("mysql".into(), load_toml_defaults("mysql", include_str!("../../../../plugins/database-plugin/src/mysql_default_metrics.toml")));
        map.insert("mongodb".into(), load_toml_defaults("mongodb", include_str!("../../../../plugins/database-plugin/src/mongodb_default_metrics.toml")));
        map.insert("sqlserver".into(), load_toml_defaults("sqlserver", include_str!("../../../../plugins/database-plugin/src/sqlserver_default_metrics.toml")));
        RwLock::new(map)
    })
}

#[derive(Deserialize)]
struct TomlMetricFile {
    metric: Vec<TomlMetric>,
}

#[derive(Deserialize)]
struct TomlMetric {
    context: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    metrics_desc: HashMap<String, String>,
    #[serde(default)]
    metrics_type: HashMap<String, String>,
    request: String,
    #[serde(default)]
    ignore_zero_result: bool,
    query_timeout: Option<u32>,
    scrape_interval: Option<u32>,
}

fn load_toml_defaults(db_type: &str, content: &str) -> Vec<MetricDef> {
    let now = Utc::now().to_rfc3339();
    let parsed: TomlMetricFile = match toml::from_str(content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse {} default metrics TOML: {}", db_type, e);
            return vec![];
        }
    };

    parsed.metric.into_iter().enumerate().map(|(i, m)| MetricDef {
        id: format!("{}-default-{}", db_type, i),
        db_type: db_type.to_string(),
        context: m.context,
        labels: m.labels,
        metrics_desc: m.metrics_desc,
        metrics_type: m.metrics_type,
        request: m.request.trim().to_string(),
        ignore_zero_result: m.ignore_zero_result,
        query_timeout: m.query_timeout,
        scrape_interval: m.scrape_interval,
        is_default: true,
        is_active: true,
        source_file: Some(format!("{}_default_metrics.toml", db_type)),
        created_at: now.clone(),
        updated_at: now.clone(),
    }).collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

/// GET /api/:db_type/metrics/definitions — list all metrics for a DB type
pub async fn list_definitions(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
) -> Json<Value> {
    let store = db_metric_defs().read();
    let defs = store.get(&db_type).cloned().unwrap_or_default();
    let active = defs.iter().filter(|d| d.is_active).count();
    Json(json!({
        "db_type": db_type,
        "total": defs.len(),
        "active": active,
        "definitions": defs,
    }))
}

/// GET /api/:db_type/metrics/definitions/:id — get single metric
pub async fn get_definition(
    State(_s): State<AppState>,
    Path((db_type, id)): Path<(String, String)>,
) -> Json<Value> {
    let store = db_metric_defs().read();
    if let Some(defs) = store.get(&db_type) {
        if let Some(def) = defs.iter().find(|d| d.id == id) {
            return Json(json!(def));
        }
    }
    Json(json!({"error": "Not found", "db_type": db_type, "id": id}))
}

/// POST /api/:db_type/metrics/definitions — add custom metric
pub async fn create_definition(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let now = Utc::now().to_rfc3339();
    let id = format!("{}-custom-{}", db_type, Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));

    let def = MetricDef {
        id: id.clone(),
        db_type: db_type.clone(),
        context: body.get("context").and_then(|v| v.as_str()).unwrap_or("custom").to_string(),
        labels: body.get("labels").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        metrics_desc: body.get("metrics_desc").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        metrics_type: body.get("metrics_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
        request: body.get("request").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        ignore_zero_result: body.get("ignore_zero_result").and_then(|v| v.as_bool()).unwrap_or(false),
        query_timeout: body.get("query_timeout").and_then(|v| v.as_u64()).map(|v| v as u32),
        scrape_interval: body.get("scrape_interval").and_then(|v| v.as_u64()).map(|v| v as u32),
        is_default: false,
        is_active: true,
        source_file: None,
        created_at: now.clone(),
        updated_at: now,
    };

    db_metric_defs().write().entry(db_type).or_default().push(def.clone());

    Json(json!({"created": true, "id": id, "definition": def}))
}

/// PUT /api/:db_type/metrics/definitions/:id — update metric
pub async fn update_definition(
    State(_s): State<AppState>,
    Path((db_type, id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let now = Utc::now().to_rfc3339();
    let mut store = db_metric_defs().write();
    if let Some(defs) = store.get_mut(&db_type) {
        if let Some(def) = defs.iter_mut().find(|d| d.id == id) {
            if let Some(v) = body.get("context").and_then(|v| v.as_str()) { def.context = v.to_string(); }
            if let Some(v) = body.get("request").and_then(|v| v.as_str()) { def.request = v.to_string(); }
            if let Some(v) = body.get("labels") { def.labels = serde_json::from_value(v.clone()).unwrap_or_default(); }
            if let Some(v) = body.get("is_active").and_then(|v| v.as_bool()) { def.is_active = v; }
            if let Some(v) = body.get("query_timeout").and_then(|v| v.as_u64()) { def.query_timeout = Some(v as u32); }
            if let Some(v) = body.get("scrape_interval").and_then(|v| v.as_u64()) { def.scrape_interval = Some(v as u32); }
            def.updated_at = now;
            return Json(json!({"updated": true, "definition": def.clone()}));
        }
    }
    Json(json!({"error": "Not found"}))
}

/// DELETE /api/:db_type/metrics/definitions/:id — delete custom metric (can't delete defaults)
pub async fn delete_definition(
    State(_s): State<AppState>,
    Path((db_type, id)): Path<(String, String)>,
) -> Json<Value> {
    let mut store = db_metric_defs().write();
    if let Some(defs) = store.get_mut(&db_type) {
        if let Some(pos) = defs.iter().position(|d| d.id == id) {
            if defs[pos].is_default {
                return Json(json!({"error": "Cannot delete default metric. Deactivate it instead."}));
            }
            defs.remove(pos);
            return Json(json!({"deleted": true, "id": id}));
        }
    }
    Json(json!({"error": "Not found"}))
}

/// POST /api/:db_type/metrics/test — test-execute a metric definition
pub async fn test_definition(
    State(_s): State<AppState>,
    Path(db_type): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let sql = body.get("request").and_then(|v| v.as_str()).unwrap_or("");
    let context = body.get("context").and_then(|v| v.as_str()).unwrap_or("test");

    // Simulate test execution — in production, would run against a real connection
    Json(json!({
        "db_type": db_type,
        "context": context,
        "sql": sql,
        "success": true,
        "duration_ms": 45,
        "rows_returned": 5,
        "columns": ["col1", "col2", "value"],
        "sample_data": [
            {"col1": "sample", "col2": "data", "value": 42},
        ],
        "tested_at": Utc::now().to_rfc3339(),
    }))
}
