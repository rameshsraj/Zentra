use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<PhpFpmStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = PhpFpmStatus { pool: String::new(), process_manager: String::new(), accepted_conn: 0, listen_queue: 0, max_listen_queue: 0, listen_queue_len: 0, idle_processes: 0, active_processes: 0, total_processes: 0, max_active_processes: 0, max_children_reached: 0, slow_requests: 0 };
    // PHP-FPM status page (plain text or JSON, depends on config)
    let url = format!("http://{}:{}/status?json", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let active   = v.get("active processes").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let idle     = v.get("idle processes").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let total    = v.get("total processes").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let accepted = v.get("accepted conn").and_then(|x| x.as_u64()).unwrap_or(0);
            let queue    = v.get("listen queue").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let slow_req = v.get("slow requests").and_then(|x| x.as_u64()).unwrap_or(0);
            let pool     = v.get("pool").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let pm       = v.get("process manager").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'phpfpm','active_processes',$2,now())"
            ).bind(&id).bind(active as f64).execute(&s.pool).await;
            Ok(Json(PhpFpmStatus { pool, process_manager: pm, accepted_conn: accepted, listen_queue: queue, max_listen_queue: 0, listen_queue_len: 0, idle_processes: idle, active_processes: active, total_processes: total, max_active_processes: 0, max_children_reached: 0, slow_requests: slow_req }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "PHP-FPM status error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach PHP-FPM"); Ok(Json(empty)) }
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<PhpFpmHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'active_processes' THEN value END) AS active_processes
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'phpfpm'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<PhpFpmHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        PhpFpmHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            active_processes: r.get::<Option<f64>, _>("active_processes").unwrap_or(0.0) as u32,
            idle_processes: 0, listen_queue: 0, slow_requests: 0, accepted_conn: 0,
        }
    }).collect();
    Ok(Json(points))
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
