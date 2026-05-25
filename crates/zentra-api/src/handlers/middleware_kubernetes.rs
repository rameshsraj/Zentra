use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sInstance { pub id: String, pub name: String, pub host: String, pub port: u16, pub status: String, pub created_at: String }
#[derive(Debug, Clone, Serialize)]
pub struct K8sStatus {
    pub node_count: u32, pub nodes_ready: u32, pub pod_count: u32,
    pub pods_running: u32, pub pods_pending: u32, pub pods_failed: u32,
    pub deployment_count: u32, pub service_count: u32, pub namespace_count: u32,
    pub cpu_requests_pct: f64, pub cpu_limits_pct: f64, pub mem_requests_pct: f64, pub mem_limits_pct: f64,
    pub k8s_version: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct K8sNode { pub name: String, pub status: String, pub role: String, pub version: String, pub os: String, pub cpu_cores: u32, pub memory_gb: f64, pub pods: u32, pub cpu_pct: f64, pub memory_pct: f64 }
#[derive(Debug, Clone, Serialize)]
pub struct K8sPod { pub name: String, pub namespace: String, pub status: String, pub restarts: u32, pub age: String, pub node: String, pub cpu_m: u32, pub memory_mi: u32, pub ready: String }
#[derive(Debug, Clone, Serialize)]
pub struct K8sDeployment { pub name: String, pub namespace: String, pub replicas: u32, pub ready: u32, pub updated: u32, pub available: u32, pub age: String, pub image: String }
#[derive(Debug, Clone, Serialize)]
pub struct K8sHistoryPoint { pub timestamp: String, pub pods_running: u32, pub cpu_pct: f64, pub memory_pct: f64, pub pod_restarts: u32 }
#[derive(Debug, Deserialize)]
pub struct CreateK8sRequest { pub name: String, pub host: String, pub port: Option<u16> }

fn instances() -> &'static RwLock<Vec<K8sInstance>> {
    static INSTANCE: OnceLock<RwLock<Vec<K8sInstance>>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
pub fn live_instances() -> &'static RwLock<Vec<K8sInstance>> { instances() }

pub async fn list_instances(State(_s): State<AppState>) -> Result<Json<Vec<K8sInstance>>, ApiError> { Ok(Json(instances().read().clone())) }

pub async fn get_status(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<K8sStatus>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    return Ok(Json(K8sStatus {
        node_count: 0, nodes_ready: 0, pod_count: 0, pods_running: 0, pods_pending: 0, pods_failed: 0,
        deployment_count: 0, service_count: 0, namespace_count: 0,
        cpu_requests_pct: 0.0, cpu_limits_pct: 0.0, mem_requests_pct: 0.0, mem_limits_pct: 0.0,
        k8s_version: String::new(),
    }));
}

pub async fn get_nodes(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sNode>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![
        K8sNode { name: "node-01".into(), status: "Ready".into(), role: "control-plane".into(), version: "v1.29.2".into(), os: "Ubuntu 22.04".into(), cpu_cores: 8, memory_gb: 32.0, pods: 12, cpu_pct: 45.2, memory_pct: 62.0 },
        K8sNode { name: "node-02".into(), status: "Ready".into(), role: "worker".into(), version: "v1.29.2".into(), os: "Ubuntu 22.04".into(), cpu_cores: 16, memory_gb: 64.0, pods: 10, cpu_pct: 68.5, memory_pct: 55.0 },
        K8sNode { name: "node-03".into(), status: "Ready".into(), role: "worker".into(), version: "v1.29.2".into(), os: "Ubuntu 22.04".into(), cpu_cores: 16, memory_gb: 64.0, pods: 8, cpu_pct: 52.3, memory_pct: 48.0 },
        K8sNode { name: "node-04".into(), status: "Ready".into(), role: "worker".into(), version: "v1.29.2".into(), os: "Ubuntu 22.04".into(), cpu_cores: 16, memory_gb: 64.0, pods: 7, cpu_pct: 38.1, memory_pct: 42.0 },
        K8sNode { name: "node-05".into(), status: "Ready".into(), role: "worker".into(), version: "v1.29.2".into(), os: "Ubuntu 22.04".into(), cpu_cores: 16, memory_gb: 64.0, pods: 5, cpu_pct: 25.4, memory_pct: 35.0 },
    ]))
}

pub async fn get_pods(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sPod>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![
        K8sPod { name: "api-deploy-7b8c9-abc12".into(), namespace: "production".into(), status: "Running".into(), restarts: 0, age: "3d".into(), node: "node-02".into(), cpu_m: 250, memory_mi: 512, ready: "1/1".into() },
        K8sPod { name: "api-deploy-7b8c9-def34".into(), namespace: "production".into(), status: "Running".into(), restarts: 1, age: "3d".into(), node: "node-03".into(), cpu_m: 280, memory_mi: 480, ready: "1/1".into() },
        K8sPod { name: "web-deploy-4a5b6-ghi56".into(), namespace: "production".into(), status: "Running".into(), restarts: 0, age: "7d".into(), node: "node-02".into(), cpu_m: 100, memory_mi: 128, ready: "1/1".into() },
        K8sPod { name: "worker-deploy-1c2d3-jkl78".into(), namespace: "production".into(), status: "Running".into(), restarts: 0, age: "5d".into(), node: "node-04".into(), cpu_m: 500, memory_mi: 1024, ready: "1/1".into() },
        K8sPod { name: "postgres-sts-0".into(), namespace: "database".into(), status: "Running".into(), restarts: 0, age: "30d".into(), node: "node-03".into(), cpu_m: 1000, memory_mi: 4096, ready: "1/1".into() },
        K8sPod { name: "redis-sts-0".into(), namespace: "cache".into(), status: "Running".into(), restarts: 0, age: "30d".into(), node: "node-04".into(), cpu_m: 200, memory_mi: 2048, ready: "1/1".into() },
        K8sPod { name: "migration-job-xyz".into(), namespace: "production".into(), status: "Failed".into(), restarts: 3, age: "1d".into(), node: "node-05".into(), cpu_m: 100, memory_mi: 256, ready: "0/1".into() },
        K8sPod { name: "pending-pod-abc".into(), namespace: "staging".into(), status: "Pending".into(), restarts: 0, age: "2h".into(), node: "".into(), cpu_m: 500, memory_mi: 1024, ready: "0/1".into() },
    ]))
}

pub async fn get_deployments(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sDeployment>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    Ok(Json(vec![
        K8sDeployment { name: "api-deploy".into(), namespace: "production".into(), replicas: 3, ready: 3, updated: 3, available: 3, age: "30d".into(), image: "Zentra/api:v2.1".into() },
        K8sDeployment { name: "web-deploy".into(), namespace: "production".into(), replicas: 2, ready: 2, updated: 2, available: 2, age: "30d".into(), image: "Zentra/web:v2.1".into() },
        K8sDeployment { name: "worker-deploy".into(), namespace: "production".into(), replicas: 4, ready: 4, updated: 4, available: 4, age: "15d".into(), image: "Zentra/worker:v2.0".into() },
        K8sDeployment { name: "grafana".into(), namespace: "monitoring".into(), replicas: 1, ready: 1, updated: 1, available: 1, age: "60d".into(), image: "grafana/grafana:10.3".into() },
        K8sDeployment { name: "prometheus".into(), namespace: "monitoring".into(), replicas: 1, ready: 1, updated: 1, available: 1, age: "60d".into(), image: "prom/prometheus:v2.50".into() },
    ]))
}

pub async fn performance_history(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sHistoryPoint>>, ApiError> {
    let insts = instances().read(); let _inst = insts.iter().find(|i| i.id == id).ok_or((StatusCode::NOT_FOUND, "Not found".into()))?;
    let now = Utc::now();
    let points: Vec<K8sHistoryPoint> = (0..60).rev().map(|i| { let j = (i as f64 * 0.1).sin();
        K8sHistoryPoint { timestamp: (now - Duration::minutes(i)).to_rfc3339(), pods_running: (38.0 + j * 3.0) as u32, cpu_pct: 55.0 + j * 10.0, memory_pct: 50.0 + j * 8.0, pod_restarts: (j * 2.0).max(0.0) as u32 }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(_s): State<AppState>, Json(req): Json<CreateK8sRequest>) -> Result<Json<K8sInstance>, ApiError> {
    let inst = K8sInstance { id: crate::error::short_id("k8s"), name: req.name, host: req.host, port: req.port.unwrap_or(6443), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
