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
// GET /api/user-groups — list with member count
// ═══════════════════════════════════════════════════════════════════════════

pub async fn list_groups(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let groups = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, i64, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT g.id, g.org_id, g.name, g.description, \
         COALESCE((SELECT COUNT(*) FROM user_group_members m WHERE m.group_id = g.id), 0) as member_count, \
         g.created_at, g.updated_at \
         FROM user_groups g WHERE g.org_id = $1 ORDER BY g.name"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let list: Vec<serde_json::Value> = groups.iter().map(|g| json!({
        "id": g.0,
        "org_id": g.1,
        "name": g.2,
        "description": g.3,
        "member_count": g.4,
        "created_at": g.5,
        "updated_at": g.6,
    })).collect();

    (StatusCode::OK, Json(json!(list))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/user-groups — create
// ═══════════════════════════════════════════════════════════════════════════

pub async fn create_group(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateUserGroupRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4();
    let result = sqlx::query(
        "INSERT INTO user_groups (id, org_id, name, description) VALUES ($1, $2, $3, $4)"
    )
    .bind(id)
    .bind(org_id)
    .bind(&req.name)
    .bind(&req.description)
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => (StatusCode::CREATED, Json(json!({
            "id": id, "name": req.name, "message": "Group created"
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": e.to_string()
        }))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GET /api/user-groups/:id — detail with members
// ═══════════════════════════════════════════════════════════════════════════

pub async fn get_group(
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> impl IntoResponse {
    let group = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, org_id, name, description, created_at, updated_at FROM user_groups WHERE id = $1"
    )
    .bind(group_id)
    .fetch_optional(&state.pool)
    .await;

    let group = match group {
        Ok(Some(g)) => g,
        _ => return (StatusCode::NOT_FOUND, Json(json!({"error": "group_not_found"}))).into_response(),
    };

    let members = sqlx::query_as::<_, (Uuid, String, String, Option<String>)>(
        "SELECT u.id, u.email, u.username, u.display_name \
         FROM users u JOIN user_group_members m ON m.user_id = u.id \
         WHERE m.group_id = $1 ORDER BY u.username"
    )
    .bind(group_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let member_list: Vec<serde_json::Value> = members.iter().map(|m| json!({
        "id": m.0, "email": m.1, "username": m.2, "display_name": m.3,
    })).collect();

    (StatusCode::OK, Json(json!({
        "id": group.0,
        "org_id": group.1,
        "name": group.2,
        "description": group.3,
        "created_at": group.4,
        "updated_at": group.5,
        "members": member_list,
        "member_count": member_list.len(),
    }))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// PUT /api/user-groups/:id — update
// ═══════════════════════════════════════════════════════════════════════════

pub async fn update_group(
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(req): Json<UpdateUserGroupRequest>,
) -> impl IntoResponse {
    let result = sqlx::query(
        "UPDATE user_groups SET name = COALESCE($1, name), description = COALESCE($2, description), updated_at = NOW() WHERE id = $3"
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(group_id)
    .execute(&state.pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"message": "Group updated"}))).into_response()
        }
        _ => (StatusCode::NOT_FOUND, Json(json!({"error": "group_not_found"}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/user-groups/:id — delete
// ═══════════════════════════════════════════════════════════════════════════

pub async fn delete_group(
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = sqlx::query("DELETE FROM user_group_members WHERE group_id = $1").bind(group_id).execute(&state.pool).await;
    let _ = sqlx::query("DELETE FROM user_groups WHERE id = $1").bind(group_id).execute(&state.pool).await;

    (StatusCode::OK, Json(json!({"message": "Group deleted"}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// POST /api/user-groups/:id/members — add members
// ═══════════════════════════════════════════════════════════════════════════

pub async fn add_members(
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(req): Json<GroupMembersRequest>,
) -> impl IntoResponse {
    let mut added = 0i64;
    for uid in &req.user_ids {
        let r = sqlx::query(
            "INSERT INTO user_group_members (group_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
        )
        .bind(group_id)
        .bind(uid)
        .execute(&state.pool)
        .await;
        if let Ok(r) = r { added += r.rows_affected() as i64; }
    }

    (StatusCode::OK, Json(json!({"message": "Members added", "added": added}))).into_response()
}

// ═══════════════════════════════════════════════════════════════════════════
// DELETE /api/user-groups/:id/members/:user_id — remove member
// ═══════════════════════════════════════════════════════════════════════════

pub async fn remove_member(
    State(state): State<AppState>,
    Path((group_id, user_id)): Path<(Uuid, Uuid)>,
) -> impl IntoResponse {
    let _ = sqlx::query("DELETE FROM user_group_members WHERE group_id = $1 AND user_id = $2")
        .bind(group_id)
        .bind(user_id)
        .execute(&state.pool)
        .await;

    (StatusCode::OK, Json(json!({"message": "Member removed"}))).into_response()
}
