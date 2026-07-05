use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxInstance {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub stub_status_path: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NginxStatus {
    pub active_connections: u64,
    pub accepts: u64,
    pub handled: u64,
    pub requests: u64,
    pub reading: u64,
    pub writing: u64,
    pub waiting: u64,
    pub accepts_per_sec: f64,
    pub handled_per_sec: f64,
    pub requests_per_sec: f64,
    pub dropped_connections: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NginxHistoryPoint {
    pub timestamp: String,
    pub active_connections: u64,
    pub requests_per_sec: f64,
    pub reading: u64,
    pub writing: u64,
    pub waiting: u64,
}

#[derive(Debug, Deserialize)]
pub struct CreateNginxRequest {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub stub_status_path: Option<String>,
}

fn instances() -> &'static RwLock<Vec<NginxInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<NginxInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}

pub fn live_instances() -> &'static RwLock<Vec<NginxInstance>> {
    instances()
}

pub async fn list_instances(State(s): State<AppState>) -> Result<Json<Vec<NginxInstance>>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, String, String)>(
        "SELECT id, name, host, port, status, created_at::TEXT, coalesce(extra->>'stub_status_path', '/nginx_status') FROM middleware_instances WHERE instance_type = 'nginx' ORDER BY created_at"
    ).fetch_all(&s.pool).await.unwrap_or_default();
    let from_db: Vec<NginxInstance> = rows.into_iter().map(|(id, name, host, port, status, created_at, path)|
        NginxInstance { id, name, host, port: port as u16, stub_status_path: path, status, created_at }
    ).collect();
    if !from_db.is_empty() { *instances().write() = from_db.clone(); return Ok(Json(from_db)); }
    Ok(Json(instances().read().clone()))
}

#[tracing::instrument(skip(s), fields(otel.kind = "client", http.target = "nginx"))]
pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<NginxStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}{}", inst.host, inst.port, inst.stub_status_path);
    match s.http_client.get(&url).send().await
    {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut active: u64 = 0; let mut accepts: u64 = 0; let mut handled: u64 = 0;
            let mut requests: u64 = 0; let mut reading: u64 = 0; let mut writing: u64 = 0; let mut waiting: u64 = 0;
            for line in body.lines() {
                let line = line.trim();
                if line.starts_with("Active connections:") {
                    active = line.split(':').nth(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                } else if line.starts_with("Reading:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    reading = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
                    writing = parts.get(3).and_then(|v| v.parse().ok()).unwrap_or(0);
                    waiting = parts.get(5).and_then(|v| v.parse().ok()).unwrap_or(0);
                } else if let Ok(vals) = line.split_whitespace().map(|v| v.parse::<u64>()).collect::<Result<Vec<u64>,_>>() {
                    if vals.len() == 3 { accepts = vals[0]; handled = vals[1]; requests = vals[2]; }
                }
            }
            let dropped = accepts.saturating_sub(handled);
            let uptime_guess = if accepts > 0 { (accepts as f64 / 100.0).max(1.0) } else { 1.0 };
            let rps = (requests as f64 / uptime_guess).min(99999.0);
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at)
                 VALUES ($1,'nginx','active_connections',$2,now()),
                        ($1,'nginx','requests_per_sec',$3,now()),
                        ($1,'nginx','reading',$4,now()),
                        ($1,'nginx','writing',$5,now()),
                        ($1,'nginx','waiting',$6,now())"
            ).bind(&id).bind(active as f64).bind(rps).bind(reading as f64).bind(writing as f64).bind(waiting as f64)
             .execute(&s.pool).await;
            Ok(Json(NginxStatus {
                active_connections: active, accepts, handled, requests,
                reading, writing, waiting,
                accepts_per_sec: (accepts as f64 / uptime_guess).min(99999.0),
                handled_per_sec: (handled as f64 / uptime_guess).min(99999.0),
                requests_per_sec: rps,
                dropped_connections: dropped,
            }))
        }
        Ok(resp) => {
            tracing::warn!(url=%url, status=%resp.status(), "Nginx status endpoint returned error");
            Ok(Json(NginxStatus { active_connections: 0, accepts: 0, handled: 0, requests: 0, reading: 0, writing: 0, waiting: 0, accepts_per_sec: 0.0, handled_per_sec: 0.0, requests_per_sec: 0.0, dropped_connections: 0 }))
        }
        Err(e) => {
            tracing::warn!(url=%url, err=%e, "Failed to reach Nginx status endpoint");
            Ok(Json(NginxStatus { active_connections: 0, accepts: 0, handled: 0, requests: 0, reading: 0, writing: 0, waiting: 0, accepts_per_sec: 0.0, handled_per_sec: 0.0, requests_per_sec: 0.0, dropped_connections: 0 }))
        }
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<NginxHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'active_connections' THEN value END) AS active_connections,
           avg(CASE WHEN metric_name = 'requests_per_sec'  THEN value END) AS requests_per_sec,
           avg(CASE WHEN metric_name = 'reading'           THEN value END) AS reading,
           avg(CASE WHEN metric_name = 'writing'           THEN value END) AS writing,
           avg(CASE WHEN metric_name = 'waiting'           THEN value END) AS waiting
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'nginx'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<NginxHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        NginxHistoryPoint {
            timestamp:          bucket.to_rfc3339(),
            active_connections: r.get::<Option<f64>, _>("active_connections").unwrap_or(0.0) as u64,
            requests_per_sec:   r.get::<Option<f64>, _>("requests_per_sec").unwrap_or(0.0),
            reading:            r.get::<Option<f64>, _>("reading").unwrap_or(0.0) as u64,
            writing:            r.get::<Option<f64>, _>("writing").unwrap_or(0.0) as u64,
            waiting:            r.get::<Option<f64>, _>("waiting").unwrap_or(0.0) as u64,
        }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(s): State<AppState>, Json(req): Json<CreateNginxRequest>) -> Result<Json<NginxInstance>, ApiError> {
    let path = req.stub_status_path.unwrap_or("/nginx_status".into());
    let inst = NginxInstance {
        id: crate::error::short_id("nginx"),
        name: req.name, host: req.host, port: req.port.unwrap_or(80),
        stub_status_path: path.clone(),
        status: "unknown".into(), created_at: Utc::now().to_rfc3339(),
    };
    let extra = serde_json::json!({"stub_status_path": path});
    let _ = sqlx::query(
        "INSERT INTO middleware_instances (id, instance_type, name, host, port, extra) VALUES ($1, 'nginx', $2, $3, $4, $5) ON CONFLICT (id) DO NOTHING"
    ).bind(&inst.id).bind(&inst.name).bind(&inst.host).bind(inst.port as i32).bind(extra).execute(&s.pool).await;
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = sqlx::query("DELETE FROM middleware_instances WHERE id = $1 AND instance_type = 'nginx'").bind(&id).execute(&s.pool).await;
    let mut insts = instances().write();
    let before = insts.len();
    insts.retain(|i| i.id != id);
    if insts.len() < before { Ok(Json(serde_json::json!({"deleted": id}))) }
    else { Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into()))) }
}

pub async fn get_config(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let instances = instances().read();
    let inst = instances.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Instance not found".into()))?;
    Ok(Json(serde_json::json!({
        "instance": inst.name,
        "note": "Config retrieval requires SSH access to the target host. Configure SSH credentials in the target settings to enable remote config fetch.",
        "config": null,
        "upstreams": [],
        "server_blocks": []
    })))
}
