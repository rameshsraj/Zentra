//! SW P04: Apdex Scoring on Services.
//!
//! Apdex = (satisfied + tolerating×0.5) / total
//! Satisfied: duration ≤ T, Tolerating: T < duration ≤ 4T, Frustrated: duration > 4T

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ApdexScore {
    pub service_name: String,
    pub apdex: f64,
    pub satisfied: i64,
    pub tolerating: i64,
    pub frustrated: i64,
    pub total: i64,
    pub rating: String,
}

#[derive(Debug, Deserialize)]
pub struct ApdexQuery {
    pub window: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpsertApdexConfigRequest {
    pub service_name: String,
    pub satisfied_ms: Option<i32>,
    pub tolerating_ms: Option<i32>,
}

fn apdex_rating(score: f64) -> &'static str {
    if score >= 0.94 { "Excellent" }
    else if score >= 0.85 { "Good" }
    else if score >= 0.70 { "Fair" }
    else if score >= 0.50 { "Poor" }
    else { "Unacceptable" }
}

fn parse_window(window: &Option<String>) -> String {
    match window.as_deref() {
        Some("5m") => "5 minutes".into(),
        Some("15m") => "15 minutes".into(),
        Some("30m") => "30 minutes".into(),
        Some("1h") | None => "1 hour".into(),
        Some("6h") => "6 hours".into(),
        Some("24h") | Some("1d") => "24 hours".into(),
        Some(other) => format!("{} hours", other.trim_end_matches('h')),
    }
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/apdex?window=1h — score per service
pub async fn list_apdex_scores(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ApdexQuery>,
) -> Result<Json<Vec<ApdexScore>>, ApiError> {
    let window = parse_window(&params.window);

    let rows = sqlx::query_as::<_, ApdexRow>(
        &format!(
            r#"WITH configs AS (
                SELECT service_name, satisfied_ms, tolerating_ms FROM apdex_config WHERE org_id = $1
            ),
            span_stats AS (
                SELECT
                    s.service_name,
                    COALESCE(c.satisfied_ms, 200) AS t_ms,
                    COUNT(*) AS total,
                    COUNT(*) FILTER (WHERE s.duration_us / 1000.0 <= COALESCE(c.satisfied_ms, 200)) AS satisfied,
                    COUNT(*) FILTER (WHERE s.duration_us / 1000.0 > COALESCE(c.satisfied_ms, 200)
                                      AND s.duration_us / 1000.0 <= COALESCE(c.satisfied_ms, 200) * 4) AS tolerating,
                    COUNT(*) FILTER (WHERE s.duration_us / 1000.0 > COALESCE(c.satisfied_ms, 200) * 4) AS frustrated
                FROM spans s
                LEFT JOIN configs c ON c.service_name = s.service_name
                WHERE s.started_at > NOW() - INTERVAL '{window}'
                GROUP BY s.service_name, c.satisfied_ms
            )
            SELECT service_name, total, satisfied, tolerating, frustrated,
                   CASE WHEN total > 0
                        THEN (satisfied::FLOAT + tolerating::FLOAT * 0.5) / total::FLOAT
                        ELSE 1.0 END AS apdex
            FROM span_stats
            ORDER BY apdex ASC"#,
            window = window
        )
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let scores = rows.into_iter().map(|r| ApdexScore {
        service_name: r.service_name,
        apdex: r.apdex,
        satisfied: r.satisfied,
        tolerating: r.tolerating,
        frustrated: r.frustrated,
        total: r.total,
        rating: apdex_rating(r.apdex).into(),
    }).collect();

    Ok(Json(scores))
}

/// GET /api/apdex/:service?window=1h — score + breakdown for one service
pub async fn get_service_apdex(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(service): Path<String>,
    Query(params): Query<ApdexQuery>,
) -> Result<Json<ApdexScore>, ApiError> {
    let window = parse_window(&params.window);

    let row = sqlx::query_as::<_, ApdexRow>(
        &format!(
            r#"WITH config AS (
                SELECT satisfied_ms, tolerating_ms FROM apdex_config WHERE org_id = $1 AND service_name = $2
            )
            SELECT
                $2::TEXT AS service_name,
                COUNT(*) AS total,
                COUNT(*) FILTER (WHERE s.duration_us / 1000.0 <= COALESCE((SELECT satisfied_ms FROM config), 200)) AS satisfied,
                COUNT(*) FILTER (WHERE s.duration_us / 1000.0 > COALESCE((SELECT satisfied_ms FROM config), 200)
                                  AND s.duration_us / 1000.0 <= COALESCE((SELECT satisfied_ms FROM config), 200) * 4) AS tolerating,
                COUNT(*) FILTER (WHERE s.duration_us / 1000.0 > COALESCE((SELECT satisfied_ms FROM config), 200) * 4) AS frustrated,
                CASE WHEN COUNT(*) > 0
                     THEN (COUNT(*) FILTER (WHERE s.duration_us / 1000.0 <= COALESCE((SELECT satisfied_ms FROM config), 200))::FLOAT
                           + COUNT(*) FILTER (WHERE s.duration_us / 1000.0 > COALESCE((SELECT satisfied_ms FROM config), 200)
                                               AND s.duration_us / 1000.0 <= COALESCE((SELECT satisfied_ms FROM config), 200) * 4)::FLOAT * 0.5)
                          / COUNT(*)::FLOAT
                     ELSE 1.0 END AS apdex
            FROM spans s
            WHERE s.service_name = $2 AND s.started_at > NOW() - INTERVAL '{window}'"#,
            window = window
        )
    )
    .bind(org_id)
    .bind(&service)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound(format!("No spans found for service '{}'", service)))?;

    Ok(Json(ApdexScore {
        service_name: row.service_name,
        apdex: row.apdex,
        satisfied: row.satisfied,
        tolerating: row.tolerating,
        frustrated: row.frustrated,
        total: row.total,
        rating: apdex_rating(row.apdex).into(),
    }))
}

/// PUT /api/apdex/config — upsert thresholds
pub async fn upsert_apdex_config(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<UpsertApdexConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let satisfied = req.satisfied_ms.unwrap_or(200);
    let tolerating = req.tolerating_ms.unwrap_or(satisfied * 4);

    sqlx::query(
        r#"INSERT INTO apdex_config (org_id, service_name, satisfied_ms, tolerating_ms)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (org_id, service_name) DO UPDATE SET satisfied_ms = $3, tolerating_ms = $4"#
    )
    .bind(org_id)
    .bind(&req.service_name)
    .bind(satisfied)
    .bind(tolerating)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "service_name": req.service_name,
        "satisfied_ms": satisfied,
        "tolerating_ms": tolerating,
        "status": "saved",
    })))
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ApdexRow {
    service_name: String,
    total: i64,
    satisfied: i64,
    tolerating: i64,
    frustrated: i64,
    apdex: f64,
}
