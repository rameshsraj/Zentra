use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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
pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<MemcachedStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = MemcachedStatus { curr_connections: 0, total_connections: 0, get_hits: 0, get_misses: 0, hit_rate: 0.0, bytes_read: 0, bytes_written: 0, curr_items: 0, total_items: 0, evictions: 0, cmd_get_per_sec: 0.0, cmd_set_per_sec: 0.0, bytes_used_mb: 0.0, limit_maxbytes_mb: 0.0, uptime: 0, version: String::new() };
    // Prometheus memcached exporter endpoint
    let url = format!("http://{}:{}/metrics", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut get_hits: u64 = 0; let mut get_misses: u64 = 0;
            let mut curr_items: u64 = 0; let mut evictions: u64 = 0;
            let mut bytes_used: f64 = 0.0; let mut max_bytes: f64 = 0.0;
            let mut curr_connections: u32 = 0; let mut uptime: u64 = 0;
            let mut version = String::new();
            for line in body.lines() {
                if line.starts_with('#') { continue; }
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts.len() < 2 { continue; }
                let val: f64 = parts[1].trim().parse().unwrap_or(0.0);
                let key = parts[0];
                if key.starts_with("memcached_get_hits") { get_hits = val as u64; }
                else if key.starts_with("memcached_get_misses") { get_misses = val as u64; }
                else if key.starts_with("memcached_current_items") { curr_items = val as u64; }
                else if key.starts_with("memcached_items_evicted") { evictions = val as u64; }
                else if key.starts_with("memcached_current_bytes") { bytes_used = val / 1_048_576.0; }
                else if key.starts_with("memcached_limit_bytes") { max_bytes = val / 1_048_576.0; }
                else if key.starts_with("memcached_current_connections") { curr_connections = val as u32; }
                else if key.starts_with("memcached_uptime_seconds") { uptime = val as u64; }
                else if key.starts_with("memcached_version") {
                    // version is a label not a value, skip
                    if let Some(v_start) = line.find("version=\"") {
                        let tail = &line[v_start + 9..];
                        if let Some(end) = tail.find('"') { version = tail[..end].to_string(); }
                    }
                }
            }
            let total = get_hits + get_misses;
            let hit_rate = if total > 0 { get_hits as f64 / total as f64 * 100.0 } else { 0.0 };
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'memcached','curr_items',$2,now())"
            ).bind(&id).bind(curr_items as f64).execute(&s.pool).await;
            Ok(Json(MemcachedStatus { curr_connections, total_connections: 0, get_hits, get_misses, hit_rate, bytes_read: 0, bytes_written: 0, curr_items, total_items: 0, evictions, cmd_get_per_sec: 0.0, cmd_set_per_sec: 0.0, bytes_used_mb: bytes_used, limit_maxbytes_mb: max_bytes, uptime, version }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "Memcached metrics error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach Memcached exporter"); Ok(Json(empty)) }
    }
}
pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<MemcachedHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'curr_items' THEN value END) AS curr_items
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'memcached'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<MemcachedHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        MemcachedHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            hit_rate: 0.0,
            curr_items: r.get::<Option<f64>, _>("curr_items").unwrap_or(0.0) as u64,
            evictions: 0, cmd_get_per_sec: 0.0, bytes_used_mb: 0.0,
        }
    }).collect();
    Ok(Json(points))
}
pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateMemcachedRequest>) -> Result<Json<MemcachedInstance>, ApiError> {
    let inst = MemcachedInstance { id: crate::error::short_id("memcached"), name: req.name, host: req.host, port: req.port.unwrap_or(11211), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
