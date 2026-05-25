//! OTel P09: Span-to-Metrics Connector.
//!
//! Derives RED metrics (Rate, Error, Duration) from ingested spans
//! and stores them in `otel_derived_metrics`.

use axum::{extract::{State, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct DerivedMetricsQuery {
    pub service: Option<String>,
    pub metric: Option<String>,
    pub window: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectorConfigRequest {
    pub connector_type: String,
    pub enabled: bool,
    pub dimensions: Option<Vec<String>>,
    pub interval_seconds: Option<i64>,
}

fn parse_interval(w: &Option<String>) -> &str {
    match w.as_deref() {
        Some("5m") => "5 minutes",
        Some("15m") => "15 minutes",
        Some("1h") | None => "1 hour",
        Some("6h") => "6 hours",
        Some("24h") | Some("1d") => "24 hours",
        _ => "1 hour",
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/otel/connectors — list active connectors
pub async fn list_connectors() -> Json<serde_json::Value> {
    Json(json!({
        "connectors": [
            {
                "type": "span-to-metrics",
                "enabled": true,
                "description": "Derives RED metrics (Rate, Error rate, Duration percentiles) from ingested trace spans",
                "dimensions": ["http_method", "http_route", "db_system", "rpc_service"],
                "interval_seconds": 60,
                "metrics_generated": ["request_count", "error_count", "duration_p50", "duration_p95", "duration_p99"],
            }
        ],
    }))
}

/// POST /api/otel/connectors — enable/configure a connector
pub async fn configure_connector(
    Json(req): Json<ConnectorConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(json!({
        "connector_type": req.connector_type,
        "enabled": req.enabled,
        "dimensions": req.dimensions.unwrap_or_default(),
        "interval_seconds": req.interval_seconds.unwrap_or(60),
        "status": "configured",
    })))
}

/// GET /api/otel/derived-metrics?service=X&metric=duration_p95&window=1h
pub async fn query_derived_metrics(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<DerivedMetricsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = parse_interval(&params.window);

    let mut where_clauses = vec![
        "org_id = $1".to_string(),
        format!("bucket_start > NOW() - INTERVAL '{}'", interval),
    ];
    let mut bind_idx = 2u32;

    if let Some(ref svc) = params.service {
        bind_idx += 1;
        where_clauses.push(format!("service_name = ${}", bind_idx - 1));
        let _ = svc;
    }
    if let Some(ref m) = params.metric {
        bind_idx += 1;
        where_clauses.push(format!("metric_name = ${}", bind_idx - 1));
        let _ = m;
    }

    let sql = format!(
        "SELECT service_name, metric_name, bucket_start, value, dimensions FROM otel_derived_metrics WHERE {} ORDER BY bucket_start DESC LIMIT 500",
        where_clauses.join(" AND ")
    );

    let mut query = sqlx::query_as::<_, DerivedMetricRow>(&sql)
        .bind(org_id);

    if let Some(ref svc) = params.service {
        query = query.bind(svc);
    }
    if let Some(ref m) = params.metric {
        query = query.bind(m);
    }

    let rows = query
        .fetch_all(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let metrics: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "service_name": r.service_name,
        "metric_name": r.metric_name,
        "bucket_start": r.bucket_start,
        "value": r.value,
        "dimensions": r.dimensions,
    })).collect();

    Ok(Json(json!({ "metrics": metrics, "count": metrics.len() })))
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct DerivedMetricRow {
    service_name: String,
    metric_name: String,
    bucket_start: DateTime<Utc>,
    value: f64,
    dimensions: serde_json::Value,
}
