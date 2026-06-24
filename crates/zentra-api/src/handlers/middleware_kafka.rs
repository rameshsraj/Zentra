use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<KafkaStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = KafkaStatus { broker_count: 0, topic_count: 0, partition_count: 0, under_replicated_partitions: 0, isr_shrinks: 0, isr_expands: 0, messages_in_per_sec: 0.0, bytes_in_per_sec: 0.0, bytes_out_per_sec: 0.0, consumer_groups: 0, total_consumer_lag: 0, controller_id: 0, active_controller_count: 0 };
    let url = format!("http://{}:{}/metrics", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            let mut msgs_in = 0.0f64;
            let mut bytes_in = 0.0f64;
            let mut bytes_out = 0.0f64;
            let mut under_rep = 0u32;
            for line in body.lines() {
                if line.starts_with('#') { continue; }
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts.len() < 2 { continue; }
                let val: f64 = parts[1].trim().parse().unwrap_or(0.0);
                if parts[0].contains("MessagesInPerSec") && parts[0].contains("OneMinuteRate") { msgs_in = val; }
                if parts[0].contains("BytesInPerSec") && parts[0].contains("OneMinuteRate") { bytes_in = val; }
                if parts[0].contains("BytesOutPerSec") && parts[0].contains("OneMinuteRate") { bytes_out = val; }
                if parts[0].contains("UnderReplicatedPartitions") { under_rep = val as u32; }
            }
            let now = Utc::now();
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'kafka','messages_in_per_sec',$2,$3)"
            ).bind(&id).bind(msgs_in).bind(now).execute(&s.pool).await;
            Ok(Json(KafkaStatus { broker_count: 0, topic_count: 0, partition_count: 0, under_replicated_partitions: under_rep, isr_shrinks: 0, isr_expands: 0, messages_in_per_sec: msgs_in, bytes_in_per_sec: bytes_in, bytes_out_per_sec: bytes_out, consumer_groups: 0, total_consumer_lag: 0, controller_id: 0, active_controller_count: 1 }))
        }
        Ok(resp) => { tracing::warn!(url=%url, status=%resp.status(), "Kafka metrics endpoint error"); Ok(Json(empty)) }
        Err(e) => { tracing::warn!(url=%url, err=%e, "Failed to reach Kafka endpoint"); Ok(Json(empty)) }
    }
}

pub async fn get_topics(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<KafkaTopic>>, ApiError> {
    let insts = instances().read();
    let _inst = insts.iter().find(|i| i.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?;
    Ok(Json(vec![]))
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<KafkaHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'messages_in_per_sec' THEN value END) AS msgs_in
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'kafka'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<KafkaHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        KafkaHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            messages_in_per_sec: r.get::<Option<f64>, _>("msgs_in").unwrap_or(0.0),
            bytes_in_per_sec: 0.0, bytes_out_per_sec: 0.0, consumer_lag: 0,
        }
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
