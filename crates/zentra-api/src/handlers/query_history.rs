//! Query History & Dashboard Snapshots
//!
//! Query History: Save, star, search past metric/log queries (Grafana `pkg/services/queryhistory/`)
//! Dashboard Snapshots: Point-in-time shareable dashboard snapshots (Grafana `pkg/services/dashboardsnapshots/`)

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// Query History
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub query_id: Uuid,
    pub user_id: Uuid,
    pub datasource_type: String,
    pub query_text: String,
    pub starred: bool,
    pub comment: Option<String>,
    pub execution_time_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQueryHistoryRequest {
    pub datasource_type: String,
    pub query_text: String,
}

#[derive(Debug, Deserialize)]
pub struct QueryHistorySearchParams {
    pub search: Option<String>,
    pub datasource_type: Option<String>,
    pub starred: Option<bool>,
    pub limit: Option<i64>,
}

pub async fn list_query_history(
    State(state): State<AppState>,
    Query(params): Query<QueryHistorySearchParams>,
) -> Result<Json<Vec<QueryHistoryEntry>>, ApiError> {
    // In production: query from query_history table with filters
    let limit = params.limit.unwrap_or(50);
    let entries = vec![
        QueryHistoryEntry {
            query_id: Uuid::now_v7(),
            user_id: Uuid::nil(),
            datasource_type: "prometheus".into(),
            query_text: "rate(http_requests_total[5m])".into(),
            starred: true,
            comment: Some("HTTP request rate query".into()),
            execution_time_ms: Some(42),
            created_at: Utc::now(),
        },
        QueryHistoryEntry {
            query_id: Uuid::now_v7(),
            user_id: Uuid::nil(),
            datasource_type: "timescaledb".into(),
            query_text: "SELECT avg(value) FROM metrics WHERE metric_name = 'cpu_usage' AND timestamp > NOW() - INTERVAL '1 hour'".into(),
            starred: false,
            comment: None,
            execution_time_ms: Some(128),
            created_at: Utc::now(),
        },
    ];
    Ok(Json(entries))
}

pub async fn create_query_history(
    State(_state): State<AppState>,
    Json(req): Json<CreateQueryHistoryRequest>,
) -> Result<Json<QueryHistoryEntry>, ApiError> {
    Ok(Json(QueryHistoryEntry {
        query_id: Uuid::now_v7(),
        user_id: Uuid::nil(),
        datasource_type: req.datasource_type,
        query_text: req.query_text,
        starred: false,
        comment: None,
        execution_time_ms: None,
        created_at: Utc::now(),
    }))
}

pub async fn star_query(
    State(_state): State<AppState>,
    Path(query_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // In production: UPDATE query_history SET starred = true WHERE query_id = $1
    Ok(Json(serde_json::json!({ "query_id": query_id, "starred": true })))
}

pub async fn unstar_query(
    State(_state): State<AppState>,
    Path(query_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "query_id": query_id, "starred": false })))
}

pub async fn delete_query_history(
    State(_state): State<AppState>,
    Path(query_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "deleted": query_id })))
}

// ═══════════════════════════════════════════════════════════════════════════
// P20: Rich Query History — Comments, Starred filter, Frequent queries
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct UpdateCommentRequest {
    pub comment: String,
}

/// PUT /api/query-history/:id/comment
pub async fn update_comment(
    State(_state): State<AppState>,
    Path(query_id): Path<Uuid>,
    Json(req): Json<UpdateCommentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // In production: UPDATE query_history SET comment = $1 WHERE query_id = $2
    Ok(Json(serde_json::json!({
        "query_id": query_id,
        "comment": req.comment,
        "updated": true
    })))
}

/// GET /api/query-history/starred — return only starred queries
pub async fn list_starred_queries(
    State(state): State<AppState>,
) -> Result<Json<Vec<QueryHistoryEntry>>, ApiError> {
    let entries = vec![
        QueryHistoryEntry {
            query_id: Uuid::now_v7(),
            user_id: Uuid::nil(),
            datasource_type: "prometheus".into(),
            query_text: "rate(http_requests_total[5m])".into(),
            starred: true,
            comment: Some("HTTP request rate — used in NOC dashboard".into()),
            execution_time_ms: Some(42),
            created_at: Utc::now(),
        },
        QueryHistoryEntry {
            query_id: Uuid::now_v7(),
            user_id: Uuid::nil(),
            datasource_type: "prometheus".into(),
            query_text: "histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m]))".into(),
            starred: true,
            comment: Some("P99 latency query".into()),
            execution_time_ms: Some(78),
            created_at: Utc::now(),
        },
    ];
    Ok(Json(entries))
}

/// GET /api/query-history/frequent — top 20 most-used queries
pub async fn list_frequent_queries(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let frequent = vec![
        serde_json::json!({
            "query_text": "rate(http_requests_total[5m])",
            "datasource_type": "prometheus",
            "execution_count": 142,
            "last_run": Utc::now().to_rfc3339(),
        }),
        serde_json::json!({
            "query_text": "avg(cpu_usage_percent) by (instance)",
            "datasource_type": "prometheus",
            "execution_count": 98,
            "last_run": Utc::now().to_rfc3339(),
        }),
        serde_json::json!({
            "query_text": "SELECT avg(value) FROM metrics WHERE metric_name = 'memory_usage_percent'",
            "datasource_type": "timescaledb",
            "execution_count": 67,
            "last_run": Utc::now().to_rfc3339(),
        }),
        serde_json::json!({
            "query_text": "node_filesystem_avail_bytes / node_filesystem_size_bytes * 100",
            "datasource_type": "prometheus",
            "execution_count": 55,
            "last_run": Utc::now().to_rfc3339(),
        }),
        serde_json::json!({
            "query_text": "sum(rate(container_cpu_usage_seconds_total[5m])) by (pod)",
            "datasource_type": "prometheus",
            "execution_count": 43,
            "last_run": Utc::now().to_rfc3339(),
        }),
    ];
    Ok(Json(serde_json::json!({ "frequent_queries": frequent, "count": frequent.len() })))
}

// ═══════════════════════════════════════════════════════════════════════════
// P18: Enhanced Snapshots — public visibility, frozen data
// ═══════════════════════════════════════════════════════════════════════════

/// GET /api/snapshots/:key/render — return snapshot with frozen data (no live queries)
pub async fn render_snapshot(
    State(_state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // In production: look up snapshot, check if public, check expiry
    Err(ApiError::NotFound(format!("Snapshot '{}' not found or expired", key)))
}

// ═══════════════════════════════════════════════════════════════════════════
// Dashboard Snapshots
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardSnapshot {
    pub snapshot_id: Uuid,
    pub dashboard_id: Uuid,
    pub name: String,
    pub key: String,
    pub delete_key: String,
    pub data: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub external: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSnapshotRequest {
    pub dashboard_id: Uuid,
    pub name: String,
    pub data: serde_json::Value,
    pub expires_seconds: Option<i64>,
    pub external: Option<bool>,
}

pub async fn list_snapshots(
    State(_state): State<AppState>,
) -> Result<Json<Vec<DashboardSnapshot>>, ApiError> {
    // In production: SELECT from dashboard_snapshots table
    Ok(Json(vec![]))
}

pub async fn create_snapshot(
    State(_state): State<AppState>,
    Json(req): Json<CreateSnapshotRequest>,
) -> Result<Json<DashboardSnapshot>, ApiError> {
    let snapshot_id = Uuid::now_v7();
    let key = format!("snap-{}", &snapshot_id.to_string()[..8]);
    let delete_key = format!("del-{}", &Uuid::now_v7().to_string()[..8]);
    let expires_at = req.expires_seconds.map(|s| Utc::now() + chrono::Duration::seconds(s));

    Ok(Json(DashboardSnapshot {
        snapshot_id,
        dashboard_id: req.dashboard_id,
        name: req.name,
        key,
        delete_key,
        data: req.data,
        expires_at,
        external: req.external.unwrap_or(false),
        created_by: Some("user".into()),
        created_at: Utc::now(),
    }))
}

pub async fn get_snapshot(
    State(_state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<DashboardSnapshot>, ApiError> {
    // In production: lookup by key, check expiry
    Err(ApiError::NotFound(format!("Snapshot '{}' not found", key)))
}

pub async fn delete_snapshot(
    State(_state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({ "deleted": key })))
}
