use axum::{
    extract::{State, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx;

use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::models::auth::*;

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/audit-log — paginated with filters
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_audit_log(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<AuditLogFilter>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        uuid::Uuid, uuid::Uuid, Option<uuid::Uuid>, String, String, Option<String>,
        serde_json::Value, Option<String>, Option<String>, String, chrono::DateTime<chrono::Utc>
    )>(
        "SELECT a.log_id, a.org_id, a.user_id, a.action, a.resource_type, a.resource_id, \
         COALESCE(a.details, '{}'), host(a.ip_address)::text, a.user_agent, a.status, a.created_at \
         FROM audit_log a \
         WHERE a.org_id = $1 \
         AND ($2::uuid IS NULL OR a.user_id = $2) \
         AND ($3::text IS NULL OR a.action ILIKE '%' || $3 || '%') \
         AND ($4::text IS NULL OR a.resource_type = $4) \
         AND ($5::timestamptz IS NULL OR a.created_at >= $5) \
         AND ($6::timestamptz IS NULL OR a.created_at <= $6) \
         ORDER BY a.created_at DESC LIMIT $7 OFFSET $8"
    )
    .bind(org_id)
    .bind(params.user_id)
    .bind(params.action.as_deref())
    .bind(params.resource_type.as_deref())
    .bind(params.from)
    .bind(params.to)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| {
        json!({
            "log_id": r.0, "org_id": r.1, "user_id": r.2, "action": r.3,
            "resource_type": r.4, "resource_id": r.5, "details": r.6,
            "ip_address": r.7, "user_agent": r.8, "status": r.9, "timestamp": r.10,
        })
    }).collect();

    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_log WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "entries": entries, "total": total, "limit": limit, "offset": offset,
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/audit-log/export — CSV/JSON export
// ═══════════════════════════════════════════════════════════════════════════

pub async fn export_audit_log(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<AuditLogFilter>,
) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (
        uuid::Uuid, Option<uuid::Uuid>, String, String, Option<String>,
        String, chrono::DateTime<chrono::Utc>
    )>(
        "SELECT a.log_id, a.user_id, a.action, a.resource_type, a.resource_id, \
         a.status, a.created_at \
         FROM audit_log a WHERE a.org_id = $1 \
         AND ($2::timestamptz IS NULL OR a.created_at >= $2) \
         AND ($3::timestamptz IS NULL OR a.created_at <= $3) \
         ORDER BY a.created_at DESC LIMIT 10000"
    )
    .bind(org_id)
    .bind(params.from)
    .bind(params.to)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let entries: Vec<serde_json::Value> = rows.iter().map(|r| json!({
        "log_id": r.0, "user_id": r.1, "action": r.2, "resource_type": r.3,
        "resource_id": r.4, "status": r.5, "timestamp": r.6,
    })).collect();

    (StatusCode::OK, Json(json!({"entries": entries, "count": entries.len()}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/audit-log/stats — summary
// ═══════════════════════════════════════════════════════════════════════════

pub async fn audit_stats(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let total: i64 = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM audit_log WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    let today: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM audit_log WHERE org_id = $1 AND created_at >= CURRENT_DATE"
    ).bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    let top_users = sqlx::query_as::<_, (uuid::Uuid, i64)>(
        "SELECT user_id, COUNT(*) as cnt FROM audit_log WHERE org_id = $1 AND user_id IS NOT NULL \
         GROUP BY user_id ORDER BY cnt DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    let top_resources = sqlx::query_as::<_, (String, i64)>(
        "SELECT resource_type, COUNT(*) as cnt FROM audit_log WHERE org_id = $1 \
         GROUP BY resource_type ORDER BY cnt DESC LIMIT 10"
    ).bind(org_id).fetch_all(&state.pool).await.unwrap_or_default();

    (StatusCode::OK, Json(json!({
        "total_entries": total,
        "actions_today": today,
        "top_users": top_users.iter().map(|(id, c)| json!({"user_id": id, "action_count": c})).collect::<Vec<_>>(),
        "top_resources": top_resources.iter().map(|(r, c)| json!({"resource_type": r, "action_count": c})).collect::<Vec<_>>(),
    }))).into_response()
}
