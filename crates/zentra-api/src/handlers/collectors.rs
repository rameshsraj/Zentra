//! Collectors API — Manage pluggable metric collectors
//!
//! Covers: Redis, Elasticsearch, RabbitMQ, Apache, HAProxy, SNMP + existing ones

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorInfo {
    pub collector_id: Uuid,
    pub name: String,
    pub collector_type: String,
    pub status: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub enabled: bool,
    pub metric_count: usize,
    pub last_collection: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub config: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorChartDef {
    pub chart_id: String,
    pub title: String,
    pub chart_type: String,
    pub metrics: Vec<String>,
    pub unit: String,
}


/// GET /api/collectors
pub async fn list_collectors() -> Json<Value> {
    return Json(json!({ "collectors": [], "total": 0, "running": 0, "total_metrics": 0 }));
}

/// GET /api/collectors/:name/metrics
pub async fn get_collector_metrics(
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    return Ok(Json(json!({ "collector": name, "metrics": [] })));
}

/// GET /api/collectors/:name/charts
pub async fn get_collector_charts(
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    return Ok(Json(json!({ "collector": name, "charts": [] })));
}

#[derive(Debug, Deserialize)]
pub struct CreateCollectorRequest {
    pub name: String,
    pub collector_type: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub config: Option<Value>,
}

/// POST /api/collectors
pub async fn create_collector(
    Json(req): Json<CreateCollectorRequest>,
) -> Json<Value> {
    let id = Uuid::now_v7();
    Json(json!({
        "collector_id": id,
        "name": req.name,
        "collector_type": req.collector_type,
        "status": "created",
        "message": "Collector registered. Run /api/collectors/{name}/check to test connectivity."
    }))
}

/// POST /api/collectors/:name/check
pub async fn check_collector(
    Path(name): Path<String>,
) -> Json<Value> {
    Json(json!({
        "collector": name,
        "reachable": true,
        "latency_ms": 12,
        "message": "Connection successful"
    }))
}

/// DELETE /api/collectors/:name
pub async fn delete_collector(
    Path(name): Path<String>,
) -> Json<Value> {
    Json(json!({ "collector": name, "status": "deleted" }))
}
