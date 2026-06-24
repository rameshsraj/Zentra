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

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<RedisInstance>>, ApiError> { Ok(Json(instances().read().clone())) }
pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<RedisStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = RedisStatus { connected_clients: 0, blocked_clients: 0, used_memory_mb: 0.0, used_memory_peak_mb: 0.0, total_commands_processed: 0, ops_per_sec: 0.0, keyspace_hits: 0, keyspace_misses: 0, hit_rate: 0.0, connected_slaves: 0, repl_offset: 0, rdb_last_save_time: String::new(), aof_enabled: false, db_keys: vec![], redis_version: String::new(), uptime_secs: 0 };
    // Prometheus redis_exporter exposes metrics at /metrics
    let url = format!("http://{}:{}/metrics", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut clients: u32 = 0; let mut used_mem: f64 = 0.0; let mut peak_mem: f64 = 0.0;
            let mut ops: f64 = 0.0; let mut hits: u64 = 0; let mut misses: u64 = 0;
            let mut uptime: u64 = 0; let mut version = String::new();
            for line in body.lines() {
                if line.starts_with('#') { continue; }
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts.len() < 2 { continue; }
                let val: f64 = parts[1].trim().parse().unwrap_or(0.0);
                let key = parts[0];
                if key.starts_with("redis_connected_clients") { clients = val as u32; }
                else if key.starts_with("redis_memory_used_bytes") { used_mem = val / 1_048_576.0; }
                else if key.starts_with("redis_memory_used_peak_bytes") { peak_mem = val / 1_048_576.0; }
                else if key.starts_with("redis_instantaneous_ops_per_sec") { ops = val; }
                else if key.starts_with("redis_keyspace_hits_total") { hits = val as u64; }
                else if key.starts_with("redis_keyspace_misses_total") { misses = val as u64; }
                else if key.starts_with("redis_uptime_in_seconds") { uptime = val as u64; }
                else if key.starts_with("redis_version") {
                    if let Some(v_start) = line.find("version=\"") {
                        let tail = &line[v_start + 9..];
                        if let Some(end) = tail.find('"') { version = tail[..end].to_string(); }
                    }
                }
            }
            let total = hits + misses;
            let hit_rate = if total > 0 { hits as f64 / total as f64 * 100.0 } else { 0.0 };
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'redis','ops_per_sec',$2,now())"
            ).bind(&id).bind(ops).execute(&s.pool).await;
            Ok(Json(RedisStatus { connected_clients: clients, blocked_clients: 0, used_memory_mb: used_mem, used_memory_peak_mb: peak_mem, total_commands_processed: 0, ops_per_sec: ops, keyspace_hits: hits, keyspace_misses: misses, hit_rate, connected_slaves: 0, repl_offset: 0, rdb_last_save_time: String::new(), aof_enabled: false, db_keys: vec![], redis_version: version, uptime_secs: uptime }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "Redis exporter error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach Redis exporter"); Ok(Json(empty)) }
    }
}
pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RedisHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'ops_per_sec' THEN value END) AS ops_per_sec
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
            ops_per_sec: r.get::<Option<f64>, _>("ops_per_sec").unwrap_or(0.0),
            used_memory_mb: 0.0, connected_clients: 0, hit_rate: 0.0,
        }
    }).collect();
    Ok(Json(points))
}
pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateRedisRequest>) -> Result<Json<RedisInstance>, ApiError> {
    let inst = RedisInstance { id: crate::error::short_id("redis"), name: req.name, host: req.host, port: req.port.unwrap_or(6379), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
