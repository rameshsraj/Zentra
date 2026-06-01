use axum::{
    extract::{Path, Query, State, Json},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
use crate::state::AppState;
use crate::auth_middleware::AuthContext;

#[allow(unused_imports)]
use crate::error::ApiError;

// ============================================================================
// Multi-Tenancy — Organization lifecycle management (database-backed)
//
// The `organizations` table is the authoritative parent for every `org_id`
// referenced by users, roles, api_keys, audit_log, org_settings, etc.
//
// Only Super Admins may create / suspend / delete organizations.
// Regular admins see only their own org.
// ============================================================================

// ── Request / Query structs ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: Option<String>,
    pub plan: Option<String>,
    pub owner_email: String,
    pub owner_username: Option<String>,
    pub owner_password: Option<String>,
    pub max_targets: Option<i32>,
    pub max_users: Option<i32>,
    pub max_dashboards: Option<i32>,
    pub max_alert_rules: Option<i32>,
    pub retention_days: Option<i32>,
    pub region: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TenantListParams {
    pub status: Option<String>,
    pub plan: Option<String>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn is_super_admin(ctx: &AuthContext) -> bool {
    ctx.role == "Super Admin"
}

fn require_super_admin(ctx: &AuthContext) -> Result<(), Response> {
    if is_super_admin(ctx) {
        Ok(())
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "forbidden", "message": "Only Super Admins can manage organizations"})),
        ).into_response())
    }
}

fn plan_defaults(plan: &str) -> (i32, i32, i32, i32, i32, i32) {
    // (max_targets, max_users, max_dashboards, max_alert_rules, retention_days, api_rate_limit)
    match plan {
        "enterprise"   => (500, 50, 200, 1000, 365, 10000),
        "professional" => (100, 20,  50,  200,  90,  2000),
        "starter"      => ( 10,  5,  10,   50,  30,   500),
        _              => (  3,  2,   5,   20,  14,   100),
    }
}

fn row_to_org_json(r: &sqlx::postgres::PgRow) -> serde_json::Value {
    json!({
        "org_id": r.get::<Uuid, _>("org_id"),
        "name": r.get::<String, _>("name"),
        "slug": r.get::<String, _>("slug"),
        "plan": r.get::<String, _>("plan"),
        "status": r.get::<String, _>("status"),
        "owner_email": r.get::<String, _>("owner_email"),
        "trial_ends_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("trial_ends_at"),
        "max_targets": r.get::<i32, _>("max_targets"),
        "max_users": r.get::<i32, _>("max_users"),
        "max_dashboards": r.get::<i32, _>("max_dashboards"),
        "max_alert_rules": r.get::<i32, _>("max_alert_rules"),
        "retention_days": r.get::<i32, _>("retention_days"),
        "features": r.get::<serde_json::Value, _>("features"),
        "custom_branding": r.get::<bool, _>("custom_branding"),
        "sso_enabled": r.get::<bool, _>("sso_enabled"),
        "api_rate_limit": r.get::<i32, _>("api_rate_limit"),
        "region": r.get::<String, _>("region"),
        "notes": r.get::<Option<String>, _>("notes"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
        "updated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at"),
    })
}

// ── GET /api/tenants — list organizations ──────────────────────────────────

pub async fn list_tenants(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Query(params): Query<TenantListParams>,
) -> impl IntoResponse {
    let sql = "SELECT org_id, name, slug, plan, status, owner_email, trial_ends_at, \
         max_targets, max_users, max_dashboards, max_alert_rules, retention_days, \
         features, custom_branding, sso_enabled, api_rate_limit, region, notes, \
         created_at, updated_at \
         FROM organizations";

    // Super Admin sees all orgs; everyone else sees only their own
    let rows = if is_super_admin(&ctx) {
        sqlx::query(&format!(
            "{} WHERE ($1::text IS NULL OR status = $1) AND ($2::text IS NULL OR plan = $2) ORDER BY created_at DESC", sql
        ))
        .bind(params.status.as_deref())
        .bind(params.plan.as_deref())
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query(&format!("{} WHERE org_id = $1", sql))
        .bind(ctx.org_id)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default()
    };

    let mut orgs = Vec::new();
    for r in &rows {
        let org_id: Uuid = r.get("org_id");
        let current_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE org_id = $1 AND is_active = true")
            .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
        let current_targets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM targets WHERE org_id = $1")
            .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

        let mut obj = row_to_org_json(r);
        if let Some(map) = obj.as_object_mut() {
            map.insert("current_users".into(), json!(current_users));
            map.insert("current_targets".into(), json!(current_targets));
        }
        orgs.push(obj);
    }

    (StatusCode::OK, Json(json!(orgs))).into_response()
}

// ── GET /api/tenants/:id — single organization ────────────────────────────

pub async fn get_tenant(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Accept UUID or slug
    let row = sqlx::query(
        "SELECT org_id, name, slug, plan, status, owner_email, trial_ends_at, \
         max_targets, max_users, max_dashboards, max_alert_rules, retention_days, \
         features, custom_branding, sso_enabled, api_rate_limit, region, notes, \
         created_at, updated_at \
         FROM organizations WHERE org_id::text = $1 OR slug = $1"
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await;

    let r = match row {
        Ok(Some(r)) => r,
        _ => return (StatusCode::NOT_FOUND, Json(json!({"error": "organization_not_found"}))).into_response(),
    };

    let org_id: Uuid = r.get("org_id");

    // Non-super-admins can only view their own org
    if !is_super_admin(&ctx) && org_id != ctx.org_id {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden", "message": "Cannot view other organizations"}))).into_response();
    }

    let current_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE org_id = $1 AND is_active = true")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
    let current_targets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM targets WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    let mut obj = row_to_org_json(&r);
    if let Some(map) = obj.as_object_mut() {
        map.insert("current_users".into(), json!(current_users));
        map.insert("current_targets".into(), json!(current_targets));
    }

    (StatusCode::OK, Json(obj)).into_response()
}

// ── POST /api/tenants — create organization + onboard ──────────────────────
//
// Onboarding creates:
//   1. organizations row
//   2. org_settings row
//   3. System roles (Admin, Editor, Viewer, Agent, Super Admin, Operator)
//   4. Role permissions (copied from the permission catalogue)
//   5. Initial admin user with Admin role
//   6. Audit log entry

pub async fn create_tenant(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Json(req): Json<CreateTenantRequest>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&ctx) { return resp; }

    let slug = req.slug.unwrap_or_else(|| {
        req.name.to_lowercase()
            .replace(' ', "-")
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
    });

    // Check slug uniqueness
    let slug_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM organizations WHERE slug = $1)")
        .bind(&slug).fetch_one(&state.pool).await.unwrap_or(false);
    if slug_exists {
        return (StatusCode::CONFLICT, Json(json!({"error": "slug_exists", "message": "An organization with this slug already exists"}))).into_response();
    }

    let plan = req.plan.as_deref().unwrap_or("starter");
    let (def_targets, def_users, def_dashboards, def_alerts, def_retention, def_rate) = plan_defaults(plan);

    let org_id = Uuid::new_v4();
    let max_targets = req.max_targets.unwrap_or(def_targets);
    let max_users = req.max_users.unwrap_or(def_users);
    let max_dashboards = req.max_dashboards.unwrap_or(def_dashboards);
    let max_alert_rules = req.max_alert_rules.unwrap_or(def_alerts);
    let retention_days = req.retention_days.unwrap_or(def_retention);
    let region = req.region.as_deref().unwrap_or("default");
    let trial_ends_at: Option<chrono::DateTime<chrono::Utc>> = if plan != "enterprise" {
        Some(chrono::Utc::now() + chrono::Duration::days(14))
    } else {
        None
    };

    // ── 1. Create organization ──────────────────────────────────────────
    let result = sqlx::query(
        "INSERT INTO organizations (org_id, name, slug, plan, status, owner_email, trial_ends_at, \
         max_targets, max_users, max_dashboards, max_alert_rules, retention_days, \
         api_rate_limit, region, notes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
    )
    .bind(org_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(plan)
    .bind(if plan == "enterprise" { "active" } else { "trial" })
    .bind(&req.owner_email)
    .bind(trial_ends_at)
    .bind(max_targets)
    .bind(max_users)
    .bind(max_dashboards)
    .bind(max_alert_rules)
    .bind(retention_days)
    .bind(def_rate)
    .bind(region)
    .bind(req.notes.as_deref())
    .execute(&state.pool)
    .await;

    if let Err(e) = result {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "create_failed", "message": e.to_string()}))).into_response();
    }

    // ── 2. Create org_settings ──────────────────────────────────────────
    let _ = sqlx::query(
        "INSERT INTO org_settings (org_id, max_targets, max_dashboards, max_alert_rules, max_users, retention_days) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (org_id) DO NOTHING"
    )
    .bind(org_id).bind(max_targets).bind(max_dashboards).bind(max_alert_rules).bind(max_users).bind(retention_days)
    .execute(&state.pool).await;

    // ── 3. Create system roles for the new org ──────────────────────────
    let role_defs = vec![
        ("Admin",       "Full access to all resources",                              true),
        ("Super Admin", "Full access including user management and system config",   true),
        ("Operator",    "All monitoring read/write, acknowledge alerts",             true),
        ("Editor",      "Read/write access to dashboards and alerts",                true),
        ("Viewer",      "Read-only access",                                          true),
        ("Agent",       "Metric/log ingestion only",                                 true),
    ];

    let mut admin_role_id: Option<Uuid> = None;
    for (name, desc, is_system) in &role_defs {
        let role_id = Uuid::new_v4();
        let _ = sqlx::query(
            "INSERT INTO roles (role_id, org_id, name, description, is_system) \
             VALUES ($1, $2, $3, $4, $5) ON CONFLICT (org_id, name) DO NOTHING"
        )
        .bind(role_id).bind(org_id).bind(name).bind(desc).bind(is_system)
        .execute(&state.pool).await;

        if *name == "Admin" { admin_role_id = Some(role_id); }
    }

    // ── 4. Grant permissions to the new roles ───────────────────────────
    // Admin + Super Admin get all permissions
    for role_name in &["Admin", "Super Admin"] {
        let _ = sqlx::query(
            "INSERT INTO role_permissions (role_id, permission_id) \
             SELECT r.role_id, p.permission_id FROM roles r, permissions p \
             WHERE r.org_id = $1 AND r.name = $2 \
             ON CONFLICT DO NOTHING"
        ).bind(org_id).bind(role_name).execute(&state.pool).await;
    }

    // Operator: read + write + acknowledge (no admin, no user mgmt)
    let _ = sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT r.role_id, p.permission_id FROM roles r, permissions p \
         WHERE r.org_id = $1 AND r.name = 'Operator' \
         AND p.action IN ('read', 'write', 'acknowledge') AND p.resource NOT IN ('user', 'admin') \
         ON CONFLICT DO NOTHING"
    ).bind(org_id).execute(&state.pool).await;

    // Editor: read + write + acknowledge
    let _ = sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT r.role_id, p.permission_id FROM roles r, permissions p \
         WHERE r.org_id = $1 AND r.name = 'Editor' \
         AND p.action IN ('read', 'write', 'acknowledge') \
         ON CONFLICT DO NOTHING"
    ).bind(org_id).execute(&state.pool).await;

    // Viewer: read only
    let _ = sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT r.role_id, p.permission_id FROM roles r, permissions p \
         WHERE r.org_id = $1 AND r.name = 'Viewer' AND p.action = 'read' \
         ON CONFLICT DO NOTHING"
    ).bind(org_id).execute(&state.pool).await;

    // Agent: metrics + logs write
    let _ = sqlx::query(
        "INSERT INTO role_permissions (role_id, permission_id) \
         SELECT r.role_id, p.permission_id FROM roles r, permissions p \
         WHERE r.org_id = $1 AND r.name = 'Agent' \
         AND p.resource IN ('metrics', 'logs') AND p.action = 'write' \
         ON CONFLICT DO NOTHING"
    ).bind(org_id).execute(&state.pool).await;

    // ── 5. Create initial admin user ────────────────────────────────────
    let admin_user_id = Uuid::new_v4();
    let username = req.owner_username.as_deref().unwrap_or("admin");
    let raw_password = req.owner_password.as_deref().unwrap_or("changeme");
    let password_hash = bcrypt::hash(raw_password, 12).unwrap_or_default();

    let _ = sqlx::query(
        "INSERT INTO users (id, org_id, email, username, password_hash, display_name) \
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING"
    )
    .bind(admin_user_id).bind(org_id).bind(&req.owner_email).bind(username).bind(&password_hash).bind("Administrator")
    .execute(&state.pool).await;

    // Assign Admin role
    if let Some(rid) = admin_role_id {
        let _ = sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, org_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
        ).bind(admin_user_id).bind(rid).bind(org_id).execute(&state.pool).await;
    }

    // ── 6. Audit log ────────────────────────────────────────────────────
    let _ = sqlx::query(
        "INSERT INTO audit_log (org_id, user_id, action, resource_type, resource_id, details, status) \
         VALUES ($1, $2, 'create_organization', 'organization', $3, $4, 'success')"
    )
    .bind(ctx.org_id)
    .bind(ctx.user_id)
    .bind(org_id.to_string())
    .bind(json!({"org_name": req.name, "slug": slug, "plan": plan, "owner_email": req.owner_email}))
    .execute(&state.pool).await;

    (StatusCode::CREATED, Json(json!({
        "org_id": org_id,
        "name": req.name,
        "slug": slug,
        "plan": plan,
        "owner_email": req.owner_email,
        "admin_user_id": admin_user_id,
        "admin_username": username,
        "message": "Organization created and onboarded successfully"
    }))).into_response()
}

// ── PUT /api/tenants/:id — update organization ─────────────────────────────

pub async fn update_tenant(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&ctx) { return resp; }

    let org_id: Option<Uuid> = sqlx::query_scalar("SELECT org_id FROM organizations WHERE org_id::text = $1 OR slug = $1")
        .bind(&id).fetch_optional(&state.pool).await.unwrap_or(None);

    let org_id = match org_id {
        Some(id) => id,
        None => return (StatusCode::NOT_FOUND, Json(json!({"error": "organization_not_found"}))).into_response(),
    };

    let _ = sqlx::query(
        "UPDATE organizations SET \
         name = COALESCE($1, name), \
         plan = COALESCE($2, plan), \
         max_targets = COALESCE($3, max_targets), \
         max_users = COALESCE($4, max_users), \
         max_dashboards = COALESCE($5, max_dashboards), \
         max_alert_rules = COALESCE($6, max_alert_rules), \
         retention_days = COALESCE($7, retention_days), \
         sso_enabled = COALESCE($8, sso_enabled), \
         custom_branding = COALESCE($9, custom_branding), \
         notes = COALESCE($10, notes), \
         region = COALESCE($11, region), \
         updated_at = NOW() \
         WHERE org_id = $12"
    )
    .bind(req.get("name").and_then(|v| v.as_str()))
    .bind(req.get("plan").and_then(|v| v.as_str()))
    .bind(req.get("max_targets").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("max_users").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("max_dashboards").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("max_alert_rules").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("retention_days").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("sso_enabled").and_then(|v| v.as_bool()))
    .bind(req.get("custom_branding").and_then(|v| v.as_bool()))
    .bind(req.get("notes").and_then(|v| v.as_str()))
    .bind(req.get("region").and_then(|v| v.as_str()))
    .bind(org_id)
    .execute(&state.pool).await;

    // Sync org_settings
    let _ = sqlx::query(
        "UPDATE org_settings SET \
         max_targets = COALESCE($1, max_targets), \
         max_dashboards = COALESCE($2, max_dashboards), \
         max_alert_rules = COALESCE($3, max_alert_rules), \
         max_users = COALESCE($4, max_users), \
         retention_days = COALESCE($5, retention_days), \
         updated_at = NOW() \
         WHERE org_id = $6"
    )
    .bind(req.get("max_targets").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("max_dashboards").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("max_alert_rules").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("max_users").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(req.get("retention_days").and_then(|v| v.as_i64()).map(|v| v as i32))
    .bind(org_id)
    .execute(&state.pool).await;

    (StatusCode::OK, Json(json!({"message": "Organization updated", "org_id": org_id}))).into_response()
}

// ── DELETE /api/tenants/:id — soft-delete (set status=cancelled) ───────────

pub async fn delete_tenant(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&ctx) { return resp; }

    // Prevent deleting the default org
    if id == "00000000-0000-0000-0000-000000000001" || id == "default" {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "cannot_delete_default_org", "message": "The default organization cannot be deleted"}))).into_response();
    }

    let result = sqlx::query(
        "UPDATE organizations SET status = 'cancelled', updated_at = NOW() WHERE org_id::text = $1 OR slug = $1"
    ).bind(&id).execute(&state.pool).await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            // Deactivate all users in the cancelled org
            let _ = sqlx::query(
                "UPDATE users SET is_active = false WHERE org_id = (SELECT org_id FROM organizations WHERE org_id::text = $1 OR slug = $1)"
            ).bind(&id).execute(&state.pool).await;

            (StatusCode::OK, Json(json!({"message": "Organization cancelled", "id": id}))).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "organization_not_found"}))).into_response(),
    }
}

// ── POST /api/tenants/:id/suspend ──────────────────────────────────────────

pub async fn suspend_tenant(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&ctx) { return resp; }

    let result = sqlx::query(
        "UPDATE organizations SET status = 'suspended', updated_at = NOW() WHERE org_id::text = $1 OR slug = $1"
    ).bind(&id).execute(&state.pool).await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"message": "Organization suspended", "id": id}))).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "organization_not_found"}))).into_response(),
    }
}

// ── POST /api/tenants/:id/activate ─────────────────────────────────────────

pub async fn activate_tenant(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&ctx) { return resp; }

    let result = sqlx::query(
        "UPDATE organizations SET status = 'active', trial_ends_at = NULL, updated_at = NOW() WHERE org_id::text = $1 OR slug = $1"
    ).bind(&id).execute(&state.pool).await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"message": "Organization activated", "id": id}))).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "organization_not_found"}))).into_response(),
    }
}

// ── GET /api/tenants/:id/usage — live usage stats ──────────────────────────

pub async fn tenant_usage(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let org = sqlx::query_as::<_, (Uuid, String, i32, i32)>(
        "SELECT org_id, name, max_targets, max_users FROM organizations WHERE org_id::text = $1 OR slug = $1"
    ).bind(&id).fetch_optional(&state.pool).await;

    let (org_id, org_name, max_t, max_u) = match org {
        Ok(Some(r)) => r,
        _ => return (StatusCode::NOT_FOUND, Json(json!({"error": "organization_not_found"}))).into_response(),
    };

    if !is_super_admin(&ctx) && org_id != ctx.org_id {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "forbidden"}))).into_response();
    }

    let targets_used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM targets WHERE org_id = $1")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
    let users_used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE org_id = $1 AND is_active = true")
        .bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);
    let alerts_30d: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM active_alerts WHERE org_id = $1 AND fired_at > NOW() - INTERVAL '30 days'"
    ).bind(org_id).fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "org_id": org_id,
        "org_name": org_name,
        "targets_used": targets_used,
        "targets_limit": max_t,
        "users_used": users_used,
        "users_limit": max_u,
        "alerts_fired_30d": alerts_30d,
    }))).into_response()
}

// ── GET /api/tenants/summary — aggregate stats across all orgs ─────────────

pub async fn tenants_summary(
    State(state): State<AppState>,
    axum::Extension(ctx): axum::Extension<AuthContext>,
) -> impl IntoResponse {
    if let Err(resp) = require_super_admin(&ctx) { return resp; }

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let active: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE status = 'active'")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let trial: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE status = 'trial'")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let suspended: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE status = 'suspended'")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_active = true")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let total_targets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM targets")
        .fetch_one(&state.pool).await.unwrap_or(0);

    // Plan breakdown
    let enterprise: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE plan = 'enterprise'")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let professional: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE plan = 'professional'")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let starter: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE plan = 'starter'")
        .fetch_one(&state.pool).await.unwrap_or(0);
    let free: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM organizations WHERE plan = 'free'")
        .fetch_one(&state.pool).await.unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "total_tenants": total,
        "active_tenants": active,
        "trial_tenants": trial,
        "suspended_tenants": suspended,
        "total_monitored_targets": total_targets,
        "total_users": total_users,
        "by_plan": {
            "enterprise": enterprise,
            "professional": professional,
            "starter": starter,
            "free": free,
        },
    }))).into_response()
}
