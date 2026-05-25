//! Windows server monitoring API — reads from the `metrics` table for Windows-specific metric names.

use axum::{
    extract::{State, Path, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct MetricQueryParams {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct WindowsMetric {
    pub time: DateTime<Utc>,
    pub value: f64,
    pub metric_name: String,
    pub unit: Option<String>,
}

async fn query_windows_metrics(
    state: &AppState,
    target_id: Uuid,
    metric_prefix: &str,
    q: &MetricQueryParams,
) -> Result<Vec<WindowsMetric>, ApiError> {
    let limit = q.limit.unwrap_or(200);
    let from = q.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = q.to.unwrap_or_else(Utc::now);
    let pattern = format!("{}%", metric_prefix);

    let rows = sqlx::query_as::<_, (DateTime<Utc>, f64, String, Option<String>)>(
        "SELECT timestamp, value, metric_name, unit
         FROM metrics
         WHERE target_id = $1
           AND metric_name LIKE $2
           AND timestamp BETWEEN $3 AND $4
         ORDER BY timestamp DESC
         LIMIT $5",
    )
    .bind(target_id)
    .bind(pattern)
    .bind(from)
    .bind(to)
    .bind(limit)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    Ok(rows.into_iter().map(|(time, value, metric_name, unit)| WindowsMetric { time, value, metric_name, unit }).collect())
}

pub async fn windows_cpu_metrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<MetricQueryParams>,
) -> Result<Json<Vec<WindowsMetric>>, ApiError> {
    Ok(Json(query_windows_metrics(&state, id, "windows.cpu", &q).await?))
}

pub async fn windows_memory_metrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<MetricQueryParams>,
) -> Result<Json<Vec<WindowsMetric>>, ApiError> {
    Ok(Json(query_windows_metrics(&state, id, "windows.memory", &q).await?))
}

pub async fn windows_disk_metrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<MetricQueryParams>,
) -> Result<Json<Vec<WindowsMetric>>, ApiError> {
    Ok(Json(query_windows_metrics(&state, id, "windows.disk", &q).await?))
}

pub async fn windows_network_metrics(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<MetricQueryParams>,
) -> Result<Json<Vec<WindowsMetric>>, ApiError> {
    Ok(Json(query_windows_metrics(&state, id, "windows.network", &q).await?))
}

pub async fn windows_services(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, f64)>(
        "SELECT metric_name, value
         FROM metrics
         WHERE target_id = $1
           AND metric_name LIKE 'windows.service.%'
           AND timestamp >= NOW() - INTERVAL '5 minutes'
         ORDER BY timestamp DESC",
    )
    .bind(id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let running = rows.iter().filter(|(_, v)| *v == 1.0).count();
    let stopped = rows.iter().filter(|(_, v)| *v == 0.0).count();
    let services: Vec<serde_json::Value> = rows.iter().map(|(name, state_val)| {
        let svc_name = name.trim_start_matches("windows.service.");
        serde_json::json!({ "name": svc_name, "state": if *state_val == 1.0 { "running" } else { "stopped" } })
    }).collect();

    Ok(Json(serde_json::json!({
        "running": running,
        "stopped": stopped,
        "services": services,
    })))
}

pub async fn windows_hardware(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (String, f64, Option<String>)>(
        "SELECT metric_name, value, unit
         FROM metrics
         WHERE target_id = $1
           AND metric_name LIKE 'windows.hardware.%'
           AND timestamp >= NOW() - INTERVAL '10 minutes'
         ORDER BY timestamp DESC",
    )
    .bind(id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let mut result = serde_json::json!({ "cpu_model": null, "memory_gb": null, "disks": [] });
    for (name, value, _unit) in &rows {
        match name.as_str() {
            "windows.hardware.memory_gb" => result["memory_gb"] = serde_json::json!(value),
            n if n.starts_with("windows.hardware.disk") => {
                if let Some(arr) = result["disks"].as_array_mut() {
                    arr.push(serde_json::json!({ "metric": n, "value": value }));
                }
            }
            _ => {}
        }
    }
    Ok(Json(result))
}
