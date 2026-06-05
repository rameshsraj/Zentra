use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HAProxyInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize)]
pub struct HAProxyStatus {
    pub frontends: Vec<HAProxyFrontend>,
    pub backends: Vec<HAProxyBackend>,
    pub total_sessions: u64,
    pub session_rate: f64,
    pub connection_errors: u64,
    pub request_errors: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HAProxyFrontend { pub name: String, pub status: String, pub scur: u64, pub smax: u64, pub stot: u64, pub bin: u64, pub bout: u64, pub rate: f64, pub hrsp_2xx: u64, pub hrsp_4xx: u64, pub hrsp_5xx: u64 }

#[derive(Debug, Clone, Serialize)]
pub struct HAProxyBackend { pub name: String, pub status: String, pub servers: Vec<HAProxyServer>, pub scur: u64, pub queue: u64, pub eresp: u64 }

#[derive(Debug, Clone, Serialize)]
pub struct HAProxyServer { pub name: String, pub status: String, pub scur: u64, pub smax: u64, pub check_status: String, pub weight: u32 }

#[derive(Debug, Clone, Serialize)]
pub struct HAProxyHistoryPoint { pub timestamp: String, pub sessions: u64, pub session_rate: f64, pub hrsp_2xx: u64, pub hrsp_5xx: u64, pub queue_depth: u64 }

#[derive(Debug, Deserialize)]
pub struct CreateHAProxyRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<HAProxyInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<HAProxyInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<HAProxyInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<HAProxyInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<HAProxyStatus>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(HAProxyStatus { frontends: vec![], backends: vec![], total_sessions: 0, session_rate: 0.0, connection_errors: 0, request_errors: 0 }));
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<HAProxyHistoryPoint>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![]))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateHAProxyRequest>) -> Result<Json<HAProxyInstance>, ApiError> {
    let inst = HAProxyInstance { id: crate::error::short_id("haproxy"), name: req.name, host: req.host, port: req.port.unwrap_or(8404), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
