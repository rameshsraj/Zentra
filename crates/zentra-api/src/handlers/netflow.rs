//! NetFlow / sFlow query and analytics API — backed by `netflow_records` and `flow_exporters`.

use axum::{
    extract::{State, Query},
    Json,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct FlowQuery {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub src_ip: Option<String>,
    pub dst_ip: Option<String>,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopTalker {
    pub address: String,
    pub bytes: i64,
    pub packets: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowExporter {
    pub ip: String,
    pub hostname: Option<String>,
    pub flow_type: String,
    pub last_seen: DateTime<Utc>,
    pub packets_rx: i64,
    pub flows_rx: i64,
}

pub async fn top_talkers(
    State(state): State<AppState>,
    Query(q): Query<FlowQuery>,
) -> Result<Json<Vec<TopTalker>>, ApiError> {
    let limit = q.limit.unwrap_or(20);
    let from = q.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = q.to.unwrap_or_else(Utc::now);

    let rows = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT src_ip::TEXT, SUM(bytes)::BIGINT, SUM(packets)::BIGINT
         FROM netflow_records
         WHERE received_at BETWEEN $1 AND $2
           AND ($3::TEXT IS NULL OR protocol_name = $3)
         GROUP BY src_ip
         ORDER BY SUM(bytes) DESC
         LIMIT $4",
    )
    .bind(from)
    .bind(to)
    .bind(q.protocol)
    .bind(limit)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let result = rows.into_iter().map(|(address, bytes, packets)| TopTalker { address, bytes, packets }).collect();
    Ok(Json(result))
}

pub async fn flow_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT COUNT(*)::BIGINT, COALESCE(SUM(bytes), 0)::BIGINT, COALESCE(SUM(packets), 0)::BIGINT
         FROM netflow_records
         WHERE received_at >= NOW() - INTERVAL '1 hour'",
    )
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(serde_json::json!({
        "total_flows": row.0,
        "total_bytes": row.1,
        "total_packets": row.2,
        "window": "1h",
    })))
}

pub async fn list_exporters(
    State(state): State<AppState>,
) -> Result<Json<Vec<FlowExporter>>, ApiError> {
    let rows = sqlx::query_as::<_, (String, Option<String>, String, DateTime<Utc>, i64, i64)>(
        "SELECT ip::TEXT, hostname, flow_type, last_seen, packets_rx, flows_rx
         FROM flow_exporters
         ORDER BY last_seen DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let exporters = rows.into_iter().map(|(ip, hostname, flow_type, last_seen, packets_rx, flows_rx)| {
        FlowExporter { ip, hostname, flow_type, last_seen, packets_rx, flows_rx }
    }).collect();
    Ok(Json(exporters))
}

pub async fn protocol_breakdown(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let rows = sqlx::query_as::<_, (Option<String>, i64, i64)>(
        "SELECT protocol_name, SUM(bytes)::BIGINT, SUM(packets)::BIGINT
         FROM netflow_records
         WHERE received_at >= NOW() - INTERVAL '1 hour'
         GROUP BY protocol_name
         ORDER BY SUM(bytes) DESC",
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let result = rows.into_iter().map(|(proto, bytes, packets)| {
        serde_json::json!({ "protocol": proto.unwrap_or_else(|| "unknown".into()), "bytes": bytes, "packets": packets })
    }).collect();
    Ok(Json(result))
}

pub async fn flow_timeseries(
    State(state): State<AppState>,
    Query(q): Query<FlowQuery>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    let from = q.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = q.to.unwrap_or_else(Utc::now);

    let rows = sqlx::query_as::<_, (DateTime<Utc>, i64, i64)>(
        "SELECT time_bucket('5 minutes', received_at) AS bucket,
                SUM(bytes)::BIGINT,
                SUM(packets)::BIGINT
         FROM netflow_records
         WHERE received_at BETWEEN $1 AND $2
         GROUP BY bucket
         ORDER BY bucket ASC",
    )
    .bind(from)
    .bind(to)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(ApiError::from)?;

    let result = rows.into_iter().map(|(bucket, bytes, packets)| {
        serde_json::json!({ "time": bucket, "bytes": bytes, "packets": packets })
    }).collect();
    Ok(Json(result))
}
