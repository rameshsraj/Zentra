//! SW P08: LLM Dashboard from OTLP GenAI Spans.
//!
//! All LLM metrics derived from `llm_spans` table populated by OTLP ingestion
//! when spans contain `gen_ai.*` attributes.

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
pub struct LlmQuery {
    pub window: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LlmTrendQuery {
    pub window: Option<String>,
    pub step: Option<String>,
}

fn parse_interval(window: &Option<String>) -> String {
    match window.as_deref() {
        Some("5m") => "5 minutes".into(),
        Some("15m") => "15 minutes".into(),
        Some("1h") | None => "1 hour".into(),
        Some("6h") => "6 hours".into(),
        Some("24h") | Some("1d") => "24 hours".into(),
        Some("7d") => "7 days".into(),
        Some(other) => format!("{} hours", other.trim_end_matches('h')),
    }
}

fn parse_step(step: &Option<String>) -> String {
    match step.as_deref() {
        Some("1m") => "1 minute".into(),
        Some("5m") => "5 minutes".into(),
        Some("15m") => "15 minutes".into(),
        Some("1h") | None => "1 hour".into(),
        Some("1d") => "1 day".into(),
        Some(other) => format!("{} minutes", other.trim_end_matches('m')),
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/llm/overview?window= — total requests, tokens, avg/p95 latency, error rate
pub async fn llm_overview(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<LlmQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = parse_interval(&params.window);

    let row = sqlx::query_as::<_, LlmOverviewRow>(
        &format!(
            r#"SELECT
                COUNT(*) AS total_requests,
                COALESCE(SUM(input_tokens), 0) AS total_input_tokens,
                COALESCE(SUM(output_tokens), 0) AS total_output_tokens,
                COALESCE(AVG(duration_ms), 0) AS avg_latency_ms,
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms), 0) AS p95_latency_ms,
                COALESCE(COUNT(*) FILTER (WHERE is_error), 0) AS error_count,
                CASE WHEN COUNT(*) > 0 THEN COUNT(*) FILTER (WHERE is_error)::FLOAT / COUNT(*)::FLOAT ELSE 0 END AS error_rate
            FROM llm_spans
            WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{interval}'"#,
            interval = interval
        )
    )
    .bind(org_id)
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Estimated cost: rough approximation based on token counts
    let estimated_cost = (row.total_input_tokens as f64 * 0.00001) + (row.total_output_tokens as f64 * 0.00003);

    Ok(Json(json!({
        "total_requests": row.total_requests,
        "total_input_tokens": row.total_input_tokens,
        "total_output_tokens": row.total_output_tokens,
        "total_tokens": row.total_input_tokens + row.total_output_tokens,
        "avg_latency_ms": row.avg_latency_ms,
        "p95_latency_ms": row.p95_latency_ms,
        "error_count": row.error_count,
        "error_rate": row.error_rate,
        "total_cost": estimated_cost,
        "models": 0, // Placeholder or query count(distinct model)
    })))
}

/// GET /api/llm/models?window= — per-model breakdown
pub async fn llm_models(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<LlmQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = parse_interval(&params.window);

    let rows = sqlx::query_as::<_, LlmModelRow>(
        &format!(
            r#"SELECT
                COALESCE(model, 'unknown') AS model,
                COALESCE(gen_ai_system, 'unknown') AS gen_ai_system,
                COUNT(*) AS request_count,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(AVG(duration_ms), 0) AS avg_latency_ms,
                COALESCE(PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_ms), 0) AS p95_latency_ms,
                COALESCE(COUNT(*) FILTER (WHERE is_error), 0) AS error_count
            FROM llm_spans
            WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{interval}'
            GROUP BY model, gen_ai_system
            ORDER BY request_count DESC"#,
            interval = interval
        )
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let models: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "model": r.model,
        "gen_ai_system": r.gen_ai_system,
        "request_count": r.request_count,
        "total_input_tokens": r.input_tokens,
        "total_output_tokens": r.output_tokens,
        "avg_latency_ms": r.avg_latency_ms,
        "p95_latency_ms": r.p95_latency_ms,
        "error_rate": if r.request_count > 0 { (r.error_count as f64 / r.request_count as f64 * 100.0).round() / 100.0 } else { 0.0 },
    })).collect();

    Ok(Json(json!({ "models": models, "count": models.len() })))
}

/// GET /api/llm/cost-trend?window=&step= — hourly cost time series
pub async fn llm_cost_trend(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<LlmTrendQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = parse_interval(&params.window);
    let step = parse_step(&params.step);

    let rows = sqlx::query_as::<_, LlmTrendRow>(
        &format!(
            r#"SELECT
                time_bucket('{step}', started_at) AS bucket,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COUNT(*) AS request_count
            FROM llm_spans
            WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{interval}'
            GROUP BY bucket
            ORDER BY bucket"#,
            step = step,
            interval = interval
        )
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let series: Vec<serde_json::Value> = rows.iter().map(|r| {
        let cost = (r.input_tokens as f64 * 0.00001) + (r.output_tokens as f64 * 0.00003);
        json!({
            "bucket": r.bucket,
            "cost": cost,
            "tokens": r.input_tokens + r.output_tokens,
        })
    }).collect();

    Ok(Json(json!({ "trend": series, "count": series.len() })))
}

/// GET /api/llm/latency-distribution?window= — histogram buckets
pub async fn llm_latency_distribution(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<LlmQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = parse_interval(&params.window);

    let rows = sqlx::query_as::<_, LatencyBucketRow>(
        &format!(
            r#"SELECT
                CASE
                    WHEN duration_ms < 100 THEN '<100ms'
                    WHEN duration_ms < 500 THEN '100-500ms'
                    WHEN duration_ms < 1000 THEN '500ms-1s'
                    WHEN duration_ms < 5000 THEN '1-5s'
                    WHEN duration_ms < 10000 THEN '5-10s'
                    ELSE '>10s'
                END AS bucket,
                COUNT(*) AS count
            FROM llm_spans
            WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{interval}'
            GROUP BY bucket
            ORDER BY MIN(duration_ms)"#,
            interval = interval
        )
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let buckets: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "range": r.bucket,
        "count": r.count,
    })).collect();

    Ok(Json(json!({ "distribution": buckets })))
}

/// GET /api/llm/token-efficiency?window= — input vs output token ratio per model
pub async fn llm_token_efficiency(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<LlmQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = parse_interval(&params.window);

    let rows = sqlx::query_as::<_, TokenEfficiencyRow>(
        &format!(
            r#"SELECT
                COALESCE(model, 'unknown') AS model,
                COALESCE(SUM(input_tokens), 0) AS total_input,
                COALESCE(SUM(output_tokens), 0) AS total_output,
                CASE WHEN SUM(input_tokens) > 0
                     THEN SUM(output_tokens)::FLOAT / SUM(input_tokens)::FLOAT
                     ELSE 0 END AS output_input_ratio
            FROM llm_spans
            WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{interval}'
            GROUP BY model
            ORDER BY total_input DESC"#,
            interval = interval
        )
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let models: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "model": r.model,
        "total_input_tokens": r.total_input,
        "total_output_tokens": r.total_output,
        "output_input_ratio": r.output_input_ratio,
    })).collect();

    Ok(Json(json!({ "models": models })))
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct LlmOverviewRow {
    total_requests: i64,
    total_input_tokens: i64,
    total_output_tokens: i64,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    error_count: i64,
    error_rate: f64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct LlmModelRow {
    model: String,
    gen_ai_system: String,
    request_count: i64,
    input_tokens: i64,
    output_tokens: i64,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    error_count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct LlmTrendRow {
    bucket: DateTime<Utc>,
    input_tokens: i64,
    output_tokens: i64,
    request_count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct LatencyBucketRow {
    bucket: String,
    count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TokenEfficiencyRow {
    model: String,
    total_input: i64,
    total_output: i64,
    output_input_ratio: f64,
}
