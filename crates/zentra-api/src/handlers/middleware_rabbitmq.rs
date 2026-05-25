use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
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

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<RabbitMQInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<RabbitMQStatus>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(RabbitMQStatus {
        queues_total: 0, messages_ready: 0, messages_unacked: 0,
        publish_rate: 0.0, deliver_rate: 0.0, connections: 0, channels: 0, consumers: 0,
        node_memory_mb: 0.0, node_disk_free_gb: 0.0, node_fd_used: 0, node_fd_total: 0,
        erlang_version: String::new(), rabbitmq_version: String::new(),
    }));
}

pub async fn get_queues(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RabbitMQQueue>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let queues = vec![
        RabbitMQQueue { name: "orders.new".into(), vhost: "/".into(), messages: 450, messages_ready: 420, messages_unacked: 30, consumers: 4, publish_rate: 120.5, deliver_rate: 118.2, memory_mb: 24.5, state: "running".into() },
        RabbitMQQueue { name: "orders.process".into(), vhost: "/".into(), messages: 120, messages_ready: 100, messages_unacked: 20, consumers: 2, publish_rate: 118.2, deliver_rate: 115.0, memory_mb: 12.1, state: "running".into() },
        RabbitMQQueue { name: "notifications.email".into(), vhost: "/".into(), messages: 85, messages_ready: 80, messages_unacked: 5, consumers: 3, publish_rate: 45.0, deliver_rate: 44.8, memory_mb: 8.3, state: "running".into() },
        RabbitMQQueue { name: "notifications.sms".into(), vhost: "/".into(), messages: 12, messages_ready: 10, messages_unacked: 2, consumers: 2, publish_rate: 8.5, deliver_rate: 8.4, memory_mb: 2.1, state: "running".into() },
        RabbitMQQueue { name: "analytics.events".into(), vhost: "/".into(), messages: 2500, messages_ready: 2400, messages_unacked: 100, consumers: 6, publish_rate: 850.0, deliver_rate: 840.0, memory_mb: 64.2, state: "running".into() },
        RabbitMQQueue { name: "dlx.failed".into(), vhost: "/".into(), messages: 23, messages_ready: 23, messages_unacked: 0, consumers: 0, publish_rate: 0.3, deliver_rate: 0.0, memory_mb: 1.2, state: "running".into() },
    ];
    Ok(Json(queues))
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<RabbitMQHistoryPoint>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let now = Utc::now();
    let points: Vec<RabbitMQHistoryPoint> = (0..60).rev().map(|i| {
        let j = (i as f64 * 0.1).sin();
        RabbitMQHistoryPoint { timestamp: (now - Duration::minutes(i)).to_rfc3339(), messages_ready: (1250.0 + j * 500.0) as u64, publish_rate: 5200.0 + j * 800.0, deliver_rate: 5150.0 + j * 780.0, connections: (48.0 + j * 5.0) as u32 }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateRabbitMQRequest>) -> Result<Json<RabbitMQInstance>, ApiError> {
    let inst = RabbitMQInstance { id: crate::error::short_id("rabbitmq"), name: req.name, host: req.host, port: req.port.unwrap_or(15672), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
