//! Anomaly detection API — backed by `anomaly_detections` and `anomaly_feedback` tables.

use axum::{
    extract::{State, Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyRecord {
    pub id: Uuid,
    pub detected_at: DateTime<Utc>,
    pub target_id: Option<Uuid>,
    pub target_name: Option<String>,
    pub metric_name: String,
    pub value: f64,
    pub expected_mean: Option<f64>,
    pub expected_stddev: Option<f64>,
    pub deviation_sigma: Option<f64>,
    pub detector: String,
    pub severity: String,
    pub acknowledged: bool,
    pub false_positive: bool,
}

#[derive(Debug, Deserialize)]
pub struct AnomalyQuery {
    pub target_id: Option<Uuid>,
    pub metric_name: Option<String>,
    pub severity: Option<String>,
    pub detector: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub include_false_positives: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackRequest {
    pub is_real: bool,
    pub comment: Option<String>,
}

pub async fn list_anomalies(
    State(state): State<AppState>,
    Query(q): Query<AnomalyQuery>,
) -> Result<Json<Vec<AnomalyRecord>>, ApiError> {
    let limit = q.limit.unwrap_or(100);
    let offset = q.offset.unwrap_or(0);
    let include_fp = q.include_false_positives.unwrap_or(false);

    let rows = sqlx::query_as::<_, (Uuid, DateTime<Utc>, Option<Uuid>, Option<String>, String, f64, Option<f64>, Option<f64>, Option<f64>, String, String, bool, bool)>(
        "SELECT a.id, a.detected_at, a.target_id, t.name,
                a.metric_name, a.value, a.expected_mean, a.expected_stddev,
                a.deviation_sigma, a.detector, a.severity, a.acknowledged, a.false_positive
         FROM anomaly_detections a
         LEFT JOIN targets t ON t.target_id = a.target_id
         WHERE ($1::UUID IS NULL OR a.target_id = $1)
           AND ($2::TEXT IS NULL OR a.metric_name = $2)
           AND ($3::TEXT IS NULL OR a.severity = $3)
           AND ($4::TEXT IS NULL OR a.detector = $4)
           AND ($5::TIMESTAMPTZ IS NULL OR a.detected_at >= $5)
           AND ($6::TIMESTAMPTZ IS NULL OR a.detected_at <= $6)
           AND ($7 OR NOT a.false_positive)
         ORDER BY a.detected_at DESC
         LIMIT $8 OFFSET $9",
    )
    .bind(q.target_id)
    .bind(q.metric_name)
    .bind(q.severity)
    .bind(q.detector)
    .bind(q.from)
    .bind(q.to)
    .bind(include_fp)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let records = rows.into_iter().map(|(id, detected_at, target_id, target_name, metric_name, value, expected_mean, expected_stddev, deviation_sigma, detector, severity, acknowledged, false_positive)| {
        AnomalyRecord { id, detected_at, target_id, target_name, metric_name, value, expected_mean, expected_stddev, deviation_sigma, detector, severity, acknowledged, false_positive }
    }).collect();
    Ok(Json(records))
}

pub async fn anomaly_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let totals = sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT COUNT(*)::BIGINT,
                SUM(CASE WHEN acknowledged THEN 1 ELSE 0 END)::BIGINT,
                SUM(CASE WHEN false_positive THEN 1 ELSE 0 END)::BIGINT
         FROM anomaly_detections
         WHERE detected_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let by_severity: Vec<(String, i64)> = sqlx::query_as(
        "SELECT severity, COUNT(*)::BIGINT FROM anomaly_detections
         WHERE detected_at >= NOW() - INTERVAL '24 hours'
         GROUP BY severity",
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let by_detector: Vec<(String, i64)> = sqlx::query_as(
        "SELECT detector, COUNT(*)::BIGINT FROM anomaly_detections
         WHERE detected_at >= NOW() - INTERVAL '24 hours'
         GROUP BY detector",
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(serde_json::json!({
        "total_24h": totals.0,
        "acknowledged": totals.1,
        "false_positives": totals.2,
        "by_severity": by_severity.into_iter().map(|(k, v)| serde_json::json!({ "severity": k, "count": v })).collect::<Vec<_>>(),
        "by_detector": by_detector.into_iter().map(|(k, v)| serde_json::json!({ "detector": k, "count": v })).collect::<Vec<_>>(),
    })))
}

pub async fn top_anomalous_metrics(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT metric_name, COUNT(*)::BIGINT as cnt
         FROM anomaly_detections
         WHERE detected_at >= NOW() - INTERVAL '7 days'
           AND NOT false_positive
         GROUP BY metric_name
         ORDER BY cnt DESC
         LIMIT 10",
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let result = rows.into_iter().map(|(m, c)| serde_json::json!({ "metric_name": m, "count": c })).collect();
    Ok(Json(result))
}

pub async fn acknowledge_anomaly(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query(
        "UPDATE anomaly_detections SET acknowledged = TRUE WHERE id = $1",
    )
    .bind(id)
    .execute(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(format!("anomaly {} not found", id)));
    }
    Ok(Json(serde_json::json!({ "acknowledged": id })))
}

pub async fn submit_feedback(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<FeedbackRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM anomaly_detections WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.tsdb_pool)
        .await
        .map_err(ApiError::from)?;

    if !exists {
        return Err(ApiError::NotFound(format!("anomaly {} not found", id)));
    }

    // Mark false_positive flag on the detection
    sqlx::query("UPDATE anomaly_detections SET false_positive = $1 WHERE id = $2")
        .bind(!req.is_real)
        .bind(id)
        .execute(&state.tsdb_pool)
        .await
        .map_err(ApiError::from)?;

    // Record feedback for model improvement
    sqlx::query(
        "INSERT INTO anomaly_feedback (anomaly_id, is_real, comment) VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(req.is_real)
    .bind(&req.comment)
    .execute(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(serde_json::json!({ "received": id, "is_real": req.is_real })))
}
