use axum::{
    extract::{State, Path},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use sqlx;
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::models::auth::*;

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/roles — list with permission counts
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_roles(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT r.role_id, r.org_id, r.name, r.description, r.is_system, r.created_at FROM roles r WHERE r.org_id = $1 ORDER BY r.is_system DESC, r.name"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut roles = Vec::new();
    for r in &rows {
        let perm_count: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM role_permissions WHERE role_id = $1"
        ).bind(r.0).fetch_one(&state.pool).await.unwrap_or(0);

        roles.push(json!({
            "role_id": r.0, "org_id": r.1, "name": r.2, "description": r.3,
            "is_system": r.4, "created_at": r.5, "permission_count": perm_count,
        }));
    }

    (StatusCode::OK, Json(json!(roles))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/roles — create custom role
// ═══════════════════════════════════════════════════════════════════════════

pub async fn create_role(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateRoleRequest>,
) -> impl IntoResponse {
    let role_id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO roles (role_id, org_id, name, description, is_system) VALUES ($1, $2, $3, $4, false)"
    )
    .bind(role_id).bind(org_id).bind(&req.name).bind(&req.description)
    .execute(&state.pool).await;

    if let Err(e) = result {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response();
    }

    for pid in &req.permission_ids {
        let _ = sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(role_id).bind(pid).execute(&state.pool).await;
    }

    (StatusCode::CREATED, Json(json!({"role_id": role_id, "name": req.name, "message": "Role created"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/roles/:id — detail with permission matrix
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
) -> impl IntoResponse {
    let role = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, bool, chrono::DateTime<chrono::Utc>)>(
        "SELECT role_id, org_id, name, description, is_system, created_at FROM roles WHERE role_id = $1"
    ).bind(role_id).fetch_optional(&state.pool).await;

    let role = match role {
        Ok(Some(r)) => r,
        _ => return (StatusCode::NOT_FOUND, Json(json!({"error": "role_not_found"}))).into_response(),
    };

    let perms = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        "SELECT p.permission_id, p.resource, p.action, p.description \
         FROM permissions p JOIN role_permissions rp ON rp.permission_id = p.permission_id \
         WHERE rp.role_id = $1 ORDER BY p.resource, p.action"
    ).bind(role_id).fetch_all(&state.pool).await.unwrap_or_default();

    let perm_list: Vec<serde_json::Value> = perms.iter().map(|p| json!({
        "permission_id": p.0, "resource": p.1, "action": p.2, "description": p.3,
    })).collect();

    (StatusCode::OK, Json(json!({
        "role_id": role.0, "org_id": role.1, "name": role.2, "description": role.3,
        "is_system": role.4, "created_at": role.5, "permissions": perm_list,
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/roles/:id — update
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> impl IntoResponse {
    // Check not system role for name changes
    let is_system: bool = sqlx::query_scalar::<_, bool>("SELECT is_system FROM roles WHERE role_id = $1")
        .bind(role_id).fetch_optional(&state.pool).await.unwrap_or(None).unwrap_or(false);

    if is_system && req.name.is_some() {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Cannot rename system roles"}))).into_response();
    }

    let _ = sqlx::query("UPDATE roles SET name = COALESCE($1, name), description = COALESCE($2, description) WHERE role_id = $3")
        .bind(&req.name).bind(&req.description).bind(role_id).execute(&state.pool).await;

    if let Some(perm_ids) = &req.permission_ids {
        let _ = sqlx::query("DELETE FROM role_permissions WHERE role_id = $1").bind(role_id).execute(&state.pool).await;
        for pid in perm_ids {
            let _ = sqlx::query("INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(role_id).bind(pid).execute(&state.pool).await;
        }
    }

    (StatusCode::OK, Json(json!({"message": "Role updated"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/roles/:id — delete custom only
// ═══════════════════════════════════════════════════════════════════════════

pub async fn delete_role(
    State(state): State<AppState>,
    Path(role_id): Path<Uuid>,
) -> impl IntoResponse {
    let is_system: bool = sqlx::query_scalar::<_, bool>("SELECT is_system FROM roles WHERE role_id = $1")
        .bind(role_id).fetch_optional(&state.pool).await.unwrap_or(None).unwrap_or(true);

    if is_system {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "Cannot delete system roles"}))).into_response();
    }

    let _ = sqlx::query("DELETE FROM role_permissions WHERE role_id = $1").bind(role_id).execute(&state.pool).await;
    let _ = sqlx::query("DELETE FROM user_roles WHERE role_id = $1").bind(role_id).execute(&state.pool).await;
    let _ = sqlx::query("DELETE FROM roles WHERE role_id = $1").bind(role_id).execute(&state.pool).await;

    (StatusCode::OK, Json(json!({"message": "Role deleted"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/permissions — all permissions grouped by resource
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_permissions(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let perms = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        "SELECT permission_id, resource, action, description FROM permissions ORDER BY resource, action"
    ).fetch_all(&state.pool).await.unwrap_or_default();

    // Group by resource
    let mut grouped: std::collections::BTreeMap<String, Vec<serde_json::Value>> = std::collections::BTreeMap::new();
    for p in &perms {
        grouped.entry(p.1.clone()).or_default().push(json!({
            "permission_id": p.0, "action": p.2, "description": p.3,
        }));
    }

    (StatusCode::OK, Json(json!(grouped))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/users/:id/roles — assign roles to user
// ═══════════════════════════════════════════════════════════════════════════

pub async fn assign_user_roles(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(user_id): Path<Uuid>,
    Json(req): Json<AssignRolesRequest>,
) -> impl IntoResponse {
    let _ = sqlx::query("DELETE FROM user_roles WHERE user_id = $1 AND org_id = $2")
        .bind(user_id).bind(org_id).execute(&state.pool).await;

    for role_id in &req.role_ids {
        let _ = sqlx::query("INSERT INTO user_roles (user_id, role_id, org_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(user_id).bind(role_id).bind(org_id).execute(&state.pool).await;
    }

    (StatusCode::OK, Json(json!({"message": "Roles assigned"}))).into_response()
}
