//! SAAS-03: Self-service signup, email verification, onboarding checklist, and quota enforcement.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth_middleware::JwtClaims;
use crate::error::ApiError;
use crate::state::AppState;
use crate::tenant::OrgId;

const JWT_EXPIRY_HOURS: i64 = 24;

// ── POST /api/signup — self-service tenant onboarding ──────────────────────

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub organization_name: String,
    pub email: String,
    pub password: String,
    pub plan: Option<String>,
}

pub async fn signup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SignupRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // Rate-limit: 5 signups per IP per hour
    let ip_str = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| s.parse::<std::net::IpAddr>().is_ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let recent_signups: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM signup_rate_limit WHERE ip_address = $1::inet AND created_at > NOW() - INTERVAL '1 hour'",
    )
    .bind(&ip_str)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    if recent_signups >= 5 {
        return Err(ApiError::from((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many signup attempts. Please try again later.".to_string(),
        )));
    }

    // Record this attempt
    let _ = sqlx::query("INSERT INTO signup_rate_limit (ip_address) VALUES ($1::inet)")
        .bind(&ip_str)
        .execute(&state.pool)
        .await;

    // Validate
    if req.password.len() < 12 {
        return Err(ApiError::BadRequest("Password must be at least 12 characters".into()));
    }

    // Check email uniqueness
    let email_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(&req.email)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);
    if email_exists {
        return Err(ApiError::BadRequest("An account with this email already exists".into()));
    }

    let plan = req.plan.as_deref().unwrap_or("starter");
    let (max_targets, max_users, max_dashboards, max_alert_rules, retention_days, api_rate_limit) =
        plan_defaults(plan);

    let org_id = Uuid::new_v4();
    let slug = req.organization_name
        .to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

    // Check slug uniqueness
    let slug_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organizations WHERE slug = $1)")
        .bind(&slug)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);
    if slug_exists {
        return Err(ApiError::BadRequest("An organization with this name already exists".into()));
    }

    let mut tx = state.pool.begin().await.map_err(|e| ApiError::Internal(e.to_string()))?;

    // 1. Create organization
    sqlx::query(
        "INSERT INTO organizations (org_id, name, slug, plan, status, owner_email, \
         max_targets, max_users, max_dashboards, max_alert_rules, retention_days, api_rate_limit) \
         VALUES ($1, $2, $3, $4, 'active', $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(org_id)
    .bind(&req.organization_name)
    .bind(&slug)
    .bind(plan)
    .bind(&req.email)
    .bind(max_targets)
    .bind(max_users)
    .bind(max_dashboards)
    .bind(max_alert_rules)
    .bind(retention_days)
    .bind(api_rate_limit)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // 2. Create org_settings
    let _ = sqlx::query(
        "INSERT INTO org_settings (org_id, max_targets, max_dashboards, max_alert_rules, max_users, retention_days) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (org_id) DO NOTHING",
    )
    .bind(org_id).bind(max_targets).bind(max_dashboards).bind(max_alert_rules).bind(max_users).bind(retention_days)
    .execute(&mut *tx)
    .await;

    // 3. Create system roles
    let role_defs = [
        ("Admin", "Full access to all resources", true),
        ("Org Admin", "Organization administrator", true),
        ("Editor", "Read/write access to dashboards and alerts", true),
        ("Viewer", "Read-only access", true),
        ("Agent", "Metric/log ingestion only", true),
    ];

    let mut admin_role_id: Option<Uuid> = None;
    for (name, desc, is_system) in &role_defs {
        let role_id = Uuid::new_v4();
        let _ = sqlx::query(
            "INSERT INTO roles (role_id, org_id, name, description, is_system) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (org_id, name) DO NOTHING",
        )
        .bind(role_id).bind(org_id).bind(name).bind(desc).bind(is_system)
        .execute(&mut *tx)
        .await;
        if *name == "Org Admin" || *name == "Admin" {
            if admin_role_id.is_none() {
                admin_role_id = Some(role_id);
            }
        }
    }

    // Grant all permissions to Admin roles
    for role_name in &["Admin", "Org Admin"] {
        let _ = sqlx::query(
            "INSERT INTO role_permissions (role_id, permission_id) \
             SELECT r.role_id, p.permission_id FROM roles r, permissions p \
             WHERE r.org_id = $1 AND r.name = $2 ON CONFLICT DO NOTHING",
        )
        .bind(org_id).bind(role_name)
        .execute(&mut *tx)
        .await;
    }

    // 4. Create admin user
    let user_id = Uuid::new_v4();
    let password_hash = bcrypt::hash(&req.password, 12)
        .map_err(|e| ApiError::Internal(format!("hash error: {}", e)))?;
    let username = req.email.split('@').next().unwrap_or("admin");

    sqlx::query(
        "INSERT INTO users (id, org_id, email, username, password_hash, display_name, is_active) \
         VALUES ($1, $2, $3, $4, $5, $6, true)",
    )
    .bind(user_id)
    .bind(org_id)
    .bind(&req.email)
    .bind(username)
    .bind(&password_hash)
    .bind("Administrator")
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Assign Admin role
    if let Some(rid) = admin_role_id {
        let _ = sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, org_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(user_id).bind(rid).bind(org_id)
        .execute(&mut *tx)
        .await;
    }

    // 5. Generate first agent API key (reusing the same logic as agents handler)
    use sha2::{Digest, Sha256};
    use rand::Rng;

    let raw_key = {
        let mut rng = rand::thread_rng();
        let suffix = (0..32).map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
        }).collect::<String>();
        format!("isok_{suffix}")
    };
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    let key_hash = format!("{:x}", hasher.finalize());
    let key_prefix = raw_key.chars().take(13).collect::<String>();

    let otlp_key_id: Uuid = sqlx::query_scalar(
        "INSERT INTO otlp_api_keys (org_id, name, key_hash, key_prefix, permissions, rate_limit_rpm, max_payload_bytes, enabled, created_at, updated_at) \
         VALUES ($1, 'signup-agent-key', $2, $3, $4, 10000, 5242880, true, NOW(), NOW()) RETURNING id",
    )
    .bind(org_id)
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(vec!["traces".to_string(), "metrics".to_string(), "logs".to_string(), "profiles".to_string()])
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let agent_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_fleet (agent_id, org_id, otlp_api_key_id, api_key_hash, datacenter_name, status, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, 'default', 'active', NOW(), NOW())",
    )
    .bind(agent_id)
    .bind(org_id)
    .bind(otlp_key_id)
    .bind(&key_hash)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // 6. Seed default dashboards
    let _ = crate::provisioning::seed_org_dashboards(&mut tx, org_id).await;

    tx.commit().await.map_err(|e| ApiError::Internal(e.to_string()))?;

    // 7. Generate JWT for immediate login
    let session_id = Uuid::new_v4();
    let now = Utc::now();
    let exp = now + chrono::Duration::hours(JWT_EXPIRY_HOURS);

    let claims = JwtClaims {
        sub: user_id,
        org: org_id,
        email: req.email.clone(),
        role: "Admin".into(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
        jti: session_id.to_string(),
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| ApiError::Internal(format!("JWT encode error: {}", e)))?;

    // Store session
    let _ = sqlx::query(
        "INSERT INTO user_sessions (session_id, user_id, token_hash, ip_address, expires_at) \
         VALUES ($1, $2, $3, '0.0.0.0'::inet, $4)",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&format!("{:x}", {
        let mut h = Sha256::new();
        h.update(token.as_bytes());
        h.finalize()
    }))
    .bind(exp)
    .execute(&state.pool)
    .await;

    Ok((StatusCode::CREATED, Json(json!({
        "org_id": org_id,
        "token": token,
        "agent_api_key": raw_key,
        "warning": "Store the agent API key securely. It will not be shown again.",
    }))))
}

// ── POST /api/signup/verify-email ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

pub async fn verify_email(
    State(state): State<AppState>,
    Json(req): Json<VerifyEmailRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updated = sqlx::query(
        "UPDATE organizations SET email_verified = true, updated_at = NOW() \
         WHERE email_verify_token = $1 AND email_verify_expires > NOW()",
    )
    .bind(&req.token)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if updated.rows_affected() == 0 {
        return Err(ApiError::BadRequest("Invalid or expired verification token".into()));
    }

    Ok(Json(json!({"verified": true})))
}

// ── GET /api/onboarding/checklist ──────────────────────────────────────────

pub async fn onboarding_checklist(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Email verified
    let email_verified: bool = sqlx::query_scalar(
        "SELECT COALESCE(email_verified, false) FROM organizations WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .unwrap_or(None)
    .unwrap_or(false);

    // Agent registered and active
    let agent_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_fleet WHERE org_id = $1 AND status = 'active')",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    // First target added
    let has_target: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM targets WHERE org_id = $1 AND is_active = true)",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    // Metric received in last hour
    let has_recent_metric: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 hour')",
    )
    .bind(org_id)
    .fetch_one(&state.tsdb_pool)
    .await
    .unwrap_or(false);

    // Dashboard viewed (check audit_log for dashboard views)
    let dashboard_viewed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM audit_log WHERE org_id = $1 AND action = 'view_dashboard')",
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    Ok(Json(json!({
        "email_verified": email_verified,
        "agent_registered": agent_active,
        "target_added": has_target,
        "metric_received": has_recent_metric,
        "dashboard_viewed": dashboard_viewed,
        "all_complete": email_verified && agent_active && has_target && has_recent_metric && dashboard_viewed,
    })))
}

// ── Quota enforcement helpers ──────────────────────────────────────────────

pub async fn check_target_quota(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<(), ApiError> {
    let (current, max_val): (i64, i32) = {
        let current: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM targets WHERE org_id = $1 AND is_active = true")
            .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
        let max_val: i32 = sqlx::query_scalar("SELECT max_targets FROM org_settings WHERE org_id = $1")
            .bind(org_id).fetch_one(&state.pool).await.unwrap_or(10);
        (current, max_val)
    };

    if current >= max_val as i64 {
        return Err(ApiError::from((
            StatusCode::PAYMENT_REQUIRED,
            format!(
                "Target limit reached ({}/{}). Upgrade to Professional for up to 100 targets.",
                current, max_val
            ),
        )));
    }
    Ok(())
}

pub async fn check_user_quota(pool: &sqlx::PgPool, org_id: Uuid) -> Result<(), ApiError> {
    let current: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE org_id = $1 AND is_active = true")
        .bind(org_id).fetch_one(pool).await.unwrap_or(0);
    let max_val: i32 = sqlx::query_scalar("SELECT max_users FROM org_settings WHERE org_id = $1")
        .bind(org_id).fetch_one(pool).await.unwrap_or(5);

    if current >= max_val as i64 {
        return Err(ApiError::from((
            StatusCode::PAYMENT_REQUIRED,
            format!("User limit reached ({}/{}). Upgrade your plan for more users.", current, max_val),
        )));
    }
    Ok(())
}

pub async fn check_dashboard_quota(pool: &sqlx::PgPool, org_id: Uuid) -> Result<(), ApiError> {
    let current: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dashboards WHERE org_id = $1")
        .bind(org_id).fetch_one(pool).await.unwrap_or(0);
    let max_val: i32 = sqlx::query_scalar("SELECT max_dashboards FROM org_settings WHERE org_id = $1")
        .bind(org_id).fetch_one(pool).await.unwrap_or(10);

    if current >= max_val as i64 {
        return Err(ApiError::from((
            StatusCode::PAYMENT_REQUIRED,
            format!("Dashboard limit reached ({}/{}). Upgrade your plan for more dashboards.", current, max_val),
        )));
    }
    Ok(())
}

fn plan_defaults(plan: &str) -> (i32, i32, i32, i32, i32, i32) {
    match plan {
        "enterprise"   => (500, 50, 200, 1000, 365, 10000),
        "professional" => (100, 20,  50,  200,  90,  2000),
        "starter"      => ( 10,  5,  10,   50,  30,   500),
        _              => (  3,  2,   5,   20,  14,   100),
    }
}
