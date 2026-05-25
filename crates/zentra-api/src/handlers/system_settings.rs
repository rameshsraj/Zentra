use axum::{
    extract::State,
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
// GET /api/settings — all system settings
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_settings(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let settings = sqlx::query_as::<_, (String, serde_json::Value, Option<String>, chrono::DateTime<chrono::Utc>)>(
        "SELECT key, value, description, updated_at FROM system_settings WHERE org_id = $1 ORDER BY key"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let list: Vec<serde_json::Value> = settings.iter().map(|s| json!({
        "key": s.0, "value": s.1, "description": s.2, "updated_at": s.3,
    })).collect();

    (StatusCode::OK, Json(json!(list))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/settings — update settings
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_settings(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<UpdateSettingsRequest>,
) -> impl IntoResponse {
    let mut updated = 0i64;
    for setting in &req.settings {
        let result = sqlx::query(
            "INSERT INTO system_settings (org_id, key, value, updated_at) VALUES ($1, $2, $3, NOW()) \
             ON CONFLICT (org_id, key) DO UPDATE SET value = $3, updated_at = NOW()"
        )
        .bind(org_id)
        .bind(&setting.key)
        .bind(&setting.value)
        .execute(&state.pool)
        .await;

        if let Ok(r) = result { updated += r.rows_affected() as i64; }
    }

    (StatusCode::OK, Json(json!({
        "message": "Settings updated",
        "updated": updated,
    }))).into_response()
}
