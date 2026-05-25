use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KafkaInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize)]
pub struct KafkaStatus {
    pub broker_count: u32, pub topic_count: u32, pub partition_count: u32,
    pub under_replicated_partitions: u32, pub isr_shrinks: u64, pub isr_expands: u64,
    pub messages_in_per_sec: f64, pub bytes_in_per_sec: f64, pub bytes_out_per_sec: f64,
    pub consumer_groups: u32, pub total_consumer_lag: u64,
    pub controller_id: u32, pub active_controller_count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct KafkaTopic { pub name: String, pub partitions: u32, pub replication_factor: u32, pub messages: u64, pub size_mb: f64, pub retention_hours: u32, pub consumer_groups: u32 }

#[derive(Debug, Clone, Serialize)]
pub struct KafkaHistoryPoint { pub timestamp: String, pub messages_in_per_sec: f64, pub bytes_in_per_sec: f64, pub bytes_out_per_sec: f64, pub consumer_lag: u64 }

#[derive(Debug, Deserialize)]
pub struct CreateKafkaRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<KafkaInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<KafkaInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<KafkaInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<KafkaInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<KafkaStatus>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(KafkaStatus {
        broker_count: 0, topic_count: 0, partition_count: 0,
        under_replicated_partitions: 0, isr_shrinks: 0, isr_expands: 0,
        messages_in_per_sec: 0.0, bytes_in_per_sec: 0.0, bytes_out_per_sec: 0.0,
        consumer_groups: 0, total_consumer_lag: 0,
        controller_id: 0, active_controller_count: 0,
    }));
}

pub async fn get_topics(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<KafkaTopic>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let topics = vec![
        KafkaTopic { name: "events.user-activity".into(), partitions: 12, replication_factor: 3, messages: 45_000_000, size_mb: 2400.0, retention_hours: 168, consumer_groups: 3 },
        KafkaTopic { name: "events.orders".into(), partitions: 6, replication_factor: 3, messages: 12_000_000, size_mb: 850.0, retention_hours: 720, consumer_groups: 2 },
        KafkaTopic { name: "events.notifications".into(), partitions: 3, replication_factor: 3, messages: 8_500_000, size_mb: 420.0, retention_hours: 48, consumer_groups: 1 },
        KafkaTopic { name: "logs.application".into(), partitions: 12, replication_factor: 2, messages: 120_000_000, size_mb: 8500.0, retention_hours: 72, consumer_groups: 2 },
        KafkaTopic { name: "metrics.infrastructure".into(), partitions: 6, replication_factor: 2, messages: 85_000_000, size_mb: 3200.0, retention_hours: 24, consumer_groups: 1 },
        KafkaTopic { name: "dlq.failures".into(), partitions: 1, replication_factor: 3, messages: 2_500, size_mb: 0.8, retention_hours: 8760, consumer_groups: 0 },
    ];
    Ok(Json(topics))
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<KafkaHistoryPoint>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let now = Utc::now();
    let points: Vec<KafkaHistoryPoint> = (0..60).rev().map(|i| {
        let j = (i as f64 * 0.1).sin();
        KafkaHistoryPoint { timestamp: (now - Duration::minutes(i)).to_rfc3339(), messages_in_per_sec: 12500.0 + j * 2000.0, bytes_in_per_sec: 4_500_000.0 + j * 800000.0, bytes_out_per_sec: 8_200_000.0 + j * 1200000.0, consumer_lag: (450.0 + j * 200.0).max(0.0) as u64 }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateKafkaRequest>) -> Result<Json<KafkaInstance>, ApiError> {
    let inst = KafkaInstance { id: crate::error::short_id("kafka"), name: req.name, host: req.host, port: req.port.unwrap_or(9092), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
