//! Alert dependency management — parent/child suppress relationships.
//! Backed by `alert_rule_dependencies` table.

use axum::{
    extract::{State, Path},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::state::AppState;
use crate::error::ApiError;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRecord {
    pub id: Uuid,
    pub parent_rule_id: Uuid,
    pub parent_rule_name: Option<String>,
    pub child_rule_id: Uuid,
    pub child_rule_name: Option<String>,
    pub suppress_duration_minutes: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDependencyRequest {
    pub parent_rule_id: Uuid,
    pub suppress_duration_minutes: Option<i64>,
}

pub async fn list_dependencies(
    State(state): State<AppState>,
    Path(rule_id): Path<Uuid>,
) -> Result<Json<Vec<DependencyRecord>>, ApiError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Option<String>, Uuid, Option<String>, i64, DateTime<Utc>)>(
        "SELECT d.id,
                d.parent_rule_id,
                p.name AS parent_name,
                d.child_rule_id,
                c.name AS child_name,
                d.suppress_duration_minutes,
                d.created_at
         FROM alert_rule_dependencies d
         LEFT JOIN alert_rules p ON p.rule_id = d.parent_rule_id
         LEFT JOIN alert_rules c ON c.rule_id = d.child_rule_id
         WHERE d.parent_rule_id = $1 OR d.child_rule_id = $1
         ORDER BY d.created_at DESC",
    )
    .bind(rule_id)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let deps = rows.into_iter().map(|(id, parent_rule_id, parent_rule_name, child_rule_id, child_rule_name, suppress_duration_minutes, created_at)| {
        DependencyRecord { id, parent_rule_id, parent_rule_name, child_rule_id, child_rule_name, suppress_duration_minutes, created_at }
    }).collect();
    Ok(Json(deps))
}

pub async fn create_dependency(
    State(state): State<AppState>,
    Path(child_rule_id): Path<Uuid>,
    Json(req): Json<CreateDependencyRequest>,
) -> Result<Json<DependencyRecord>, ApiError> {
    let id = Uuid::new_v4();
    let suppress = req.suppress_duration_minutes.unwrap_or(15);

    sqlx::query(
        "INSERT INTO alert_rule_dependencies (id, parent_rule_id, child_rule_id, suppress_duration_minutes)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(req.parent_rule_id)
    .bind(child_rule_id)
    .bind(suppress)
    .execute(&state.pool)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(DependencyRecord {
        id,
        parent_rule_id: req.parent_rule_id,
        parent_rule_name: None,
        child_rule_id,
        child_rule_name: None,
        suppress_duration_minutes: suppress,
        created_at: Utc::now(),
    }))
}

pub async fn delete_dependency(
    State(state): State<AppState>,
    Path(dep_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = sqlx::query("DELETE FROM alert_rule_dependencies WHERE id = $1")
        .bind(dep_id)
        .execute(&state.pool)
        .await
        .map_err(ApiError::from)?;

    if result.rows_affected() == 0 {
        return Err(ApiError::NotFound(format!("dependency {} not found", dep_id)));
    }
    Ok(Json(serde_json::json!({ "deleted": dep_id })))
}

pub async fn dependency_graph(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let rows = sqlx::query_as::<_, (Uuid, Option<String>, Uuid, Option<String>, i64)>(
        "SELECT d.parent_rule_id, p.name, d.child_rule_id, c.name, d.suppress_duration_minutes
         FROM alert_rule_dependencies d
         LEFT JOIN alert_rules p ON p.rule_id = d.parent_rule_id
         LEFT JOIN alert_rules c ON c.rule_id = d.child_rule_id
         ORDER BY d.created_at",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::from)?;

    let mut node_set: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    let mut edges: Vec<serde_json::Value> = Vec::new();

    for (parent_id, parent_name, child_id, child_name, suppress) in &rows {
        node_set.insert(*parent_id);
        node_set.insert(*child_id);
        edges.push(serde_json::json!({
            "from": parent_id,
            "from_name": parent_name,
            "to": child_id,
            "to_name": child_name,
            "suppress_minutes": suppress,
        }));
    }

    let nodes: Vec<serde_json::Value> = rows.iter().flat_map(|(pid, pname, cid, cname, _)| {
        vec![
            serde_json::json!({ "id": pid, "name": pname, "type": "parent" }),
            serde_json::json!({ "id": cid, "name": cname, "type": "child" }),
        ]
    }).collect();

    Ok(Json(serde_json::json!({ "nodes": nodes, "edges": edges })))
}
