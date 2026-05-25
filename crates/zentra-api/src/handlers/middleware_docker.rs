use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
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

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<DockerStatus>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(DockerStatus {
        containers_running: 0, containers_stopped: 0, containers_paused: 0,
        images: 0, volumes: 0, networks: 0,
        cpu_pct: 0.0, memory_used_mb: 0.0, memory_total_mb: 0.0,
        docker_version: String::new(), storage_driver: String::new(),
    }));
}

pub async fn get_containers(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DockerContainer>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let containers = vec![
        DockerContainer { container_id: "a1b2c3d4e5f6".into(), name: "nginx-proxy".into(), image: "nginx:1.25".into(), status: "Up 15 days".into(), state: "running".into(), cpu_pct: 2.1, memory_mb: 128.0, memory_limit_mb: 512.0, net_rx_mb: 4500.0, net_tx_mb: 12000.0, block_read_mb: 45.0, block_write_mb: 12.0, restart_count: 0, created: (Utc::now() - Duration::days(15)).to_rfc3339(), ports: vec!["80:80".into(), "443:443".into()] },
        DockerContainer { container_id: "b2c3d4e5f6a7".into(), name: "api-server".into(), image: "Zentra/api:latest".into(), status: "Up 3 days".into(), state: "running".into(), cpu_pct: 8.5, memory_mb: 512.0, memory_limit_mb: 2048.0, net_rx_mb: 2200.0, net_tx_mb: 8500.0, block_read_mb: 120.0, block_write_mb: 85.0, restart_count: 1, created: (Utc::now() - Duration::days(3)).to_rfc3339(), ports: vec!["7001:7001".into()] },
        DockerContainer { container_id: "c3d4e5f6a7b8".into(), name: "postgres-db".into(), image: "postgres:16".into(), status: "Up 30 days".into(), state: "running".into(), cpu_pct: 5.2, memory_mb: 1024.0, memory_limit_mb: 4096.0, net_rx_mb: 850.0, net_tx_mb: 3200.0, block_read_mb: 8500.0, block_write_mb: 4200.0, restart_count: 0, created: (Utc::now() - Duration::days(30)).to_rfc3339(), ports: vec!["5432:5432".into()] },
        DockerContainer { container_id: "d4e5f6a7b8c9".into(), name: "redis-cache".into(), image: "redis:7.2".into(), status: "Up 30 days".into(), state: "running".into(), cpu_pct: 1.8, memory_mb: 2048.0, memory_limit_mb: 4096.0, net_rx_mb: 1200.0, net_tx_mb: 4500.0, block_read_mb: 25.0, block_write_mb: 180.0, restart_count: 0, created: (Utc::now() - Duration::days(30)).to_rfc3339(), ports: vec!["6379:6379".into()] },
        DockerContainer { container_id: "e5f6a7b8c9d0".into(), name: "grafana".into(), image: "grafana/grafana:10.3".into(), status: "Up 7 days".into(), state: "running".into(), cpu_pct: 3.1, memory_mb: 256.0, memory_limit_mb: 1024.0, net_rx_mb: 120.0, net_tx_mb: 450.0, block_read_mb: 35.0, block_write_mb: 8.0, restart_count: 0, created: (Utc::now() - Duration::days(7)).to_rfc3339(), ports: vec!["3000:3000".into()] },
        DockerContainer { container_id: "f6a7b8c9d0e1".into(), name: "prometheus".into(), image: "prom/prometheus:v2.50".into(), status: "Up 7 days".into(), state: "running".into(), cpu_pct: 4.5, memory_mb: 1536.0, memory_limit_mb: 4096.0, net_rx_mb: 3500.0, net_tx_mb: 850.0, block_read_mb: 12000.0, block_write_mb: 8500.0, restart_count: 0, created: (Utc::now() - Duration::days(7)).to_rfc3339(), ports: vec!["9090:9090".into()] },
        DockerContainer { container_id: "a7b8c9d0e1f2".into(), name: "alertmanager".into(), image: "prom/alertmanager:v0.27".into(), status: "Up 7 days".into(), state: "running".into(), cpu_pct: 0.3, memory_mb: 64.0, memory_limit_mb: 256.0, net_rx_mb: 25.0, net_tx_mb: 12.0, block_read_mb: 2.0, block_write_mb: 1.0, restart_count: 0, created: (Utc::now() - Duration::days(7)).to_rfc3339(), ports: vec!["9093:9093".into()] },
        DockerContainer { container_id: "b8c9d0e1f2a3".into(), name: "old-migration".into(), image: "migrate:v1".into(), status: "Exited (0) 5 days ago".into(), state: "exited".into(), cpu_pct: 0.0, memory_mb: 0.0, memory_limit_mb: 512.0, net_rx_mb: 0.0, net_tx_mb: 0.0, block_read_mb: 0.5, block_write_mb: 0.1, restart_count: 0, created: (Utc::now() - Duration::days(10)).to_rfc3339(), ports: vec![] },
    ];
    Ok(Json(containers))
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DockerHistoryPoint>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let now = Utc::now();
    let points: Vec<DockerHistoryPoint> = (0..60).rev().map(|i| { let j = (i as f64 * 0.1).sin();
        DockerHistoryPoint { timestamp: (now - Duration::minutes(i)).to_rfc3339(), containers_running: (18.0 + j * 2.0) as u32, cpu_pct: 34.5 + j * 8.0, memory_used_mb: 8192.0 + j * 500.0, net_rx_mb: (4500.0 + j * 500.0), net_tx_mb: (12000.0 + j * 1500.0) }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateDockerRequest>) -> Result<Json<DockerInstance>, ApiError> {
    let inst = DockerInstance { id: crate::error::short_id("docker"), name: req.name, host: req.host, port: req.port.unwrap_or(2375), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
