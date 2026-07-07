use axum::{extract::{Extension, Path, State}, Json};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use crate::auth_middleware::AuthContext;
use crate::error::ApiError;
use crate::state::AppState;

fn require_admin(ctx: &AuthContext) -> Result<(), ApiError> {
    if matches!(ctx.role.as_str(), "Admin" | "Org Admin" | "Super Admin") {
        Ok(())
    } else {
        Err(ApiError::Forbidden("Only Org Admin or Super Admin can manage enrollment keys".into()))
    }
}

fn generate_org_key() -> String {
    let mut rng = rand::thread_rng();
    let suffix = (0..40)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect::<String>();
    format!("isok_{suffix}")
}

fn hash_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Deserialize)]
pub struct CreateOrgEnrollmentKeyRequest {
    pub max_registrations_per_hour: Option<i32>,
    pub allowed_cidr: Option<String>,
    pub require_approval: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub async fn create_org_enrollment_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Json(body): Json<CreateOrgEnrollmentKeyRequest>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;

    let raw_key = generate_org_key();
    let key_hash = hash_key(&raw_key);
    let display_prefix = raw_key.chars().take(18).collect::<String>();
    let id = Uuid::now_v7();

    let max_per_hour = body.max_registrations_per_hour.unwrap_or(5).max(1);

    sqlx::query(
        "INSERT INTO org_enrollment_keys
         (id, org_id, key_hash, display_prefix, enabled, max_registrations_per_hour,
          allowed_cidr, require_approval, expires_at, created_by, created_at)
         VALUES ($1, $2, $3, $4, true, $5, $6::inet, $7, $8, $9, NOW())",
    )
    .bind(id)
    .bind(ctx.org_id)
    .bind(&key_hash)
    .bind(&display_prefix)
    .bind(max_per_hour)
    .bind(body.allowed_cidr.as_deref())
    .bind(body.require_approval.unwrap_or(false))
    .bind(body.expires_at)
    .bind(ctx.user_id)
    .execute(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    sqlx::query(
        "INSERT INTO audit_log (org_id, user_id, action, resource_type, resource_id, details, status)
         VALUES ($1, $2, 'org_enrollment_key.create', 'org_enrollment_key', $3, $4::jsonb, 'success')",
    )
    .bind(ctx.org_id)
    .bind(ctx.user_id)
    .bind(id.to_string())
    .bind(json!({"max_registrations_per_hour": max_per_hour}))
    .execute(&state.pool)
    .await
    .ok();

    Ok(Json(json!({
        "id": id,
        "org_key": raw_key,
        "display_prefix": display_prefix,
        "warning": "Store this key securely. It is shown only once.",
    })))
}

pub async fn list_org_enrollment_keys(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;

    let rows = sqlx::query(
        "SELECT id, display_prefix, enabled, max_registrations_per_hour, allowed_cidr::text,
                require_approval, expires_at, created_at, last_used_at, registration_count
         FROM org_enrollment_keys
         WHERE org_id = $1
         ORDER BY created_at DESC",
    )
    .bind(ctx.org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "display_prefix": r.get::<String, _>("display_prefix"),
                "enabled": r.get::<bool, _>("enabled"),
                "max_registrations_per_hour": r.get::<i32, _>("max_registrations_per_hour"),
                "allowed_cidr": r.get::<Option<String>, _>("allowed_cidr"),
                "require_approval": r.get::<bool, _>("require_approval"),
                "expires_at": r.get::<Option<DateTime<Utc>>, _>("expires_at"),
                "created_at": r.get::<DateTime<Utc>, _>("created_at"),
                "last_used_at": r.get::<Option<DateTime<Utc>>, _>("last_used_at"),
                "registration_count": r.get::<i32, _>("registration_count"),
            })
        })
        .collect();

    Ok(Json(json!(items)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrgEnrollmentKeyRequest {
    pub enabled: Option<bool>,
    pub max_registrations_per_hour: Option<i32>,
    pub allowed_cidr: Option<String>,
    pub require_approval: Option<bool>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub async fn update_org_enrollment_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateOrgEnrollmentKeyRequest>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;

    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE org_enrollment_keys
         SET enabled = COALESCE($3, enabled),
             max_registrations_per_hour = COALESCE($4, max_registrations_per_hour),
             allowed_cidr = COALESCE($5::inet, allowed_cidr),
             require_approval = COALESCE($6, require_approval),
             expires_at = COALESCE($7, expires_at)
         WHERE id = $1 AND org_id = $2
         RETURNING id",
    )
    .bind(id)
    .bind(ctx.org_id)
    .bind(body.enabled)
    .bind(body.max_registrations_per_hour.map(|v| v.max(1)))
    .bind(body.allowed_cidr.as_deref())
    .bind(body.require_approval)
    .bind(body.expires_at)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if updated.is_none() {
        return Err(ApiError::NotFound("Enrollment key not found".into()));
    }

    Ok(Json(json!({ "updated": true, "id": id })))
}

pub async fn delete_org_enrollment_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;

    let deleted: Option<Uuid> = sqlx::query_scalar(
        "UPDATE org_enrollment_keys
         SET enabled = false
         WHERE id = $1 AND org_id = $2
         RETURNING id",
    )
    .bind(id)
    .bind(ctx.org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if deleted.is_none() {
        return Err(ApiError::NotFound("Enrollment key not found".into()));
    }

    Ok(Json(json!({ "revoked": true, "id": id })))
}

pub async fn list_org_enrollment_key_registrations(
    State(state): State<AppState>,
    Extension(ctx): Extension<AuthContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&ctx)?;

    let rows = sqlx::query(
        "SELECT id, agent_id, request_ip::text, hostname, status, reason, created_at
         FROM agent_registration_audit
         WHERE org_id = $1 AND enrollment_key_id = $2
         ORDER BY created_at DESC
         LIMIT 200",
    )
    .bind(ctx.org_id)
    .bind(id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let logs: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "agent_id": r.get::<Option<Uuid>, _>("agent_id"),
                "request_ip": r.get::<Option<String>, _>("request_ip"),
                "hostname": r.get::<Option<String>, _>("hostname"),
                "status": r.get::<String, _>("status"),
                "reason": r.get::<Option<String>, _>("reason"),
                "created_at": r.get::<DateTime<Utc>, _>("created_at"),
            })
        })
        .collect();

    Ok(Json(json!(logs)))
}
