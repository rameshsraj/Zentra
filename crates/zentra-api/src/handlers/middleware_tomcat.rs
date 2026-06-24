use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<TomcatStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = TomcatStatus { max_threads: 0, current_thread_count: 0, current_threads_busy: 0, request_count: 0, error_count: 0, processing_time_ms: 0, max_processing_time_ms: 0, bytes_sent: 0, bytes_received: 0, active_sessions: 0, session_count: 0, jdbc_pools: vec![], jvm_memory: JvmMemory { heap_used_mb: 0.0, heap_max_mb: 0.0, non_heap_used_mb: 0.0, gc_count: 0, gc_time_ms: 0 } };
    // Tomcat Manager status page (XML)
    let url = format!("http://{}:{}/manager/status?XML=true", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            // Extract key values from XML via simple string search
            fn extract_attr(xml: &str, tag: &str, attr: &str) -> f64 {
                xml.find(&format!("<{}", tag))
                    .and_then(|pos| xml[pos..].find(&format!("{}=\"", attr)))
                    .and_then(|apos| {
                        let start = xml.find(&format!("{}=\"", attr)).map(|p| p + attr.len() + 2)?;
                        let end = xml[start..].find('"')?;
                        xml[start..start+end].parse().ok()
                    }).unwrap_or(0.0)
            }
            let threads_busy = extract_attr(&body, "connector", "currentThreadsBusy") as u32;
            let request_count = extract_attr(&body, "requestInfo", "requestCount") as u64;
            let error_count   = extract_attr(&body, "requestInfo", "errorCount") as u64;
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'tomcat','threads_busy',$2,now())"
            ).bind(&id).bind(threads_busy as f64).execute(&s.pool).await;
            Ok(Json(TomcatStatus { max_threads: 200, current_thread_count: 0, current_threads_busy: threads_busy, request_count, error_count, processing_time_ms: 0, max_processing_time_ms: 0, bytes_sent: 0, bytes_received: 0, active_sessions: 0, session_count: 0, jdbc_pools: vec![], jvm_memory: JvmMemory { heap_used_mb: 0.0, heap_max_mb: 0.0, non_heap_used_mb: 0.0, gc_count: 0, gc_time_ms: 0 } }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "Tomcat status error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach Tomcat"); Ok(Json(empty)) }
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<TomcatHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'threads_busy' THEN value END) AS threads_busy
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'tomcat'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<TomcatHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        TomcatHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            threads_busy: r.get::<Option<f64>, _>("threads_busy").unwrap_or(0.0) as u32,
            request_count: 0, error_count: 0, active_sessions: 0, heap_used_mb: 0.0,
        }
    }).collect();
    Ok(Json(points))
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
