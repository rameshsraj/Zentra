//! SW P07: Trace ↔ DB Metric Cross-Correlation.
//!
//! When a trace contains a DB span (db.system, db.statement attributes),
//! correlate it with actual DB metrics at the exact time of the slowness.

use axum::{extract::{State, Path, Query}, Json};
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

#[derive(Debug, Serialize)]
pub struct DbSpanContext {
    pub span_id: String,
    pub trace_id: String,
    pub service_name: String,
    pub db_system: String,
    pub db_statement: Option<String>,
    pub duration_ms: f64,
    pub started_at: DateTime<Utc>,
    pub db_metrics: DbMetricsSnapshot,
}

#[derive(Debug, Serialize)]
pub struct DbMetricsSnapshot {
    pub active_connections: Option<f64>,
    pub active_queries: Option<f64>,
    pub lock_waits: Option<f64>,
    pub cache_hit_ratio: Option<f64>,
    pub target_id: Option<Uuid>,
    pub target_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SlowDbTrace {
    pub trace_id: String,
    pub service_name: String,
    pub operation_name: String,
    pub duration_ms: f64,
    pub db_system: String,
    pub db_statement: Option<String>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ServiceDbMapping {
    pub service_name: String,
    pub db_system: String,
    pub target_name: Option<String>,
    pub call_count: i64,
    pub p95_latency_ms: f64,
}

#[derive(Debug, Deserialize)]
pub struct DbSlownessQuery {
    pub target_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub threshold_ms: Option<f64>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/drilldown/trace/:trace_id/db-context
pub async fn trace_db_context(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(trace_id): Path<String>,
) -> Result<Json<Vec<DbSpanContext>>, ApiError> {
    // Find DB spans in this trace
    let db_spans = sqlx::query_as::<_, DbSpanRow>(
        r#"SELECT span_id, trace_id, service_name, operation_name,
                  duration_us, started_at,
                  tags->>'db.system' AS db_system,
                  tags->>'db.statement' AS db_statement
           FROM spans
           WHERE trace_id = $1 AND tags ? 'db.system'
           ORDER BY started_at"#
    )
    .bind(&trace_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut contexts = Vec::new();

    for span in &db_spans {
        let window_start = span.started_at - chrono::Duration::seconds(30);
        let window_end = span.started_at + chrono::Duration::microseconds(span.duration_us) + chrono::Duration::seconds(30);

        // Find matching DB target by db.system
        let db_system = span.db_system.as_deref().unwrap_or("unknown");

        // Query DB metrics in the window
        let active_conns: Option<f64> = sqlx::query_scalar(
            r#"SELECT AVG(value) FROM metrics
               WHERE org_id = $1 AND metric_name LIKE '%active_connections%'
                 AND time >= $2 AND time <= $3"#
        )
        .bind(org_id).bind(window_start).bind(window_end)
        .fetch_optional(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .flatten();

        let lock_waits: Option<f64> = sqlx::query_scalar(
            r#"SELECT AVG(value) FROM metrics
               WHERE org_id = $1 AND metric_name LIKE '%lock_wait%'
                 AND time >= $2 AND time <= $3"#
        )
        .bind(org_id).bind(window_start).bind(window_end)
        .fetch_optional(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .flatten();

        let cache_hit: Option<f64> = sqlx::query_scalar(
            r#"SELECT AVG(value) FROM metrics
               WHERE org_id = $1 AND metric_name LIKE '%cache_hit%'
                 AND time >= $2 AND time <= $3"#
        )
        .bind(org_id).bind(window_start).bind(window_end)
        .fetch_optional(&state.tsdb_pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .flatten();

        contexts.push(DbSpanContext {
            span_id: span.span_id.clone(),
            trace_id: span.trace_id.clone(),
            service_name: span.service_name.clone(),
            db_system: db_system.to_string(),
            db_statement: span.db_statement.clone(),
            duration_ms: span.duration_us as f64 / 1000.0,
            started_at: span.started_at,
            db_metrics: DbMetricsSnapshot {
                active_connections: active_conns,
                active_queries: None,
                lock_waits,
                cache_hit_ratio: cache_hit,
                target_id: None,
                target_name: None,
            },
        });
    }

    Ok(Json(contexts))
}

/// GET /api/drilldown/db-slowness?target_id=&from=&to=&threshold_ms=
pub async fn db_slowness(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
    Query(params): Query<DbSlownessQuery>,
) -> Result<Json<Vec<SlowDbTrace>>, ApiError> {
    let threshold = params.threshold_ms.unwrap_or(1000.0);
    let from = params.from.unwrap_or_else(|| Utc::now() - chrono::Duration::hours(1));
    let to = params.to.unwrap_or_else(Utc::now);

    let rows = sqlx::query_as::<_, SlowTraceRow>(
        r#"SELECT trace_id, service_name, operation_name, duration_us, started_at,
                  tags->>'db.system' AS db_system,
                  tags->>'db.statement' AS db_statement
           FROM spans
           WHERE tags ? 'db.system'
             AND started_at >= $1 AND started_at <= $2
             AND duration_us / 1000.0 >= $3
           ORDER BY duration_us DESC
           LIMIT 100"#
    )
    .bind(from)
    .bind(to)
    .bind(threshold)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| SlowDbTrace {
        trace_id: r.trace_id,
        service_name: r.service_name,
        operation_name: r.operation_name,
        duration_ms: r.duration_us as f64 / 1000.0,
        db_system: r.db_system.unwrap_or_default(),
        db_statement: r.db_statement,
        started_at: r.started_at,
    }).collect()))
}

/// GET /api/correlation/service-to-db — which services call which DBs
pub async fn service_to_db_map(
    State(state): State<AppState>,
    OrgId(_org_id): OrgId,
) -> Result<Json<Vec<ServiceDbMapping>>, ApiError> {
    let rows = sqlx::query_as::<_, ServiceDbRow>(
        r#"SELECT service_name,
                  tags->>'db.system' AS db_system,
                  COUNT(*) AS call_count,
                  PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_us / 1000.0) AS p95_ms
           FROM spans
           WHERE tags ? 'db.system'
             AND started_at > NOW() - INTERVAL '24 hours'
           GROUP BY service_name, tags->>'db.system'
           ORDER BY call_count DESC
           LIMIT 100"#
    )
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(rows.into_iter().map(|r| ServiceDbMapping {
        service_name: r.service_name,
        db_system: r.db_system.unwrap_or_default(),
        target_name: None,
        call_count: r.call_count,
        p95_latency_ms: r.p95_ms.unwrap_or(0.0),
    }).collect()))
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct DbSpanRow {
    span_id: String,
    trace_id: String,
    service_name: String,
    operation_name: String,
    duration_us: i64,
    started_at: DateTime<Utc>,
    db_system: Option<String>,
    db_statement: Option<String>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct SlowTraceRow {
    trace_id: String,
    service_name: String,
    operation_name: String,
    duration_us: i64,
    started_at: DateTime<Utc>,
    db_system: Option<String>,
    db_statement: Option<String>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ServiceDbRow {
    service_name: String,
    db_system: Option<String>,
    call_count: i64,
    p95_ms: Option<f64>,
}
