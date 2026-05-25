//! Oracle Metric Definitions Handler (V5 Prompt 15)
//! CRUD for metric definitions + upload + test + reload

use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;

use crate::state::AppState;

use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// In-memory Metric Definitions Store
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDef {
    pub id: String,
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

fn metric_defs() -> &'static RwLock<Vec<MetricDef>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<MetricDef>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(default_metrics()))
}

fn default_metrics() -> Vec<MetricDef> {
    let now = Utc::now().to_rfc3339();
    vec![
        md("sessions", &["status","type"], "SELECT status, type, COUNT(*) AS value FROM v$session GROUP BY status, type", &now),
        md("resource", &["resource_name"], "SELECT resource_name, current_utilization, max_utilization, initial_allocation, limit_value FROM v$resource_limit", &now),
        md("asm_diskgroup", &["name"], "SELECT name, total_mb, usable_file_mb AS usable_mb, offline_disks FROM v$asm_diskgroup", &now),
        md("activity", &["name"], "SELECT name, value FROM v$sysstat WHERE name IN ('db block gets from cache','consistent gets from cache','physical reads','physical writes')", &now),
        md("process", &[], "SELECT (SELECT COUNT(*) FROM v$process) AS count, (SELECT value FROM v$parameter WHERE name='processes') AS limit FROM DUAL", &now),
        md("wait_class", &["wait_class"], "SELECT wait_class, SUM(total_waits) AS total_waits, SUM(time_waited) AS time_waited FROM v$system_wait_class WHERE wait_class != 'Idle' GROUP BY wait_class", &now),
        md("tablespace", &["tablespace_name"], "SELECT tablespace_name, bytes, max_bytes, free_bytes, used_percent FROM dba_tablespace_usage_metrics", &now),
        md("db_system", &["name"], "SELECT name, value FROM v$sysmetric WHERE group_id=2", &now),
        md("db_platform", &[], "SELECT db_cpu, host_cpu, db_time FROM v$sysmetric", &now),
        md("top_sql", &["sql_id"], "SELECT sql_id, ROUND(elapsed_time/1e6,2) AS elapsed_time, executions, buffer_gets FROM v$sql ORDER BY elapsed_time DESC FETCH FIRST 20 ROWS ONLY", &now),
        md("cache_hit_ratio", &[], "SELECT buffer_cache_hit, library_cache_hit, dict_cache_hit FROM DUAL", &now),
        md("slow_queries", &["sql_id"], "SELECT sql_id, elapsed_per_exec, executions FROM v$sql WHERE executions>0 ORDER BY elapsed_time/NULLIF(executions,0) DESC FETCH FIRST 20 ROWS ONLY", &now),
        md("big_queries", &["sql_id"], "SELECT sql_id, disk_reads, buffer_gets, rows_processed FROM v$sql ORDER BY disk_reads DESC FETCH FIRST 20 ROWS ONLY", &now),
        md("size_user_segments", &["segment_name","tablespace_name"], "SELECT segment_name, tablespace_name, bytes FROM dba_segments ORDER BY bytes DESC FETCH FIRST 50 ROWS ONLY", &now),
    ]
}

fn md(ctx: &str, labels: &[&str], sql: &str, ts: &str) -> MetricDef {
    MetricDef {
        id: format!("def-{}", ctx),
        context: ctx.into(),
        labels: labels.iter().map(|s| s.to_string()).collect(),
        metrics_desc: HashMap::from([("value".into(), format!("{} metric", ctx))]),
        metrics_type: HashMap::from([("value".into(), "gauge".into())]),
        request: sql.into(),
        ignore_zero_result: false,
        query_timeout: Some(30),
        scrape_interval: Some(60),
        is_default: true,
        is_active: true,
        source_file: Some("default-metrics.toml".into()),
        created_at: ts.into(),
        updated_at: ts.into(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

/// GET /api/oracle/metrics — list all (default + custom)
pub async fn list_metrics(State(_s): State<AppState>) -> Json<Value> {
    let store = metric_defs().read();
    let defaults = store.iter().filter(|m| m.is_default).count();
    let custom = store.iter().filter(|m| !m.is_default).count();
    Json(json!({
        "metrics": *store,
        "total": store.len(),
        "defaults": defaults,
        "custom": custom,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateMetricRequest {
    pub context: String,
    pub labels: Option<Vec<String>>,
    pub metrics_desc: Option<HashMap<String, String>>,
    pub metrics_type: Option<HashMap<String, String>>,
    pub request: String,
    pub ignore_zero_result: Option<bool>,
    pub query_timeout: Option<u32>,
    pub scrape_interval: Option<u32>,
}

/// POST /api/oracle/metrics — create custom metric
pub async fn create_metric(
    State(_s): State<AppState>,
    Json(req): Json<CreateMetricRequest>,
) -> Json<MetricDef> {
    let now = Utc::now().to_rfc3339();
    let def = MetricDef {
        id: Uuid::new_v4().to_string(),
        context: req.context,
        labels: req.labels.unwrap_or_default(),
        metrics_desc: req.metrics_desc.unwrap_or_default(),
        metrics_type: req.metrics_type.unwrap_or_default(),
        request: req.request,
        ignore_zero_result: req.ignore_zero_result.unwrap_or(false),
        query_timeout: req.query_timeout,
        scrape_interval: req.scrape_interval,
        is_default: false,
        is_active: true,
        source_file: None,
        created_at: now.clone(),
        updated_at: now,
    };
    metric_defs().write().push(def.clone());
    Json(def)
}

/// GET /api/oracle/metrics/:id — detail
pub async fn get_metric(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<MetricDef>, ApiError> {
    metric_defs().read().iter().find(|m| m.id == id).cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("Metric not found".into()))
}

#[derive(Debug, Deserialize)]
pub struct UpdateMetricRequest {
    pub request: Option<String>,
    pub labels: Option<Vec<String>>,
    pub query_timeout: Option<u32>,
    pub scrape_interval: Option<u32>,
    pub is_active: Option<bool>,
}

/// PUT /api/oracle/metrics/:id — update (defaults: enable/disable only)
pub async fn update_metric(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMetricRequest>,
) -> Result<Json<MetricDef>, ApiError> {
    let mut store = metric_defs().write();
    if let Some(m) = store.iter_mut().find(|m| m.id == id) {
        if m.is_default {
            // Defaults: only toggle active
            if let Some(a) = req.is_active { m.is_active = a; }
        } else {
            if let Some(r) = req.request { m.request = r; }
            if let Some(l) = req.labels { m.labels = l; }
            if let Some(t) = req.query_timeout { m.query_timeout = Some(t); }
            if let Some(s) = req.scrape_interval { m.scrape_interval = Some(s); }
            if let Some(a) = req.is_active { m.is_active = a; }
        }
        m.updated_at = Utc::now().to_rfc3339();
        Ok(Json(m.clone()))
    } else {
        Err(ApiError::NotFound("Metric not found".into()))
    }
}

/// DELETE /api/oracle/metrics/:id — delete custom only
pub async fn delete_metric(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let mut store = metric_defs().write();
    if let Some(m) = store.iter().find(|m| m.id == id) {
        if m.is_default {
            return Err(ApiError::BadRequest("Cannot delete default metrics; disable instead".into()));
        }
    }
    store.retain(|m| m.id != id);
    Json(json!({"deleted": id}));
    Ok(Json(json!({"deleted": id})))
}

/// POST /api/oracle/metrics/upload — upload TOML/YAML as JSON body
#[derive(Debug, Deserialize)]
pub struct UploadMetricsRequest {
    pub content: String,
    pub filename: Option<String>,
}

pub async fn upload_metrics(
    State(_s): State<AppState>,
    Json(req): Json<UploadMetricsRequest>,
) -> Json<Value> {
    let filename = req.filename.unwrap_or_else(|| "upload.toml".into());
    let now = Utc::now().to_rfc3339();
    let mut imported = 0u32;

    if req.content.contains("[[metric]]") {
        let mut store = metric_defs().write();
        for block in req.content.split("[[metric]]").skip(1) {
            let context = extract_field(block, "context").unwrap_or_else(|| "custom".into());
            let request = extract_field(block, "request").unwrap_or_default();
            if !request.is_empty() {
                store.push(MetricDef {
                    id: Uuid::new_v4().to_string(),
                    context: context.clone(),
                    labels: Vec::new(),
                    metrics_desc: HashMap::from([("value".into(), format!("{} metric", context))]),
                    metrics_type: HashMap::from([("value".into(), "gauge".into())]),
                    request,
                    ignore_zero_result: false,
                    query_timeout: Some(30),
                    scrape_interval: Some(60),
                    is_default: false,
                    is_active: true,
                    source_file: Some(filename.clone()),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                });
                imported += 1;
            }
        }
    }
    Json(json!({"imported": imported}))
}

fn extract_field(block: &str, field: &str) -> Option<String> {
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(field) {
            if let Some(val) = trimmed.split('=').nth(1) {
                return Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
    None
}

/// POST /api/oracle/metrics/:id/test — execute against connection, preview
pub async fn test_metric(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = metric_defs().read();
    let def = store.iter().find(|m| m.id == id)
        .ok_or_else(|| ApiError::NotFound("Metric not found".into()))?;

    Ok(Json(json!({
        "metric_id": id,
        "context": def.context,
        "sql": def.request,
        "sample_rows": [],
        "row_count": 0,
        "duration_ms": 0,
        "executed_at": Utc::now().to_rfc3339(),
        "note": "Connect an Oracle target to execute this metric",
    })))
}

/// POST /api/oracle/metrics/reload — hot reload defaults
pub async fn reload_metrics(State(_s): State<AppState>) -> Json<Value> {
    let mut store = metric_defs().write();
    let custom: Vec<MetricDef> = store.iter().filter(|m| !m.is_default).cloned().collect();
    let mut new_defs = default_metrics();
    new_defs.extend(custom);
    let total = new_defs.len();
    *store = new_defs;
    Json(json!({"reloaded": true, "total": total}))
}
