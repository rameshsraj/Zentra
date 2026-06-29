use axum::{
    extract::{State, Path, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostDependency {
    pub id: String,
    pub parent_target_id: String,
    pub child_target_id: String,
    pub dependency_type: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyTreeNode {
    pub target_id: String,
    pub target_name: String,
    pub status: String,
    pub children: Vec<DependencyTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlappingCheck {
    pub check_id: String,
    pub check_name: String,
    pub target_host: String,
    pub transition_count: usize,
    pub window_minutes: u64,
    pub current_status: String,
    pub acknowledged: bool,
    pub flapping_since: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDependencyRequest {
    pub parent_target_id: String,
    pub child_target_id: String,
    pub dependency_type: Option<String>,
    pub description: Option<String>,
}

struct DepStore {
    dependencies: Vec<HostDependency>,
    flapping: Vec<FlappingCheck>,
}

fn store() -> &'static RwLock<DepStore> {
    static INSTANCE: OnceLock<RwLock<DepStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> DepStore {
    DepStore { dependencies: vec![], flapping: vec![] }
}

// ── Dependency API ───────────────────────────────────────────────────

pub async fn list_dependencies(
    State(_s): State<AppState>,
) -> Result<Json<Vec<HostDependency>>, ApiError> {
    Ok(Json(store().read().dependencies.clone()))
}

pub async fn create_dependency(
    State(_s): State<AppState>,
    Json(req): Json<CreateDependencyRequest>,
) -> Result<Json<HostDependency>, ApiError> {
    let dep = HostDependency {
        id: crate::error::short_id("dep"),
        parent_target_id: req.parent_target_id,
        child_target_id: req.child_target_id,
        dependency_type: req.dependency_type.unwrap_or("network".into()),
        description: req.description.unwrap_or_default(),
        created_at: Utc::now().to_rfc3339(),
    };
    store().write().dependencies.push(dep.clone());
    Ok(Json(dep))
}

pub async fn delete_dependency(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.dependencies.len();
    store.dependencies.retain(|d| d.id != id);
    if store.dependencies.len() < before {
        Ok(Json(serde_json::json!({"deleted": id})))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Dependency not found".into())))
    }
}

pub async fn dependency_tree(
    State(_s): State<AppState>,
) -> Result<Json<Vec<DependencyTreeNode>>, ApiError> {
    let store = store().read();
    let deps = &store.dependencies;

    // Find root nodes (parents that are not children of anything)
    let children_set: std::collections::HashSet<&str> = deps.iter().map(|d| d.child_target_id.as_str()).collect();
    let roots: Vec<&str> = deps.iter()
        .map(|d| d.parent_target_id.as_str())
        .filter(|p| !children_set.contains(p))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    fn build_tree(target_id: &str, deps: &[HostDependency], depth: usize) -> DependencyTreeNode {
        let children_ids: Vec<&str> = deps.iter()
            .filter(|d| d.parent_target_id == target_id)
            .map(|d| d.child_target_id.as_str())
            .collect();
        let children = if depth < 5 {
            children_ids.iter().map(|c| build_tree(c, deps, depth + 1)).collect()
        } else { vec![] };

        DependencyTreeNode {
            target_id: target_id.into(),
            target_name: target_id.into(),
            status: "ok".into(),
            children,
        }
    }

    let tree: Vec<DependencyTreeNode> = roots.into_iter()
        .map(|r| build_tree(r, deps, 0))
        .collect();
    Ok(Json(tree))
}

// ── Flapping API ─────────────────────────────────────────────────────

pub async fn list_flapping(
    State(_s): State<AppState>,
) -> Result<Json<Vec<FlappingCheck>>, ApiError> {
    Ok(Json(store().read().flapping.clone()))
}

pub async fn acknowledge_flapping(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    if let Some(f) = store.flapping.iter_mut().find(|f| f.check_id == id) {
        f.acknowledged = true;
        Ok(Json(serde_json::json!({"acknowledged": id})))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Flapping check not found".into())))
    }
}
