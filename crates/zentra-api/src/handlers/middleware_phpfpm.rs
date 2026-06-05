use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpFpmInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize)]
pub struct PhpFpmStatus {
    pub pool: String, pub process_manager: String,
    pub accepted_conn: u64, pub listen_queue: u32, pub max_listen_queue: u32, pub listen_queue_len: u32,
    pub idle_processes: u32, pub active_processes: u32, pub total_processes: u32,
    pub max_active_processes: u32, pub max_children_reached: u32, pub slow_requests: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhpFpmHistoryPoint { pub timestamp: String, pub active_processes: u32, pub idle_processes: u32, pub listen_queue: u32, pub slow_requests: u64, pub accepted_conn: u64 }

#[derive(Debug, Deserialize)]
pub struct CreatePhpFpmRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<PhpFpmInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<PhpFpmInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<PhpFpmInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<PhpFpmInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<PhpFpmStatus>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(PhpFpmStatus {
        pool: String::new(), process_manager: String::new(),
        accepted_conn: 0, listen_queue: 0, max_listen_queue: 0, listen_queue_len: 0,
        idle_processes: 0, active_processes: 0, total_processes: 0,
        max_active_processes: 0, max_children_reached: 0, slow_requests: 0,
    }));
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<PhpFpmHistoryPoint>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![]))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreatePhpFpmRequest>) -> Result<Json<PhpFpmInstance>, ApiError> {
    let inst = PhpFpmInstance { id: crate::error::short_id("phpfpm"), name: req.name, host: req.host, port: req.port.unwrap_or(9000), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
