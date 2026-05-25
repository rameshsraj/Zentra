use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElasticsearchInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }

#[derive(Debug, Clone, Serialize)]
pub struct ElasticsearchStatus {
    pub cluster_name: String, pub cluster_status: String, pub node_count: u32,
    pub active_shards: u32, pub relocating_shards: u32, pub initializing_shards: u32, pub unassigned_shards: u32,
    pub index_count: u32, pub doc_count: u64, pub store_size_gb: f64,
    pub indexing_rate: f64, pub search_rate: f64,
    pub jvm_heap_used_pct: f64, pub gc_old_count: u64, pub gc_young_count: u64,
    pub es_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ElasticsearchIndex { pub name: String, pub health: String, pub status: String, pub docs_count: u64, pub store_size_mb: f64, pub pri_shards: u32, pub replicas: u32 }

#[derive(Debug, Clone, Serialize)]
pub struct ElasticsearchHistoryPoint { pub timestamp: String, pub indexing_rate: f64, pub search_rate: f64, pub jvm_heap_pct: f64, pub doc_count: u64 }

#[derive(Debug, Deserialize)]
pub struct CreateElasticsearchRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<ElasticsearchInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<ElasticsearchInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<ElasticsearchInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<ElasticsearchInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<ElasticsearchStatus>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(ElasticsearchStatus {
        cluster_name: String::new(), cluster_status: "unknown".into(), node_count: 0,
        active_shards: 0, relocating_shards: 0, initializing_shards: 0, unassigned_shards: 0,
        index_count: 0, doc_count: 0, store_size_gb: 0.0,
        indexing_rate: 0.0, search_rate: 0.0,
        jvm_heap_used_pct: 0.0, gc_old_count: 0, gc_young_count: 0,
        es_version: String::new(),
    }));
}

pub async fn get_indices(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<ElasticsearchIndex>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let indices = vec![
        ElasticsearchIndex { name: "logs-2026.04.25".into(), health: "green".into(), status: "open".into(), docs_count: 12_500_000, store_size_mb: 4500.0, pri_shards: 5, replicas: 1 },
        ElasticsearchIndex { name: "logs-2026.04.24".into(), health: "green".into(), status: "open".into(), docs_count: 14_200_000, store_size_mb: 5100.0, pri_shards: 5, replicas: 1 },
        ElasticsearchIndex { name: "metrics-2026.04".into(), health: "green".into(), status: "open".into(), docs_count: 85_000_000, store_size_mb: 24000.0, pri_shards: 10, replicas: 1 },
        ElasticsearchIndex { name: "events-app".into(), health: "green".into(), status: "open".into(), docs_count: 45_000_000, store_size_mb: 12000.0, pri_shards: 5, replicas: 1 },
        ElasticsearchIndex { name: "alerts-history".into(), health: "green".into(), status: "open".into(), docs_count: 2_500_000, store_size_mb: 800.0, pri_shards: 3, replicas: 1 },
        ElasticsearchIndex { name: ".kibana".into(), health: "green".into(), status: "open".into(), docs_count: 1250, store_size_mb: 12.0, pri_shards: 1, replicas: 1 },
    ];
    Ok(Json(indices))
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<ElasticsearchHistoryPoint>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let now = Utc::now();
    let points: Vec<ElasticsearchHistoryPoint> = (0..60).rev().map(|i| {
        let j = (i as f64 * 0.1).sin();
        ElasticsearchHistoryPoint { timestamp: (now - Duration::minutes(i)).to_rfc3339(), indexing_rate: 450.0 + j * 100.0, search_rate: 1200.0 + j * 300.0, jvm_heap_pct: 62.5 + j * 8.0, doc_count: (245_000_000.0 + i as f64 * 10000.0) as u64 }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateElasticsearchRequest>) -> Result<Json<ElasticsearchInstance>, ApiError> {
    let inst = ElasticsearchInstance { id: crate::error::short_id("es"), name: req.name, host: req.host, port: req.port.unwrap_or(9200), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}

pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write();
    let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
