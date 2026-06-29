use axum::{extract::{State, Path, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::Utc;
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

pub async fn list_instances(State(s): State<AppState>) -> Result<Json<Vec<K8sInstance>>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, String)>(
        "SELECT id, name, host, port, status, created_at::TEXT FROM middleware_instances WHERE instance_type = 'kubernetes' ORDER BY created_at"
    ).fetch_all(&s.pool).await.unwrap_or_default();
    let from_db: Vec<K8sInstance> = rows.into_iter().map(|(id, name, host, port, status, created_at)|
        K8sInstance { id, name, host, port: port as u16, status, created_at }
    ).collect();
    if !from_db.is_empty() { *instances().write() = from_db.clone(); return Ok(Json(from_db)); }
    Ok(Json(instances().read().clone()))
}

pub async fn get_status(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<K8sStatus>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let empty = K8sStatus { node_count: 0, nodes_ready: 0, pod_count: 0, pods_running: 0, pods_pending: 0, pods_failed: 0, deployment_count: 0, service_count: 0, namespace_count: 0, cpu_requests_pct: 0.0, cpu_limits_pct: 0.0, mem_requests_pct: 0.0, mem_limits_pct: 0.0, k8s_version: String::new() };
    // K8s API server (unauthenticated /version + /api/v1/nodes)
    let version_url = format!("http://{}:{}/version", inst.host, inst.port);
    let nodes_url   = format!("http://{}:{}/api/v1/nodes", inst.host, inst.port);
    let pods_url    = format!("http://{}:{}/api/v1/pods", inst.host, inst.port);
    let version = match s.http_client.get(&version_url).send().await {
        Ok(r) if r.status().is_success() => r.json::<serde_json::Value>().await.ok()
            .and_then(|v| v.get("gitVersion").and_then(|x| x.as_str()).map(|s| s.to_string()))
            .unwrap_or_default(),
        _ => String::new(),
    };
    let (node_count, nodes_ready) = match s.http_client.get(&nodes_url).send().await {
        Ok(r) if r.status().is_success() => {
            let v: serde_json::Value = r.json().await.unwrap_or_default();
            let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let total = items.len() as u32;
            let ready = items.iter().filter(|n| {
                n.get("status").and_then(|s| s.get("conditions")).and_then(|c| c.as_array())
                    .map(|conds| conds.iter().any(|c| c.get("type").and_then(|t| t.as_str()) == Some("Ready") && c.get("status").and_then(|s| s.as_str()) == Some("True")))
                    .unwrap_or(false)
            }).count() as u32;
            (total, ready)
        }
        _ => (0, 0),
    };
    let (pod_count, pods_running, pods_pending, pods_failed) = match s.http_client.get(&pods_url).send().await {
        Ok(r) if r.status().is_success() => {
            let v: serde_json::Value = r.json().await.unwrap_or_default();
            let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let total = items.len() as u32;
            let running = items.iter().filter(|p| p.get("status").and_then(|s| s.get("phase")).and_then(|x| x.as_str()) == Some("Running")).count() as u32;
            let pending = items.iter().filter(|p| p.get("status").and_then(|s| s.get("phase")).and_then(|x| x.as_str()) == Some("Pending")).count() as u32;
            let failed  = items.iter().filter(|p| p.get("status").and_then(|s| s.get("phase")).and_then(|x| x.as_str()) == Some("Failed")).count() as u32;
            (total, running, pending, failed)
        }
        _ => (0, 0, 0, 0),
    };
    let _ = sqlx::query(
        "INSERT INTO middleware_metrics (instance_id, instance_type, metric_name, value, collected_at) VALUES ($1,'kubernetes','pods_running',$2,now())"
    ).bind(&id).bind(pods_running as f64).execute(&s.pool).await;
    Ok(Json(K8sStatus { node_count, nodes_ready, pod_count, pods_running, pods_pending, pods_failed, deployment_count: 0, service_count: 0, namespace_count: 0, cpu_requests_pct: 0.0, cpu_limits_pct: 0.0, mem_requests_pct: 0.0, mem_limits_pct: 0.0, k8s_version: version }))
}

pub async fn get_nodes(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sNode>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/api/v1/nodes", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let nodes = items.iter().map(|n| {
                let meta    = n.get("metadata").cloned().unwrap_or_default();
                let status  = n.get("status").cloned().unwrap_or_default();
                let info    = status.get("nodeInfo").cloned().unwrap_or_default();
                let name    = meta.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let version = info.get("kubeletVersion").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let os      = info.get("osImage").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let ready   = status.get("conditions").and_then(|c| c.as_array())
                    .map(|conds| conds.iter().any(|c| c.get("type").and_then(|t| t.as_str()) == Some("Ready") && c.get("status").and_then(|s| s.as_str()) == Some("True")))
                    .unwrap_or(false);
                K8sNode { name, status: if ready { "Ready".into() } else { "NotReady".into() }, role: String::new(), version, os, cpu_cores: 0, memory_gb: 0.0, pods: 0, cpu_pct: 0.0, memory_pct: 0.0 }
            }).collect();
            Ok(Json(nodes))
        }
        _ => Ok(Json(vec![]))
    }
}

pub async fn get_pods(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sPod>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/api/v1/pods", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let pods = items.iter().map(|p| {
                let meta = p.get("metadata").cloned().unwrap_or_default();
                let spec = p.get("spec").cloned().unwrap_or_default();
                let status = p.get("status").cloned().unwrap_or_default();
                let cs: Vec<serde_json::Value> = status.get("containerStatuses").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                let restarts = cs.iter().map(|c| c.get("restartCount").and_then(|x| x.as_u64()).unwrap_or(0)).sum::<u64>() as u32;
                K8sPod {
                    name:      meta.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    namespace: meta.get("namespace").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    status:    status.get("phase").and_then(|x| x.as_str()).unwrap_or("Unknown").to_string(),
                    restarts, age: String::new(),
                    node:      spec.get("nodeName").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    cpu_m: 0, memory_mi: 0,
                    ready: format!("{}/{}", cs.iter().filter(|c| c.get("ready").and_then(|x| x.as_bool()).unwrap_or(false)).count(), cs.len()),
                }
            }).collect();
            Ok(Json(pods))
        }
        _ => Ok(Json(vec![]))
    }
}

pub async fn get_deployments(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sDeployment>>, ApiError> {
    let inst = {
        let insts = instances().read();
        insts.iter().find(|i| i.id == id).cloned()
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())))?
    };
    let url = format!("http://{}:{}/apis/apps/v1/deployments", inst.host, inst.port);
    match s.http_client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let v: serde_json::Value = resp.json().await.unwrap_or_default();
            let items = v.get("items").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let deployments = items.iter().map(|d| {
                let meta   = d.get("metadata").cloned().unwrap_or_default();
                let spec   = d.get("spec").cloned().unwrap_or_default();
                let status = d.get("status").cloned().unwrap_or_default();
                let containers: Vec<serde_json::Value> = spec.get("template").and_then(|t| t.get("spec")).and_then(|s| s.get("containers")).and_then(|c| c.as_array()).cloned().unwrap_or_default();
                let image = containers.first().and_then(|c| c.get("image")).and_then(|x| x.as_str()).unwrap_or("").to_string();
                K8sDeployment {
                    name:      meta.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    namespace: meta.get("namespace").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    replicas:  spec.get("replicas").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                    ready:     status.get("readyReplicas").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                    updated:   status.get("updatedReplicas").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                    available: status.get("availableReplicas").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
                    age: String::new(), image,
                }
            }).collect();
            Ok(Json(deployments))
        }
        _ => Ok(Json(vec![]))
    }
}

pub async fn performance_history(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<K8sHistoryPoint>>, ApiError> {
    if !instances().read().iter().any(|i| i.id == id) {
        return Err(ApiError::from((StatusCode::NOT_FOUND, "Instance not found".into())));
    }
    let rows = sqlx::query(
        r#"SELECT time_bucket('1 minute', collected_at) AS bucket,
           avg(CASE WHEN metric_name = 'pods_running' THEN value END) AS pods_running
           FROM middleware_metrics
           WHERE instance_id = $1 AND instance_type = 'kubernetes'
             AND collected_at > now() - INTERVAL '1 hour'
           GROUP BY bucket ORDER BY bucket"#
    ).bind(&id).fetch_all(&s.pool).await.unwrap_or_default();
    let points: Vec<K8sHistoryPoint> = rows.iter().map(|r| {
        use sqlx::Row;
        let bucket: chrono::DateTime<Utc> = r.get("bucket");
        K8sHistoryPoint {
            timestamp: bucket.to_rfc3339(),
            pods_running: r.get::<Option<f64>, _>("pods_running").unwrap_or(0.0) as u32,
            cpu_pct: 0.0, memory_pct: 0.0, pod_restarts: 0,
        }
    }).collect();
    Ok(Json(points))
}

pub async fn create_instance(State(s): State<AppState>, Json(req): Json<CreateK8sRequest>) -> Result<Json<K8sInstance>, ApiError> {
    let inst = K8sInstance { id: crate::error::short_id("k8s"), name: req.name, host: req.host, port: req.port.unwrap_or(6443), status: "unknown".into(), created_at: Utc::now().to_rfc3339() };
    let _ = sqlx::query(
        "INSERT INTO middleware_instances (id, instance_type, name, host, port) VALUES ($1, 'kubernetes', $2, $3, $4) ON CONFLICT (id) DO NOTHING"
    ).bind(&inst.id).bind(&inst.name).bind(&inst.host).bind(inst.port as i32).execute(&s.pool).await;
    instances().write().push(inst.clone()); Ok(Json(inst))
}
pub async fn delete_instance(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let _ = sqlx::query("DELETE FROM middleware_instances WHERE id = $1 AND instance_type = 'kubernetes'").bind(&id).execute(&s.pool).await;
    let mut insts = instances().write(); let b = insts.len(); insts.retain(|i| i.id != id);
    if insts.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
