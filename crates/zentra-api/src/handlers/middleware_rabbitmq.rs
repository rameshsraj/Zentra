use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RabbitMQInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize)]
pub struct RabbitMQStatus {
    pub queues_total: u32, pub messages_ready: u64, pub messages_unacked: u64,
    pub publish_rate: f64, pub deliver_rate: f64, pub connections: u32, pub channels: u32, pub consumers: u32,
    pub node_memory_mb: f64, pub node_disk_free_gb: f64, pub node_fd_used: u32, pub node_fd_total: u32,
    pub erlang_version: String, pub rabbitmq_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RabbitMQQueue { pub name: String, pub vhost: String, pub messages: u64, pub messages_ready: u64, pub messages_unacked: u64, pub consumers: u32, pub publish_rate: f64, pub deliver_rate: f64, pub memory_mb: f64, pub state: String }

#[derive(Debug, Clone, Serialize)]
pub struct RabbitMQHistoryPoint { pub timestamp: String, pub messages_ready: u64, pub publish_rate: f64, pub deliver_rate: f64, pub connections: u32 }

#[derive(Debug, Deserialize)]
pub struct CreateRabbitMQRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<RabbitMQInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<RabbitMQInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<RabbitMQInstance>> { instances() }

pub async fn list_instances(State(s): State<AppState>) -> Result<Json<Vec<RabbitMQInstance>>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, String)>(
        "SELECT id, name, host, port, status, created_at::TEXT FROM middleware_instances WHERE instance_type = 'rabbitmq' ORDER BY created_at"
    ).fetch_all(&s.pool).await.unwrap_or_default();
    let from_db: Vec<RabbitMQInstance> = rows.into_iter().map(|(id, name, host, port, status, created_at)|
        RabbitMQInstance { id, name, host, port: port as u16, status, created_at }
    ).collect();
    if !from_db.is_empty() { *instances().write() = from_db.clone(); return Ok(Json(from_db)); }
    Ok(Json(instances().read().clone()))
}

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<RabbitMQStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = RabbitMQStatus { queues_total: 0, messages_ready: 0, messages_unacked: 0, publish_rate: 0.0, deliver_rate: 0.0, connections: 0, channels: 0, consumers: 0, node_memory_mb: 0.0, node_disk_free_gb: 0.0, node_fd_used: 0, node_fd_total: 0, erlang_version: String::new(), rabbitmq_version: String::new() };
    let url = format!("http://{}:{}/api/overview", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let queue_totals = v.get("queue_totals").cloned().unwrap_or_default();
            let msg_stats    = v.get("message_stats").cloned().unwrap_or_default();
            let object_totals = v.get("object_totals").cloned().unwrap_or_default();
            let messages_ready  = queue_totals.get("messages_ready").and_then(|x| x.as_u64()).unwrap_or(0);
            let messages_unacked = queue_totals.get("messages_unacknowledged").and_then(|x| x.as_u64()).unwrap_or(0);
            let publish_rate = msg_stats.get("publish_details").and_then(|d| d.get("rate")).and_then(|x| x.as_f64()).unwrap_or(0.0);
            let deliver_rate = msg_stats.get("deliver_get_details").and_then(|d| d.get("rate")).and_then(|x| x.as_f64()).unwrap_or(0.0);
            let connections = object_totals.get("connections").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let channels    = object_totals.get("channels").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let consumers   = object_totals.get("consumers").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let version     = v.get("rabbitmq_version").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let erlang      = v.get("erlang_version").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'rabbitmq','messages_ready',$2,now())"
            ).bind(&id).bind(messages_ready as f64).execute(&s.pool).await;
            Ok(Json(RabbitMQStatus { queues_total: 0, messages_ready, messages_unacked, publish_rate, deliver_rate, connections, channels, consumers, node_memory_mb: 0.0, node_disk_free_gb: 0.0, node_fd_used: 0, node_fd_total: 0, erlang_version: erlang, rabbitmq_version: version }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "RabbitMQ overview error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach RabbitMQ"); Ok(Json(empty)) }
    }
}

pub async fn get_queues(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RabbitMQQueue>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/api/queues", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
            let queues = v.iter().map(|q| RabbitMQQueue {
                name:             q.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                vhost:            q.get("vhost").and_then(|x| x.as_str()).unwrap_or("/").to_string(),
                messages:         q.get("messages").and_then(|x| x.as_u64()).unwrap_or(0),
                messages_ready:   q.get("messages_ready").and_then(|x| x.as_u64()).unwrap_or(0),
                messages_unacked: q.get("messages_unacknowledged").and_then(|x| x.as_u64()).unwrap_or(0),
                consumers:        q.get("consumers").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                publish_rate:     q.get("message_stats").and_then(|m| m.get("publish_details")).and_then(|d| d.get("rate")).and_then(|x| x.as_f64()).unwrap_or(0.0),
                deliver_rate:     q.get("message_stats").and_then(|m| m.get("deliver_get_details")).and_then(|d| d.get("rate")).and_then(|x| x.as_f64()).unwrap_or(0.0),
                memory_mb:        q.get("memory").and_then(|x| x.as_u64()).unwrap_or(0) as f64 / 1_048_576.0,
                state:            q.get("state").and_then(|x| x.as_str()).unwrap_or("unknown").to_string(),
            }).collect();
            Ok(Json(queues))
        }
        _ => Ok(Json(vec![]))
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RabbitMQHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'messages_ready' THEN value END) AS messages_ready
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'rabbitmq'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<RabbitMQHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        RabbitMQHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            messages_ready: r.get::<Option<f64>, _>("messages_ready").unwrap_or(0.0) as u64,
            publish_rate: 0.0, deliver_rate: 0.0, connections: 0,
        }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(s): State<AppState>, Json(req): Json<CreateRabbitMQRequest>) -> Result<Json<RabbitMQInstance>, ApiError> {
    let inst = RabbitMQInstance { id: crate::error::short_id("rabbitmq"), name: req.name, host: req.host, port: req.port.unwrap_or(15672), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    let _ = sqlx::query(
        "INSERT INTO middleware_instances (id, instance_type, name, host, port) VALUES ($1, 'rabbitmq', $2, $3, $4) ON CONFLICT (id) DO NOTHING"
    ).bind(&inst.id).bind(&inst.name).bind(&inst.host).bind(inst.port as i32).execute(&s.pool).await;
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = sqlx::query("DELETE FROM middleware_instances WHERE id = $1 AND instance_type = 'rabbitmq'").bind(&id).execute(&s.pool).await;
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
