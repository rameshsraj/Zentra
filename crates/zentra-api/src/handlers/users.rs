use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use sqlx;
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;
use crate::auth_middleware::AuthContext;
use zentra_core::models::auth::*;

// ═══════════════════════════════════════════════════════════════════════════
// Query params
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct ListUsersQuery {
    pub search: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,  // "active", "locked", "disabled"
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/users — paginated list with filters
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_users(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ListUsersQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let search_filter = params.search.as_deref().unwrap_or("");
    let search_pattern = format!("%{}%", search_filter);

    // Single JOIN query with ARRAY_AGG to eliminate the N+1 roles fetch.
    // org_id scoping ($1) enforces tenant isolation.
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, Option<String>, bool, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, Vec<String>)>(
        "SELECT u.id, u.org_id, u.email, u.username, u.display_name, u.avatar_url, \
         u.is_active, u.is_locked, u.last_login, u.created_at, \
         COALESCE(ARRAY_AGG(r.name ORDER BY r.name) FILTER (WHERE r.name IS NOT NULL), ARRAY[]::text[]) AS roles \
         FROM users u \
         LEFT JOIN user_roles ur ON ur.user_id = u.id AND ur.org_id = u.org_id \
         LEFT JOIN roles r ON r.role_id = ur.role_id \
         WHERE u.org_id = $1 \
         AND ($2 = '' OR u.email ILIKE $3 OR u.username ILIKE $3 OR u.display_name ILIKE $3) \
         AND ($4 = '' OR \
              ($4 = 'active' AND u.is_active = true AND u.is_locked = false) OR \
              ($4 = 'locked' AND u.is_locked = true) OR \
              ($4 = 'disabled' AND u.is_active = false)) \
         GROUP BY u.id \
         ORDER BY u.created_at DESC LIMIT $5 OFFSET $6"
    )
    .bind(org_id)
    .bind(search_filter)
    .bind(&search_pattern)
    .bind(params.status.as_deref().unwrap_or(""))
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut users = Vec::new();
    for row in &rows {
        let roles = &row.10;

        // Filter by role if requested
        if let Some(ref role_filter) = params.role {
            if !roles.iter().any(|r| r.eq_ignore_ascii_case(role_filter)) {
                continue;
            }
        }

        users.push(json!({
            "id": row.0,
            "org_id": row.1,
            "email": row.2,
            "username": row.3,
            "display_name": row.4,
            "avatar_url": row.5,
            "is_active": row.6,
            "is_locked": row.7,
            "last_login": row.8,
            "created_at": row.9,
            "roles": roles,
        }));
    }

    let total: i64 = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE org_id = $1"
    )
    .bind(org_id)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    (StatusCode::OK, Json(json!({
        "users": users,
        "total": total,
        "limit": limit,
        "offset": offset,
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/users — create user (role-enforced)
//
// Business rules:
//   • Super Admin → can create Admins only, must specify target org_id
//   • Admin       → can create Users only, within their own org
//   • Others      → forbidden
// ═══════════════════════════════════════════════════════════════════════════

pub async fn create_user(
    State(state): State<AppState>,
    OrgId(caller_org_id): OrgId,
    axum::Extension(ctx): axum::Extension<AuthContext>,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    // Quota enforcement
    if let Err(e) = crate::handlers::signup::check_user_quota(&state.pool, caller_org_id).await {
        return (StatusCode::PAYMENT_REQUIRED, Json(json!({
            "error": "quota_exceeded",
            "message": e.to_string()
        }))).into_response();
    }

    let caller_role = ctx.role.as_str();

    // ── Role gate ────────────────────────────────────────────────────────
    let is_super = caller_role == "Super Admin";
    let is_admin = caller_role == "Admin";

    if !is_super && !is_admin {
        return (StatusCode::FORBIDDEN, Json(json!({
            "error": "forbidden",
            "message": "Only Admins can create users"
        }))).into_response();
    }

    // ── Determine target org_id ──────────────────────────────────────────
    let target_org_id = if is_super {
        // Super Admin MUST supply org_id for the new Admin
        match req.org_id {
            Some(oid) => {
                // Verify org exists and is active
                let org_ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM organizations WHERE org_id = $1 AND status IN ('active','trial'))"
                ).bind(oid).fetch_one(&state.pool).await.unwrap_or(false);
                if !org_ok {
                    return (StatusCode::BAD_REQUEST, Json(json!({
                        "error": "invalid_org",
                        "message": "Organization does not exist or is not active"
                    }))).into_response();
                }
                oid
            }
            None => {
                return (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "org_required",
                    "message": "Super Admin must specify org_id when creating an Admin"
                }))).into_response();
            }
        }
    } else {
        // Admin creates within their own org; ignore any supplied org_id
        caller_org_id
    };

    // ── Role assignment enforcement ──────────────────────────────────────
    // Resolve the role names the caller is trying to assign
    if let Some(ref role_ids) = req.role_ids {
        if !role_ids.is_empty() {
            let placeholders: Vec<String> = (1..=role_ids.len()).map(|i| format!("${}", i)).collect();
            let sql = format!(
                "SELECT name FROM roles WHERE role_id IN ({}) AND org_id = ${}",
                placeholders.join(", "),
                role_ids.len() + 1
            );
            let mut query = sqlx::query_scalar::<_, String>(&sql);
            for rid in role_ids {
                query = query.bind(rid);
            }
            query = query.bind(target_org_id);
            let assigned_roles: Vec<String> = query.fetch_all(&state.pool).await.unwrap_or_default();

            if is_super {
                // Super Admin can only create Admins — must include Admin role, must not include
                // regular user-only roles without Admin
                let has_admin = assigned_roles.iter().any(|r| r == "Admin");
                if !has_admin {
                    return (StatusCode::BAD_REQUEST, Json(json!({
                        "error": "role_required",
                        "message": "Super Admin must assign the Admin role when creating users"
                    }))).into_response();
                }
            } else {
                // Admin cannot assign Admin or Super Admin roles
                let assigns_elevated = assigned_roles.iter().any(|r| r == "Admin" || r == "Super Admin");
                if assigns_elevated {
                    return (StatusCode::FORBIDDEN, Json(json!({
                        "error": "forbidden",
                        "message": "Admin cannot assign Admin or Super Admin roles"
                    }))).into_response();
                }
            }
        }
    } else if is_super {
        // Super Admin did not select any roles — reject
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": "role_required",
            "message": "Super Admin must assign the Admin role when creating users"
        }))).into_response();
    }

    // ── Check email uniqueness ───────────────────────────────────────────
    let existing = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    if existing > 0 {
        return (StatusCode::CONFLICT, Json(json!({
            "error": "email_exists",
            "message": "A user with this email already exists"
        }))).into_response();
    }

    let password_hash = match bcrypt::hash(&req.password, 12) {
        Ok(h) => h,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                "error": "hash_failed"
            }))).into_response();
        }
    };

    let user_id = Uuid::new_v4();

    let result = sqlx::query(
        "INSERT INTO users (id, org_id, email, username, password_hash, display_name) \
         VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(user_id)
    .bind(target_org_id)
    .bind(&req.email)
    .bind(&req.username)
    .bind(&password_hash)
    .bind(&req.display_name)
    .execute(&state.pool)
    .await;

    if let Err(e) = result {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": "create_failed",
            "message": e.to_string()
        }))).into_response();
    }

    // Assign roles if provided
    if let Some(role_ids) = &req.role_ids {
        for role_id in role_ids {
            let _ = sqlx::query(
                "INSERT INTO user_roles (user_id, role_id, org_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
            )
            .bind(user_id)
            .bind(role_id)
            .bind(target_org_id)
            .execute(&state.pool)
            .await;
        }
    } else {
        // Assign default Viewer role (only for Admin-created users)
        let _ = sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, org_id) \
             SELECT $1, role_id, $2 FROM roles WHERE name = 'Viewer' AND org_id = $2 LIMIT 1"
        )
        .bind(user_id)
        .bind(target_org_id)
        .execute(&state.pool)
        .await;
    }

    // Audit
    let _ = sqlx::query(
        "INSERT INTO audit_log (org_id, user_id, action, resource_type, resource_id, details, status) \
         VALUES ($1, $2, 'create_user', 'user', $3, $4, 'success')"
    )
    .bind(target_org_id)
    .bind(ctx.user_id)
    .bind(user_id.to_string())
    .bind(json!({"email": req.email, "username": req.username, "created_by": ctx.email, "target_org": target_org_id}))
    .execute(&state.pool)
    .await;

    (StatusCode::CREATED, Json(json!({
        "id": user_id,
        "org_id": target_org_id,
        "email": req.email,
        "username": req.username,
        "message": "User created successfully"
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/users/:id — detail with roles
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    let user = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, Option<String>, String, String, bool, bool, Option<chrono::DateTime<chrono::Utc>>, bool, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, org_id, email, username, display_name, avatar_url, timezone, theme, \
         is_active, is_locked, last_login, mfa_enabled, password_changed_at, created_at, updated_at \
         FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await;

    let user = match user {
        Ok(Some(u)) => u,
        _ => {
            return (StatusCode::NOT_FOUND, Json(json!({"error": "user_not_found"}))).into_response();
        }
    };

    let roles: Vec<serde_json::Value> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT r.role_id, r.name FROM roles r JOIN user_roles ur ON ur.role_id = r.role_id WHERE ur.user_id = $1"
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name)| json!({"role_id": id, "name": name}))
    .collect();

    let groups: Vec<serde_json::Value> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT g.group_id, g.name FROM user_groups g JOIN user_group_members m ON m.group_id = g.group_id WHERE m.user_id = $1"
    )
    .bind(user_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, name)| json!({"id": id, "name": name}))
    .collect();

    (StatusCode::OK, Json(json!({
        "id": user.0,
        "org_id": user.1,
        "email": user.2,
        "username": user.3,
        "display_name": user.4,
        "avatar_url": user.5,
        "timezone": user.6,
        "theme": user.7,
        "is_active": user.8,
        "is_locked": user.9,
        "last_login": user.10,
        "mfa_enabled": user.11,
        "password_changed_at": user.12,
        "created_at": user.13,
        "updated_at": user.14,
        "roles": roles,
        "groups": groups,
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/users/:id — update user
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE users SET \
         email = COALESCE($1, email), \
         username = COALESCE($2, username), \
         display_name = COALESCE($3, display_name), \
         avatar_url = COALESCE($4, avatar_url), \
         timezone = COALESCE($5, timezone), \
         theme = COALESCE($6, theme), \
         is_active = COALESCE($7, is_active), \
         updated_at = NOW() \
         WHERE id = $8"
    )
    .bind(&req.email)
    .bind(&req.username)
    .bind(&req.display_name)
    .bind(&req.avatar_url)
    .bind(&req.timezone)
    .bind(&req.theme)
    .bind(&req.is_active)
    .bind(user_id)
    .execute(&state.pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"message": "User updated", "id": user_id}))).into_response()
        }
        Ok(_) => {
            (StatusCode::NOT_FOUND, Json(json!({"error": "user_not_found"}))).into_response()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/users/:id — soft delete
// ═══════════════════════════════════════════════════════════════════════════

pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    // Prevent deactivating the Super Admin
    let is_super: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.role_id = ur.role_id WHERE ur.user_id = $1 AND r.name = 'Super Admin')"
    ).bind(user_id).fetch_one(&state.pool).await.unwrap_or(false);
    if is_super {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "cannot_deactivate_super_admin", "message": "The Super Admin account cannot be deactivated."}))).into_response();
    }

    let _ = sqlx::query("UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;

    (StatusCode::OK, Json(json!({"message": "User deactivated", "id": user_id}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/users/:id/lock
// ═══════════════════════════════════════════════════════════════════════════

pub async fn lock_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    // Prevent locking the Super Admin
    let is_super: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_roles ur JOIN roles r ON r.role_id = ur.role_id WHERE ur.user_id = $1 AND r.name = 'Super Admin')"
    ).bind(user_id).fetch_one(&state.pool).await.unwrap_or(false);
    if is_super {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "cannot_lock_super_admin", "message": "The Super Admin account cannot be locked."}))).into_response();
    }

    let _ = sqlx::query("UPDATE users SET is_locked = true, updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;

    (StatusCode::OK, Json(json!({"message": "User locked", "id": user_id}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/users/:id/unlock
// ═══════════════════════════════════════════════════════════════════════════

pub async fn unlock_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = sqlx::query("UPDATE users SET is_locked = false, locked_until = NULL, updated_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.pool)
        .await;

    (StatusCode::OK, Json(json!({"message": "User unlocked", "id": user_id}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/users/:id/password — reset password
// ═══════════════════════════════════════════════════════════════════════════

pub async fn reset_password(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    let password_hash = match bcrypt::hash(&req.new_password, 12) {
        Ok(h) => h,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "hash_failed"}))).into_response();
        }
    };

    let _ = sqlx::query(
        "UPDATE users SET password_hash = $1, password_changed_at = NOW(), updated_at = NOW() WHERE id = $2"
    )
    .bind(&password_hash)
    .bind(user_id)
    .execute(&state.pool)
    .await;

    (StatusCode::OK, Json(json!({"message": "Password reset successfully"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/users/bulk — bulk actions
// ═══════════════════════════════════════════════════════════════════════════

pub async fn bulk_action(
    State(state): State<AppState>,
    Json(req): Json<BulkUserAction>,
) -> impl IntoResponse {
    let mut affected = 0i64;

    for uid in &req.user_ids {
        let result = match req.action.as_str() {
            "enable" => {
                sqlx::query("UPDATE users SET is_active = true, updated_at = NOW() WHERE id = $1")
                    .bind(uid).execute(&state.pool).await
            }
            "disable" => {
                sqlx::query("UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1")
                    .bind(uid).execute(&state.pool).await
            }
            "lock" => {
                sqlx::query("UPDATE users SET is_locked = true, updated_at = NOW() WHERE id = $1")
                    .bind(uid).execute(&state.pool).await
            }
            "unlock" => {
                sqlx::query("UPDATE users SET is_locked = false, locked_until = NULL, updated_at = NOW() WHERE id = $1")
                    .bind(uid).execute(&state.pool).await
            }
            "delete" => {
                sqlx::query("UPDATE users SET is_active = false, updated_at = NOW() WHERE id = $1")
                    .bind(uid).execute(&state.pool).await
            }
            _ => {
                return (StatusCode::BAD_REQUEST, Json(json!({
                    "error": "invalid_action",
                    "message": "Valid actions: enable, disable, lock, unlock, delete"
                }))).into_response();
            }
        };

        if let Ok(r) = result {
            affected += r.rows_affected() as i64;
        }
    }

    (StatusCode::OK, Json(json!({
        "message": format!("Bulk {} completed", req.action),
        "affected": affected
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/users/:id/modules — get a user's module access list
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_user_modules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(user_id): Path<Uuid>,
) -> impl IntoResponse {
    let modules: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT module_id FROM user_module_access WHERE user_id = $1 AND (org_id = $2 OR org_id IS NULL) ORDER BY module_id"
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    (StatusCode::OK, Json(json!({ "user_id": user_id, "modules": modules }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/users/:id/modules — replace a user's module access list
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct SetModulesRequest {
    pub modules: Vec<String>,
}

pub async fn set_user_modules(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(user_id): Path<Uuid>,
    Json(req): Json<SetModulesRequest>,
) -> impl IntoResponse {
    // Verify user exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND org_id = $2)")
        .bind(user_id)
        .bind(org_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);

    if !exists {
        return (StatusCode::NOT_FOUND, Json(json!({ "error": "user_not_found" }))).into_response();
    }

    // Delete existing module access for this user+org
    let _ = sqlx::query("DELETE FROM user_module_access WHERE user_id = $1 AND org_id = $2")
        .bind(user_id)
        .bind(org_id)
        .execute(&state.pool)
        .await;

    // Insert new module access entries
    let mut inserted = 0i64;
    for module_id in &req.modules {
        let r = sqlx::query(
            "INSERT INTO user_module_access (org_id, user_id, module_id) VALUES ($1, $2, $3) ON CONFLICT (org_id, user_id, module_id) DO NOTHING"
        )
        .bind(org_id)
        .bind(user_id)
        .bind(module_id)
        .execute(&state.pool)
        .await;
        if let Ok(r) = r { inserted += r.rows_affected() as i64; }
    }

    (StatusCode::OK, Json(json!({
        "message": "Module access updated",
        "user_id": user_id,
        "modules_set": inserted,
        "modules": req.modules,
    }))).into_response()
}
