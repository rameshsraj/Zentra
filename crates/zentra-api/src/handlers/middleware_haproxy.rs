use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<HAProxyStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = HAProxyStatus { frontends: vec![], backends: vec![], total_sessions: 0, session_rate: 0.0, connection_errors: 0, request_errors: 0 };
    // HAProxy stats page (CSV) exposed via HTTP
    let url = format!("http://{}:{}/stats;csv", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut total_sessions: u64 = 0;
            let mut frontends: Vec<HAProxyFrontend> = vec![];
            let mut backends: Vec<HAProxyBackend> = vec![];
            for line in body.lines() {
                if line.starts_with('#') || line.is_empty() { continue; }
                let cols: Vec<&str> = line.split(',').collect();
                if cols.len() < 18 { continue; }
                let pxname = cols[0]; let svname = cols[1];
                let scur = cols[4].parse::<u64>().unwrap_or(0);
                let smax = cols[5].parse::<u64>().unwrap_or(0);
                let stot = cols[7].parse::<u64>().unwrap_or(0);
                total_sessions += scur;
                if svname == "FRONTEND" {
                    frontends.push(HAProxyFrontend { name: pxname.to_string(), status: cols[17].to_string(), scur, smax, stot, bin: 0, bout: 0, rate: 0.0, hrsp_2xx: 0, hrsp_4xx: 0, hrsp_5xx: 0 });
                } else if svname == "BACKEND" {
                    backends.push(HAProxyBackend { name: pxname.to_string(), status: cols[17].to_string(), servers: vec![], scur, queue: 0, eresp: 0 });
                }
            }
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'haproxy','total_sessions',$2,now())"
            ).bind(&id).bind(total_sessions as f64).execute(&s.pool).await;
            Ok(Json(HAProxyStatus { frontends, backends, total_sessions, session_rate: 0.0, connection_errors: 0, request_errors: 0 }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "HAProxy stats error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach HAProxy"); Ok(Json(empty)) }
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<HAProxyHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'total_sessions' THEN value END) AS sessions
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'haproxy'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<HAProxyHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        HAProxyHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            sessions: r.get::<Option<f64>, _>("sessions").unwrap_or(0.0) as u64,
            session_rate: 0.0, hrsp_2xx: 0, hrsp_5xx: 0, queue_depth: 0,
        }
    }).collect();
    Ok(Json(points))
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
