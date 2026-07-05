use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn list_instances(State(s): State<AppState>) -> Result<Json<Vec<RedisInstance>>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, String)>(
        "SELECT id, name, host, port, status, created_at::TEXT FROM middleware_instances WHERE instance_type = 'redis' ORDER BY created_at"
    ).fetch_all(&s.pool).await.unwrap_or_default();
    let from_db: Vec<RedisInstance> = rows.into_iter().map(|(id, name, host, port, status, created_at)|
        RedisInstance { id, name, host, port: port as u16, status, created_at }
    ).collect();
    if !from_db.is_empty() { *instances().write() = from_db.clone(); return Ok(Json(from_db)); }
    Ok(Json(instances().read().clone()))
}
pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<RedisStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = RedisStatus { connected_clients: 0, blocked_clients: 0, used_memory_mb: 0.0, used_memory_peak_mb: 0.0, total_commands_processed: 0, ops_per_sec: 0.0, keyspace_hits: 0, keyspace_misses: 0, hit_rate: 0.0, connected_slaves: 0, repl_offset: 0, rdb_last_save_time: String::new(), aof_enabled: false, db_keys: vec![], redis_version: String::new(), uptime_secs: 0 };
    // Native Redis INFO command via TCP
    use tokio::io::{AsyncWriteExt, AsyncReadExt};
    let addr = format!("{}:{}", inst.host, inst.port);
    let mut stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => { tracing::warn!(addr=%addr, err=%e, "Failed to connect to Redis"); return Ok(Json(empty)); }
    };
    let _ = stream.write_all(b"INFO\r\n").await;
    let mut buf = vec![0u8; 16384];
    let n = stream.read(&mut buf).await.unwrap_or(0);
    let info = String::from_utf8_lossy(&buf[..n]).to_string();
    let get = |key: &str| -> f64 {
        for line in info.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{}:", key)) {
                return rest.trim().parse().unwrap_or(0.0);
            }
        }
        0.0
    };
    let get_str = |key: &str| -> String {
        for line in info.lines() {
            if let Some(rest) = line.strip_prefix(&format!("{}:", key)) {
                return rest.trim().to_string();
            }
        }
        String::new()
    };
    let clients    = get("connected_clients") as u32;
    let used_mem   = get("used_memory") / 1_048_576.0;
    let peak_mem   = get("used_memory_peak") / 1_048_576.0;
    let ops        = get("instantaneous_ops_per_sec");
    let hits       = get("keyspace_hits") as u64;
    let misses     = get("keyspace_misses") as u64;
    let uptime     = get("uptime_in_seconds") as u64;
    let slaves     = get("connected_slaves") as u32;
    let aof        = get_str("aof_enabled") == "1";
    let version    = get_str("redis_version");
    let total      = hits + misses;
    let hit_rate   = if total > 0 { hits as f64 / total as f64 * 100.0 } else { 0.0 };
    if !info.is_empty() {
        let _ = sqlx::query(
            "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) 
             VALUES ($1,'redis','ops_per_sec',$2,now()),
                    ($1,'redis','used_memory_mb',$3,now()),
                    ($1,'redis','connected_clients',$4,now()),
                    ($1,'redis','hit_rate',$5,now())"
        ).bind(&id).bind(ops).bind(used_mem).bind(clients as f64).bind(hit_rate).execute(&s.pool).await;
    }
    Ok(Json(RedisStatus { connected_clients: clients, blocked_clients: 0, used_memory_mb: used_mem, used_memory_peak_mb: peak_mem, total_commands_processed: 0, ops_per_sec: ops, keyspace_hits: hits, keyspace_misses: misses, hit_rate, connected_slaves: slaves, repl_offset: 0, rdb_last_save_time: String::new(), aof_enabled: aof, db_keys: vec![], redis_version: version, uptime_secs: uptime }))
}
pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RedisHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'ops_per_sec'        THEN value END) AS ops_per_sec,
           avg(CASE WHEN metric_name = 'used_memory_mb'     THEN value END) AS used_memory_mb,
           avg(CASE WHEN metric_name = 'connected_clients'  THEN value END) AS connected_clients,
           avg(CASE WHEN metric_name = 'hit_rate'           THEN value END) AS hit_rate
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'redis'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<RedisHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        RedisHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            ops_per_sec:       r.get::<Option<f64>, _>("ops_per_sec").unwrap_or(0.0),
            used_memory_mb:    r.get::<Option<f64>, _>("used_memory_mb").unwrap_or(0.0),
            connected_clients: r.get::<Option<f64>, _>("connected_clients").unwrap_or(0.0) as u32,
            hit_rate:          r.get::<Option<f64>, _>("hit_rate").unwrap_or(0.0),
        }
    }).collect();
    Ok(Json(points))
}
pub async fn create_instance(State(s): State<AppState>, Json(req): Json<CreateRedisRequest>) -> Result<Json<RedisInstance>, ApiError> {
    let inst = RedisInstance { id: crate::error::short_id("redis"), name: req.name, host: req.host, port: req.port.unwrap_or(6379), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    let _ = sqlx::query(
        "INSERT INTO middleware_instances (id, instance_type, name, host, port) VALUES ($1, 'redis', $2, $3, $4) ON CONFLICT (id) DO NOTHING"
    ).bind(&inst.id).bind(&inst.name).bind(&inst.host).bind(inst.port as i32).execute(&s.pool).await;
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = sqlx::query("DELETE FROM middleware_instances WHERE id = $1 AND instance_type = 'redis'").bind(&id).execute(&s.pool).await;
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
