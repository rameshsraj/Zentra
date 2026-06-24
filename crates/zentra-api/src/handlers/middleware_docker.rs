use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }
#[derive(Debug, Clone, Serialize)]
pub struct DockerStatus {
    pub containers_running: u32, pub containers_stopped: u32, pub containers_paused: u32,
    pub images: u32, pub volumes: u32, pub networks: u32,
    pub cpu_pct: f64, pub memory_used_mb: f64, pub memory_total_mb: f64,
    pub docker_version: String, pub storage_driver: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct DockerContainer {
    pub container_id: String, pub name: String, pub image: String, pub status: String, pub state: String,
    pub cpu_pct: f64, pub memory_mb: f64, pub memory_limit_mb: f64,
    pub net_rx_mb: f64, pub net_tx_mb: f64, pub block_read_mb: f64, pub block_write_mb: f64,
    pub restart_count: u32, pub created: String, pub ports: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct DockerHistoryPoint { pub timestamp: String, pub containers_running: u32, pub cpu_pct: f64, pub memory_used_mb: f64, pub net_rx_mb: f64, pub net_tx_mb: f64 }
#[derive(Debug, Deserialize)]
pub struct CreateDockerRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<DockerInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<DockerInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<DockerInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<DockerInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<DockerStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/v1.24/info", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let containers = v.get("Containers").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let running  = v.get("ContainersRunning").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let stopped  = v.get("ContainersStopped").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let paused   = v.get("ContainersPaused").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let images   = v.get("Images").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
            let _ = containers;
            let mem_total = v.get("MemTotal").and_then(|x| x.as_u64()).unwrap_or(0) as f64 / 1_048_576.0;
            let version = v.get("ServerVersion").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let driver  = v.get("Driver").and_then(|x| x.as_str()).unwrap_or("").to_string();
            // Store metrics snapshot for history
            let now = Utc::now();
            let _ = sqlx::query(
                "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'docker','containers_running',$2,$3)"
            ).bind(&id).bind(running as f64).bind(now).execute(&s.pool).await;
            Ok(Json(DockerStatus {
                containers_running: running, containers_stopped: stopped, containers_paused: paused,
                images, volumes: 0, networks: 0,
                cpu_pct: 0.0, memory_used_mb: 0.0, memory_total_mb: mem_total,
                docker_version: version, storage_driver: driver,
            }))
        }
        Ok(resp) => {
            tracing::warn!(url=%url, status=%resp.status(), "Docker info endpoint error");
            Ok(Json(DockerStatus { containers_running: 0, containers_stopped: 0, containers_paused: 0, images: 0, volumes: 0, networks: 0, cpu_pct: 0.0, memory_used_mb: 0.0, memory_total_mb: 0.0, docker_version: String::new(), storage_driver: String::new() }))
        }
        Err(e) => {
            tracing::warn!(url=%url, err=%e, "Failed to reach Docker endpoint");
            Ok(Json(DockerStatus { containers_running: 0, containers_stopped: 0, containers_paused: 0, images: 0, volumes: 0, networks: 0, cpu_pct: 0.0, memory_used_mb: 0.0, memory_total_mb: 0.0, docker_version: String::new(), storage_driver: String::new() }))
        }
    }
}

pub async fn get_containers(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DockerContainer>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/v1.24/containers/json?all=1", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: Vec<serde_json::Value> = resp.json().await.unwrap_or_default();
            let containers = v.iter().map(|c| {
                let names: Vec<String> = c.get("Names").and_then(|n| n.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str()).map(|s| s.trim_start_matches('/').to_string()).collect())
                    .unwrap_or_default();
                let name = names.first().cloned().unwrap_or_default();
                let ports: Vec<String> = c.get("Ports").and_then(|p| p.as_array())
                    .map(|arr| arr.iter().filter_map(|p| {
                        let pub_port = p.get("PublicPort").and_then(|v| v.as_u64())?;
                        let priv_port = p.get("PrivatePort").and_then(|v| v.as_u64())?;
                        Some(format!("{}:{}", pub_port, priv_port))
                    }).collect())
                    .unwrap_or_default();
                DockerContainer {
                    container_id: c.get("Id").and_then(|v| v.as_str()).unwrap_or("").chars().take(12).collect(),
                    name,
                    image: c.get("Image").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    status: c.get("Status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    state: c.get("State").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    cpu_pct: 0.0, memory_mb: 0.0, memory_limit_mb: 0.0,
                    net_rx_mb: 0.0, net_tx_mb: 0.0, block_read_mb: 0.0, block_write_mb: 0.0,
                    restart_count: 0,
                    created: c.get("Created").and_then(|v| v.as_i64())
                        .map(|ts| chrono::DateTime::from_timestamp(ts, 0).map(|d| d.to_rfc3339()).unwrap_or_default())
                        .unwrap_or_default(),
                    ports,
                }
            }).collect();
            Ok(Json(containers))
        }
        _ => Ok(Json(vec![]))
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DockerHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'containers_running' THEN value END) AS containers_running
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'docker'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    )
    .bind(&id)
    .fetch_all(&s.pool)
    .await
    .unwrap_or_default();
    let points: Vec<DockerHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        DockerHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            containers_running: r.get::<Option<f64>, _>("containers_running").unwrap_or(0.0) as u32,
            cpu_pct: 0.0, memory_used_mb: 0.0, net_rx_mb: 0.0, net_tx_mb: 0.0,
        }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateDockerRequest>) -> Result<Json<DockerInstance>, ApiError> {
    let inst = DockerInstance { id: crate::error::short_id("docker"), name: req.name, host: req.host, port: req.port.unwrap_or(2375), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone());
    Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
