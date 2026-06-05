use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomcatInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize)]
pub struct TomcatStatus {
    pub max_threads: u32, pub current_thread_count: u32, pub current_threads_busy: u32,
    pub request_count: u64, pub error_count: u64, pub processing_time_ms: u64,
    pub max_processing_time_ms: u64, pub bytes_sent: u64, pub bytes_received: u64,
    pub active_sessions: u32, pub session_count: u64,
    pub jdbc_pools: Vec<JdbcPool>, pub jvm_memory: JvmMemory,
}

#[derive(Debug, Clone, Serialize)]
pub struct JdbcPool { pub name: String, pub active: u32, pub idle: u32, pub max: u32, pub wait_count: u64 }

#[derive(Debug, Clone, Serialize)]
pub struct JvmMemory { pub heap_used_mb: f64, pub heap_max_mb: f64, pub non_heap_used_mb: f64, pub gc_count: u64, pub gc_time_ms: u64 }

#[derive(Debug, Clone, Serialize)]
pub struct TomcatHistoryPoint { pub timestamp: String, pub threads_busy: u32, pub request_count: u64, pub error_count: u64, pub active_sessions: u32, pub heap_used_mb: f64 }

#[derive(Debug, Deserialize)]
pub struct CreateTomcatRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<TomcatInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<TomcatInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<TomcatInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<TomcatInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<TomcatStatus>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(TomcatStatus {
        max_threads: 0, current_thread_count: 0, current_threads_busy: 0,
        request_count: 0, error_count: 0, processing_time_ms: 0, max_processing_time_ms: 0,
        bytes_sent: 0, bytes_received: 0, active_sessions: 0, session_count: 0,
        jdbc_pools: vec![], jvm_memory: JvmMemory { heap_used_mb: 0.0, heap_max_mb: 0.0, non_heap_used_mb: 0.0, gc_count: 0, gc_time_ms: 0 },
    }));
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<TomcatHistoryPoint>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![]))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateTomcatRequest>) -> Result<Json<TomcatInstance>, ApiError> {
    let inst = TomcatInstance { id: crate::error::short_id("tomcat"), name: req.name, host: req.host, port: req.port.unwrap_or(8080), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
