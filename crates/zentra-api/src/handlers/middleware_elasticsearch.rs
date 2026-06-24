use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<ElasticsearchStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = ElasticsearchStatus { cluster_name: String::new(), cluster_status: "unknown".into(), node_count: 0, active_shards: 0, relocating_shards: 0, initializing_shards: 0, unassigned_shards: 0, index_count: 0, doc_count: 0, store_size_gb: 0.0, indexing_rate: 0.0, search_rate: 0.0, jvm_heap_used_pct: 0.0, gc_old_count: 0, gc_young_count: 0, es_version: String::new() };
    let url = format!("http://{}:{}/_cluster/health", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let cluster_name  = v.get("cluster_name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let cluster_status = v.get("status").and_then(|x| x.as_str()).unwrap_or("unknown").to_string();
            let node_count    = v.get("number_of_nodes").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let active_shards = v.get("active_shards").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let relocating    = v.get("relocating_shards").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let initializing  = v.get("initializing_shards").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let unassigned    = v.get("unassigned_shards").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'elasticsearch','active_shards',$2,now())"
            ).bind(&id).bind(active_shards as f64).execute(&s.pool).await;
            Ok(Json(ElasticsearchStatus { cluster_name, cluster_status, node_count, active_shards, relocating_shards: relocating, initializing_shards: initializing, unassigned_shards: unassigned, index_count: 0, doc_count: 0, store_size_gb: 0.0, indexing_rate: 0.0, search_rate: 0.0, jvm_heap_used_pct: 0.0, gc_old_count: 0, gc_young_count: 0, es_version: String::new() }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "ES cluster health error"); Ok(Json(empty)) }
        Err(e)   => { tracing::warn!(url=%url, err=%e, "Failed to reach ES"); Ok(Json(empty)) }
    }
}

pub async fn get_indices(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<ElasticsearchIndex>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/_cat/indices?format=json", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
            let indices = v.iter().map(|idx| ElasticsearchIndex {
                name:          idx.get("index").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                health:        idx.get("health").and_then(|x| x.as_str()).unwrap_or("unknown").to_string(),
                status:        idx.get("status").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                docs_count:    idx.get("docs.count").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0),
                store_size_mb: idx.get("store.size").and_then(|x| x.as_str()).and_then(|s| s.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.').parse().ok()).unwrap_or(0.0),
                pri_shards:    idx.get("pri").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0),
                replicas:      idx.get("rep").and_then(|x| x.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0),
            }).collect();
            Ok(Json(indices))
        }
        _ => Ok(Json(vec![]))
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<ElasticsearchHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'active_shards' THEN value END) AS active_shards
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'elasticsearch'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<ElasticsearchHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        ElasticsearchHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            indexing_rate: 0.0, search_rate: 0.0, jvm_heap_pct: 0.0,
            doc_count: r.get::<Option<f64>, _>("active_shards").unwrap_or(0.0) as u64,
        }
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
