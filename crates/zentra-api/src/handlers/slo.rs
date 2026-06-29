//! SLO/SLI handlers — Service Level Objectives derived from OTEL RED metrics.

use axum::{extract::{State, Path, Query}, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use chrono::Utc;

use crate::state::AppState;
use crate::tenant::OrgId;
use crate::error::ApiError;

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WindowQuery {
    pub window: Option<String>,
}

impl WindowQuery {
    fn interval(&self) -> &str {
        match self.window.as_deref() {
            Some("15m") => "15 minutes",
            Some("1h")  => "1 hour",
            Some("6h")  => "6 hours",
            Some("24h") => "24 hours",
            _           => "1 hour",
        }
    }
}

// ── Rows ─────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow, Serialize)]
struct ServiceRedRow {
    service_name: String,
    total_requests: i64,
    error_count: i64,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
}

#[derive(sqlx::FromRow, Serialize)]
struct SloTargetRow {
    id: Uuid,
    org_id: Uuid,
    service_name: String,
    metric: String,
    threshold: f64,
    window_days: i32,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

// ── Request bodies ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateSloTargetRequest {
    pub service_name: String,
    pub metric: String,
    pub threshold: f64,
    pub window_days: Option<i32>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// GET /api/slo/services?window=1h
/// Returns per-service RED metrics (Rate, Errors, Duration) from OTEL traces.
pub async fn slo_services(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<WindowQuery>,
) -> Result<Json<Value>, ApiError> {
    let interval = params.interval();

    let rows = sqlx::query_as::<_, ServiceRedRow>(&format!(
        "SELECT
            service_name,
            COUNT(*) AS total_requests,
            COUNT(*) FILTER (WHERE status = 'error') AS error_count,
            ROUND(
                PERCENTILE_CONT(0.50) WITHIN GROUP (ORDER BY duration_us)::numeric / 1000.0,
                2
            ) AS p50_ms,
            ROUND(
                PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_us)::numeric / 1000.0,
                2
            ) AS p95_ms,
            ROUND(
                PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_us)::numeric / 1000.0,
                2
            ) AS p99_ms
         FROM traces
         WHERE org_id = $1
           AND started_at > NOW() - INTERVAL '{interval}'
           AND parent_span_id IS NULL
         GROUP BY service_name
         ORDER BY total_requests DESC"
    ))
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let services: Vec<Value> = rows.iter().map(|r| {
        let error_rate = if r.total_requests > 0 {
            (r.error_count as f64 / r.total_requests as f64) * 100.0
        } else { 0.0 };
        json!({
            "service_name": r.service_name,
            "total_requests": r.total_requests,
            "error_count": r.error_count,
            "error_rate_pct": (error_rate * 100.0).round() / 100.0,
            "p50_ms": r.p50_ms,
            "p95_ms": r.p95_ms,
            "p99_ms": r.p99_ms,
            "req_per_sec": (r.total_requests as f64 / 3600.0 * 100.0).round() / 100.0,
        })
    }).collect();

    Ok(Json(json!({ "services": services, "count": services.len(), "window": params.window })))
}

/// GET /api/slo/targets
pub async fn list_slo_targets(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Value>, ApiError> {
    let rows = sqlx::query_as::<_, SloTargetRow>(
        "SELECT id, org_id, service_name, metric, threshold, window_days, created_at, updated_at
         FROM slo_targets WHERE org_id = $1 ORDER BY created_at DESC"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({ "targets": rows, "count": rows.len() })))
}

/// POST /api/slo/targets
pub async fn create_slo_target(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateSloTargetRequest>,
) -> Result<Json<Value>, ApiError> {
    let valid_metrics = ["error_rate", "p99_latency", "p95_latency", "availability"];
    if !valid_metrics.contains(&req.metric.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "Invalid metric '{}'. Must be one of: {}",
            req.metric, valid_metrics.join(", ")
        )));
    }

    let id = Uuid::now_v7();
    let window_days = req.window_days.unwrap_or(30);

    sqlx::query(
        "INSERT INTO slo_targets (id, org_id, service_name, metric, threshold, window_days)
         VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(id)
    .bind(org_id)
    .bind(&req.service_name)
    .bind(&req.metric)
    .bind(req.threshold)
    .bind(window_days)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "id": id,
        "service_name": req.service_name,
        "metric": req.metric,
        "threshold": req.threshold,
        "window_days": window_days,
        "status": "created",
    })))
}

/// DELETE /api/slo/targets/:id
pub async fn delete_slo_target(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let result = sqlx::query(
        "DELETE FROM slo_targets WHERE id = $1 AND org_id = $2"
    )
    .bind(id)
    .bind(org_id)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(format!("SLO target {} not found", id)));
    }

    Ok(Json(json!({ "id": id, "status": "deleted" })))
}

/// GET /api/slo/budget/:service_name?window=30d
/// Computes error budget remaining based on SLO targets for this service.
pub async fn slo_budget(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(service_name): Path<String>,
    Query(params): Query<WindowQuery>,
) -> Result<Json<Value>, ApiError> {
    let window_days: i64 = params.window.as_deref()
        .and_then(|w| w.trim_end_matches('d').parse().ok())
        .unwrap_or(30);

    let window_seconds = window_days * 86400;

    // Fetch aggregate error stats for the service over the window
    #[derive(sqlx::FromRow)]
    struct BudgetRow {
        total: i64,
        errors: i64,
    }

    let row = sqlx::query_as::<_, BudgetRow>(
        "SELECT COUNT(*) AS total,
                COUNT(*) FILTER (WHERE status = 'error') AS errors
         FROM traces
         WHERE org_id = $1
           AND service_name = $2
           AND started_at > NOW() - $3::interval
           AND parent_span_id IS NULL"
    )
    .bind(org_id)
    .bind(&service_name)
    .bind(format!("{} days", window_days))
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let (total, errors) = row.map(|r| (r.total, r.errors)).unwrap_or((0, 0));

    let error_rate_pct = if total > 0 { errors as f64 / total as f64 * 100.0 } else { 0.0 };
    let availability_pct = 100.0 - error_rate_pct;

    // Look up user SLO target for this service (error_rate metric)
    let slo_threshold: Option<f64> = sqlx::query_scalar(
        "SELECT threshold FROM slo_targets
         WHERE org_id = $1 AND service_name = $2 AND metric = 'error_rate'
         ORDER BY created_at DESC LIMIT 1"
    )
    .bind(org_id)
    .bind(&service_name)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None);

    let target_availability = 100.0 - slo_threshold.unwrap_or(1.0); // default 99%

    let budget_seconds_total = (target_availability / 100.0 - 1.0).abs() * window_seconds as f64;
    let budget_seconds_used = (error_rate_pct / 100.0) * window_seconds as f64;
    let budget_seconds_remaining = (budget_seconds_total - budget_seconds_used).max(0.0);
    let burn_rate = if budget_seconds_total > 0.0 { budget_seconds_used / budget_seconds_total } else { 0.0 };

    Ok(Json(json!({
        "service_name": service_name,
        "window_days": window_days,
        "total_requests": total,
        "error_count": errors,
        "error_rate_pct": (error_rate_pct * 100.0).round() / 100.0,
        "availability_pct": (availability_pct * 100.0).round() / 100.0,
        "slo_target_availability_pct": target_availability,
        "budget_seconds_total": budget_seconds_total.round() as i64,
        "budget_seconds_used": budget_seconds_used.round() as i64,
        "budget_seconds_remaining": budget_seconds_remaining.round() as i64,
        "burn_rate": (burn_rate * 1000.0).round() / 1000.0,
        "slo_status": if burn_rate <= 1.0 { "ok" } else { "breached" },
    })))
}
