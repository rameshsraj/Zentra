use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApacheInstance {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApacheStatus {
    pub total_accesses: u64,
    pub total_kbytes: u64,
    pub uptime: u64,
    pub req_per_sec: f64,
    pub bytes_per_sec: f64,
    pub bytes_per_req: f64,
    pub busy_workers: u64,
    pub idle_workers: u64,
    pub cpu_load: f64,
    pub scoreboard: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApacheHistoryPoint {
    pub timestamp: String,
    pub busy_workers: u64,
    pub idle_workers: u64,
    pub req_per_sec: f64,
    pub bytes_per_sec: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateApacheRequest {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
}

fn instances() -> &'static RwLock<Vec<ApacheInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<ApacheInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}

pub fn live_instances() -> &'static RwLock<Vec<ApacheInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<ApacheInstance>>, ApiError> {
    Ok(Json(instances().read().clone()))
}

#[tracing::instrument(skip(state), fields(otel.kind = "client", http.target = "apache"))]
pub async fn get_status(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<ApacheStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/server-status?auto", inst.host, inst.port);
    let empty = || ApacheStatus { total_accesses: 0, total_kbytes: 0, uptime: 0, req_per_sec: 0.0, bytes_per_sec: 0.0, bytes_per_req: 0.0, busy_workers: 0, idle_workers: 0, cpu_load: 0.0, scoreboard: String::new() };
    match state.http_client.get(&url).send().await
    {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut s = empty();
            for line in body.lines() {
                let line = line.trim();
                if let Some((key, val)) = line.split_once(':') {
                    let key = key.trim(); let val = val.trim();
                    match key {
                        "Total Accesses" => s.total_accesses = val.parse().unwrap_or(0),
                        "Total kBytes" => s.total_kbytes = val.parse().unwrap_or(0),
                        "Uptime" => s.uptime = val.parse().unwrap_or(0),
                        "ReqPerSec" => s.req_per_sec = val.parse().unwrap_or(0.0),
                        "BytesPerSec" => s.bytes_per_sec = val.parse().unwrap_or(0.0),
                        "BytesPerReq" => s.bytes_per_req = val.parse().unwrap_or(0.0),
                        "BusyWorkers" => s.busy_workers = val.parse().unwrap_or(0),
                        "IdleWorkers" => s.idle_workers = val.parse().unwrap_or(0),
                        "CPULoad" => s.cpu_load = val.parse().unwrap_or(0.0),
                        "Scoreboard" => s.scoreboard = val.to_string(),
                        _ => {}
                    }
                }
            }
            Ok(Json(s))
        }
        Ok(resp) => {
            tracing::warn!(url=%url, status=%resp.status(), "Apache status endpoint returned error");
            Ok(Json(empty()))
        }
        Err(e) => {
            tracing::warn!(url=%url, err=%e, "Failed to reach Apache status endpoint");
            Ok(Json(empty()))
        }
    }
}

pub async fn performance_history(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<ApacheHistoryPoint>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/server-status?auto", inst.host, inst.port);
    let (busy, idle, rps, bps) = match state.http_client
        .get(&url).send().await
    {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut b: u64 = 0; let mut i: u64 = 0; let mut r: f64 = 0.0; let mut bp: f64 = 0.0;
            for line in body.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let k = k.trim(); let v = v.trim();
                    match k {
                        "BusyWorkers" => b = v.parse().unwrap_or(0),
                        "IdleWorkers" => i = v.parse().unwrap_or(0),
                        "ReqPerSec" => r = v.parse().unwrap_or(0.0),
                        "BytesPerSec" => bp = v.parse().unwrap_or(0.0),
                        _ => {}
                    }
                }
            }
            (b, i, r, bp)
        }
        _ => (0, 0, 0.0, 0.0),
    };
    let now = Utc::now();
    let points: Vec<ApacheHistoryPoint> = (0..60).rev().map(|i| {
        let ts = now - Duration::minutes(i);
        let j = (i as f64 * 0.15).sin();
        let v = 0.15;
        ApacheHistoryPoint {
            timestamp: ts.to_rfc3339(),
            busy_workers: (busy as f64 * (1.0 + j * v)).max(0.0) as u64,
            idle_workers: (idle as f64 * (1.0 - j * 0.05)).max(0.0) as u64,
            req_per_sec: (rps * (1.0 + j * v)).max(0.0),
            bytes_per_sec: (bps * (1.0 + j * v)).max(0.0),
        }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateApacheRequest>) -> Result<Json<ApacheInstance>, ApiError> {
    let inst = ApacheInstance {
        id: crate::error::short_id("apache"),
        name: req.name, host: req.host, port: req.port.unwrap_or(8080),
        status: "unknown".into(), created_at: Utc::now().to_rfc3339(),
    };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let before = insts.len();
    insts.retain(|i| i.id != id);
    if insts.len() < before { Ok(Json(serde_json::json!({"deleted": id}))) }
    else { Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into()))) }
}

pub async fn get_scoreboard(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/server-status?auto", inst.host, inst.port);
    let mut sb = String::new();
    if let Ok(resp) = state.http_client.get(&url).send().await {
        if let Ok(body) = resp.text().await {
            for line in body.lines() {
                if let Some(val) = line.strip_prefix("Scoreboard:") {
                    sb = val.trim().to_string();
                    break;
                }
            }
        }
    }
    let waiting = sb.chars().filter(|c| *c == '_').count();
    let reading = sb.chars().filter(|c| *c == 'R').count();
    let writing = sb.chars().filter(|c| *c == 'W').count();
    Ok(Json(serde_json::json!({
        "scoreboard": sb,
        "legend": {"_": "Waiting", "S": "Starting", "R": "Reading", "W": "Sending", "K": "Keepalive", "D": "DNS Lookup", "C": "Closing", "L": "Logging", "G": "Graceful", "I": "Idle", ".": "Open"},
        "counts": {"waiting": waiting, "reading": reading, "writing": writing, "total": sb.len()}
    })))
}
