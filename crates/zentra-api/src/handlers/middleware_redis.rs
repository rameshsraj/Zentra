use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }
#[derive(Debug, Clone, Serialize)]
pub struct RedisStatus {
    pub connected_clients: u32, pub blocked_clients: u32, pub used_memory_mb: f64, pub used_memory_peak_mb: f64,
    pub total_commands_processed: u64, pub ops_per_sec: f64, pub keyspace_hits: u64, pub keyspace_misses: u64, pub hit_rate: f64,
    pub connected_slaves: u32, pub repl_offset: u64, pub rdb_last_save_time: String, pub aof_enabled: bool,
    pub db_keys: Vec<RedisDB>, pub redis_version: String, pub uptime_secs: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct RedisDB { pub db: String, pub keys: u64, pub expires: u64, pub avg_ttl: u64 }
#[derive(Debug, Clone, Serialize)]
pub struct RedisHistoryPoint { pub timestamp: String, pub ops_per_sec: f64, pub used_memory_mb: f64, pub connected_clients: u32, pub hit_rate: f64 }
#[derive(Debug, Deserialize)]
pub struct CreateRedisRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<RedisInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<RedisInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<RedisInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<RedisInstance>>, ApiError> { Ok(Json(instances().read().clone())) }
pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<RedisStatus>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(RedisStatus {
        connected_clients: 0, blocked_clients: 0, used_memory_mb: 0.0, used_memory_peak_mb: 0.0,
        total_commands_processed: 0, ops_per_sec: 0.0, keyspace_hits: 0, keyspace_misses: 0, hit_rate: 0.0,
        connected_slaves: 0, repl_offset: 0, rdb_last_save_time: String::new(), aof_enabled: false,
        db_keys: vec![], redis_version: String::new(), uptime_secs: 0,
    }));
}
pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RedisHistoryPoint>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![]))
}
pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateRedisRequest>) -> Result<Json<RedisInstance>, ApiError> {
    let inst = RedisInstance { id: crate::error::short_id("redis"), name: req.name, host: req.host, port: req.port.unwrap_or(6379), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
