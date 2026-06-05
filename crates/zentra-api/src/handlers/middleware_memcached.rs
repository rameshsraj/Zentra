use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemcachedInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }
#[derive(Debug, Clone, Serialize)]
pub struct MemcachedStatus {
    pub curr_connections: u32, pub total_connections: u64, pub get_hits: u64, pub get_misses: u64, pub hit_rate: f64,
    pub bytes_read: u64, pub bytes_written: u64, pub curr_items: u64, pub total_items: u64, pub evictions: u64,
    pub cmd_get_per_sec: f64, pub cmd_set_per_sec: f64, pub bytes_used_mb: f64, pub limit_maxbytes_mb: f64,
    pub uptime: u64, pub version: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct MemcachedHistoryPoint { pub timestamp: String, pub hit_rate: f64, pub curr_items: u64, pub evictions: u64, pub cmd_get_per_sec: f64, pub bytes_used_mb: f64 }
#[derive(Debug, Deserialize)]
pub struct CreateMemcachedRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<MemcachedInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<MemcachedInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<MemcachedInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<MemcachedInstance>>, ApiError> { Ok(Json(instances().read().clone())) }
pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<MemcachedStatus>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(MemcachedStatus {
        curr_connections: 0, total_connections: 0, get_hits: 0, get_misses: 0, hit_rate: 0.0,
        bytes_read: 0, bytes_written: 0, curr_items: 0, total_items: 0, evictions: 0,
        cmd_get_per_sec: 0.0, cmd_set_per_sec: 0.0, bytes_used_mb: 0.0, limit_maxbytes_mb: 0.0,
        uptime: 0, version: String::new(),
    }));
}
pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<MemcachedHistoryPoint>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![]))
}
pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateMemcachedRequest>) -> Result<Json<MemcachedInstance>, ApiError> {
    let inst = MemcachedInstance { id: crate::error::short_id("memcached"), name: req.name, host: req.host, port: req.port.unwrap_or(11211), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
