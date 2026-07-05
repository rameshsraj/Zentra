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

pub async fn list_instances(State(s): State<AppState>) -> Result<Json<Vec<MemcachedInstance>>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, String)>(
        "SELECT id, name, host, port, status, created_at::TEXT FROM middleware_instances WHERE instance_type = 'memcached' ORDER BY created_at"
    ).fetch_all(&s.pool).await.unwrap_or_default();
    let from_db: Vec<MemcachedInstance> = rows.into_iter().map(|(id, name, host, port, status, created_at)|
        MemcachedInstance { id, name, host, port: port as u16, status, created_at }
    ).collect();
    if !from_db.is_empty() { *instances().write() = from_db.clone(); return Ok(Json(from_db)); }
    Ok(Json(instances().read().clone()))
}
pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<MemcachedStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = MemcachedStatus { curr_connections: 0, total_connections: 0, get_hits: 0, get_misses: 0, hit_rate: 0.0, bytes_read: 0, bytes_written: 0, curr_items: 0, total_items: 0, evictions: 0, cmd_get_per_sec: 0.0, cmd_set_per_sec: 0.0, bytes_used_mb: 0.0, limit_maxbytes_mb: 0.0, uptime: 0, version: String::new() };
    // Native Memcached stats command via TCP
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    let addr = format!("{}:{}", inst.host, inst.port);
    let mut stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => { tracing::warn!(addr=%addr, err=%e, "Failed to connect to Memcached"); return Ok(Json(empty)); }
    };
    let _ = stream.write_all(b"stats\r\n").await;
    let mut buf = vec![0u8; 16384];
    let n = stream.read(&mut buf).await.unwrap_or(0);
    let stats = String::from_utf8_lossy(&buf[..n]).to_string();
    let get = |key: &str| -> f64 {
        for line in stats.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == key { return parts[2].parse().unwrap_or(0.0); }
        }
        0.0
    };
    let get_str = |key: &str| -> String {
        for line in stats.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == key { return parts[2].to_string(); }
        }
        String::new()
    };
    let get_hits        = get("get_hits") as u64;
    let get_misses      = get("get_misses") as u64;
    let curr_items      = get("curr_items") as u64;
    let total_items     = get("total_items") as u64;
    let evictions       = get("evictions") as u64;
    let bytes_used      = get("bytes") / 1_048_576.0;
    let max_bytes       = get("limit_maxbytes") / 1_048_576.0;
    let curr_connections = get("curr_connections") as u32;
    let total_connections = get("total_connections") as u64;
    let bytes_read      = get("bytes_read") as u64;
    let bytes_written   = get("bytes_written") as u64;
    let uptime          = get("uptime") as u64;
    let version         = get_str("version");
    let total           = get_hits + get_misses;
    let hit_rate        = if total > 0 { get_hits as f64 / total as f64 * 100.0 } else { 0.0 };
    if !stats.is_empty() {
        let _ = sqlx::query(
            "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at)
             VALUES ($1,'memcached','curr_items',$2,now()),
                    ($1,'memcached','hit_rate',$3,now()),
                    ($1,'memcached','bytes_used_mb',$4,now()),
                    ($1,'memcached','evictions',$5,now())"
        ).bind(&id).bind(curr_items as f64).bind(hit_rate).bind(bytes_used).bind(evictions as f64).execute(&s.pool).await;
    }
    Ok(Json(MemcachedStatus { curr_connections, total_connections, get_hits, get_misses, hit_rate, bytes_read, bytes_written, curr_items, total_items, evictions, cmd_get_per_sec: 0.0, cmd_set_per_sec: 0.0, bytes_used_mb: bytes_used, limit_maxbytes_mb: max_bytes, uptime, version }))
}
pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<MemcachedHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'curr_items'    THEN value END) AS curr_items,
           avg(CASE WHEN metric_name = 'hit_rate'      THEN value END) AS hit_rate,
           avg(CASE WHEN metric_name = 'bytes_used_mb' THEN value END) AS bytes_used_mb,
           avg(CASE WHEN metric_name = 'evictions'     THEN value END) AS evictions
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'memcached'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<MemcachedHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        MemcachedHistoryPoint {
            timestamp:       bucket.to_rfc3339(),
            curr_items:      r.get::<Option<f64>, _>("curr_items").unwrap_or(0.0) as u64,
            hit_rate:        r.get::<Option<f64>, _>("hit_rate").unwrap_or(0.0),
            bytes_used_mb:   r.get::<Option<f64>, _>("bytes_used_mb").unwrap_or(0.0),
            evictions:       r.get::<Option<f64>, _>("evictions").unwrap_or(0.0) as u64,
            cmd_get_per_sec: 0.0,
        }
    }).collect();
    Ok(Json(points))
}
pub async fn create_instance(State(s): State<AppState>, Json(req): Json<CreateMemcachedRequest>) -> Result<Json<MemcachedInstance>, ApiError> {
    let inst = MemcachedInstance { id: crate::error::short_id("memcached"), name: req.name, host: req.host, port: req.port.unwrap_or(11211), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    let _ = sqlx::query(
        "INSERT INTO middleware_instances (id, instance_type, name, host, port) VALUES ($1, 'memcached', $2, $3, $4) ON CONFLICT (id) DO NOTHING"
    ).bind(&inst.id).bind(&inst.name).bind(&inst.host).bind(inst.port as i32).execute(&s.pool).await;
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = sqlx::query("DELETE FROM middleware_instances WHERE id = $1 AND instance_type = 'memcached'").bind(&id).execute(&s.pool).await;
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
