use axum::{
    extract::{State, Path},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, Algorithm, Validation, DecodingKey};
use serde_json::json;
use sha2::{Sha256, Digest};
use sqlx;
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::models::auth::*;

fn jwt_secret() -> String {
    std::env::var("ZENTRA_JWT_SECRET").unwrap_or_else(|_| "Zentra-default-jwt-secret-change-me".into())
}

fn extract_user_id(headers: &HeaderMap) -> Option<Uuid> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let token = auth.strip_prefix("Bearer ")?;
    let validation = Validation::new(Algorithm::HS256);
    let data = decode::<JwtClaims>(token, &DecodingKey::from_secret(jwt_secret().as_bytes()), &validation).ok()?;
    Some(data.claims.sub)
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/api-tokens — list current user's tokens
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_tokens(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
) -> impl IntoResponse {
    let user_id = match extract_user_id(&headers) {
        Some(id) => id,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response(),
    };

    let tokens = sqlx::query_as::<_, (Uuid, String, String, bool, Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)>(
        "SELECT key_id, name, LEFT(key_hash, 8) as key_prefix, enabled, expires_at, last_used_at, created_at \
         FROM api_keys WHERE user_id = $1 AND org_id = $2 ORDER BY created_at DESC"
    )
    .bind(user_id).bind(org_id)
    .fetch_all(&state.pool).await.unwrap_or_default();

    let list: Vec<serde_json::Value> = tokens.iter().map(|t| json!({
        "id": t.0, "name": t.1, "key_prefix": t.2, "enabled": t.3,
        "expires_at": t.4, "last_used_at": t.5, "created_at": t.6,
    })).collect();

    (StatusCode::OK, Json(json!(list))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/api-tokens — create (return raw token ONCE)
// ═══════════════════════════════════════════════════════════════════════════

pub async fn create_token(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    headers: HeaderMap,
    Json(req): Json<CreateApiTokenRequest>,
) -> impl IntoResponse {
    let user_id = match extract_user_id(&headers) {
        Some(id) => id,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response(),
    };

    let raw_key = format!("isk_{}", Uuid::new_v4().to_string().replace('-', ""));
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    let key_hash = format!("{:x}", hasher.finalize());

    let key_id = Uuid::new_v4();
    let expires_at = req.expires_in_days.map(|d| Utc::now() + Duration::days(d));
    let scopes = serde_json::to_value(&req.scopes.unwrap_or_default()).unwrap_or(json!([]));

    let result = sqlx::query(
        "INSERT INTO api_keys (key_id, org_id, user_id, name, key_hash, scopes, expires_at, enabled) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, true)"
    )
    .bind(key_id).bind(org_id).bind(user_id)
    .bind(&req.name).bind(&key_hash).bind(&scopes).bind(expires_at)
    .execute(&state.pool).await;

    match result {
        Ok(_) => (StatusCode::CREATED, Json(json!({
            "token": {
                "id": key_id, "name": req.name, "key_prefix": &raw_key[..12],
                "enabled": true, "expires_at": expires_at, "created_at": Utc::now(),
            },
            "raw_key": raw_key,
            "message": "Save this key — it will not be shown again!"
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/api-tokens/:id — revoke
// ═══════════════════════════════════════════════════════════════════════════

pub async fn revoke_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token_id): Path<Uuid>,
) -> impl IntoResponse {
    let user_id = match extract_user_id(&headers) {
        Some(id) => id,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not_authenticated"}))).into_response(),
    };

    let _ = sqlx::query("DELETE FROM api_keys WHERE key_id = $1 AND user_id = $2")
        .bind(token_id).bind(user_id).execute(&state.pool).await;

    (StatusCode::OK, Json(json!({"message": "Token revoked"}))).into_response()
}
