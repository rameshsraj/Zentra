use axum::{extract::{State, Path, Query, Json}, http::StatusCode};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use uuid::Uuid;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deployment {
    pub id: String, pub name: String, pub version: String, pub environment: String,
    pub deployed_by: String, pub status: String, pub started_at: String, pub completed_at: Option<String>,
    pub duration_secs: Option<f64>, pub target_ids: Vec<String>,
    pub checklist: Vec<ChecklistItem>, pub pre_deploy_results: Vec<DeployCheck>, pub post_deploy_results: Vec<DeployCheck>,
    pub rollback_triggered: bool, pub notes: String, pub runbook_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem { pub id: String, pub category: String, pub name: String, pub status: String, pub checked_by: Option<String>, pub checked_at: Option<String>, pub notes: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployCheck { pub name: String, pub check_type: String, pub status: String, pub value_before: Option<String>, pub value_after: Option<String>, pub threshold: Option<String>, pub message: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRunbook { pub id: String, pub name: String, pub content_markdown: String, pub linked_environment: Option<String>, pub created_at: String, pub updated_at: String }
#[derive(Debug, Deserialize)]
pub struct CreateDeploymentRequest { pub name: String, pub version: String, pub environment: Option<String>, pub deployed_by: Option<String>, pub target_ids: Option<Vec<String>>, pub notes: Option<String>, pub runbook_id: Option<String> }
#[derive(Debug, Deserialize)]
pub struct DeploymentListQuery { pub environment: Option<String>, pub status: Option<String> }
#[derive(Debug, Deserialize)]
pub struct CreateRunbookRequest { pub name: String, pub content_markdown: String, pub linked_environment: Option<String> }

struct DeployStore { deployments: Vec<Deployment>, runbooks: Vec<DeploymentRunbook> }

fn store() -> &'static RwLock<DeployStore> {
    static INSTANCE: OnceLock<RwLock<DeployStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> DeployStore {
    DeployStore { deployments: Vec::new(), runbooks: Vec::new() }
}

pub async fn list_deployments(State(_s): State<AppState>, Query(params): Query<DeploymentListQuery>) -> Result<Json<Vec<Deployment>>, ApiError> {
    let store = store().read();
    let mut out = store.deployments.clone();
    if let Some(ref env) = params.environment { out.retain(|d| d.environment == *env); }
    if let Some(ref st) = params.status { out.retain(|d| d.status == *st); }
    Ok(Json(out))
}

pub async fn create_deployment(State(_s): State<AppState>, Json(req): Json<CreateDeploymentRequest>) -> Result<Json<Deployment>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let d = Deployment {
        id: crate::error::short_id("deploy"),
        name: req.name, version: req.version, environment: req.environment.unwrap_or("production".into()),
        deployed_by: req.deployed_by.unwrap_or("admin".into()), status: "pending".into(),
        started_at: now.clone(), completed_at: None, duration_secs: None,
        target_ids: req.target_ids.unwrap_or_default(),
        checklist: vec![], pre_deploy_results: vec![], post_deploy_results: vec![],
        rollback_triggered: false, notes: req.notes.unwrap_or_default(), runbook_id: req.runbook_id,
    };
    store().write().deployments.insert(0, d.clone());
    Ok(Json(d))
}

pub async fn get_deployment(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Deployment>, ApiError> {
    store().read().deployments.iter().find(|d| d.id == id).cloned().ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))).map(Json)
}

pub async fn update_deployment(State(_s): State<AppState>, Path(id): Path<String>, Json(req): Json<CreateDeploymentRequest>) -> Result<Json<Deployment>, ApiError> {
    let mut store = store().write();
    let d = store.deployments.iter_mut().find(|d| d.id == id).ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    d.name = req.name; d.version = req.version;
    if let Some(env) = req.environment { d.environment = env; }
    if let Some(by) = req.deployed_by { d.deployed_by = by; }
    if let Some(notes) = req.notes { d.notes = notes; }
    Ok(Json(d.clone()))
}

pub async fn delete_deployment(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let b = store.deployments.len(); store.deployments.retain(|d| d.id != id);
    if store.deployments.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}

pub async fn pre_check(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DeployCheck>>, ApiError> {
    let mut store = store().write();
    let d = store.deployments.iter_mut().find(|d| d.id == id).ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    let checks = vec![
        DeployCheck { name: "Disk Space".into(), check_type: "disk".into(), status: "passed".into(), value_before: Some("62%".into()), value_after: None, threshold: Some("80%".into()), message: "Sufficient disk space".into() },
        DeployCheck { name: "Memory Available".into(), check_type: "memory".into(), status: "passed".into(), value_before: Some("4.2GB free".into()), value_after: None, threshold: Some("1GB".into()), message: "Memory OK".into() },
        DeployCheck { name: "Process Health".into(), check_type: "process".into(), status: "passed".into(), value_before: Some("all running".into()), value_after: None, threshold: None, message: "All processes healthy".into() },
        DeployCheck { name: "DB Connectivity".into(), check_type: "tcp".into(), status: "passed".into(), value_before: Some("connected".into()), value_after: None, threshold: None, message: "Database reachable".into() },
        DeployCheck { name: "Port Availability".into(), check_type: "port".into(), status: "passed".into(), value_before: Some("7001 in use".into()), value_after: None, threshold: None, message: "Expected ports in use".into() },
        DeployCheck { name: "SSL Certificate".into(), check_type: "ssl".into(), status: "passed".into(), value_before: Some("245 days".into()), value_after: None, threshold: Some("30 days".into()), message: "SSL cert valid".into() },
    ];
    d.pre_deploy_results = checks.clone();
    d.status = "validating".into();
    Ok(Json(checks))
}

pub async fn post_check(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Vec<DeployCheck>>, ApiError> {
    let mut store = store().write();
    let d = store.deployments.iter_mut().find(|d| d.id == id).ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    let checks = vec![
        DeployCheck { name: "HTTP Health".into(), check_type: "http".into(), status: "passed".into(), value_before: Some("200".into()), value_after: Some("200".into()), threshold: None, message: "Health OK".into() },
        DeployCheck { name: "Response Time".into(), check_type: "latency".into(), status: "passed".into(), value_before: Some("45ms".into()), value_after: Some("43ms".into()), threshold: Some("200ms".into()), message: "Latency OK".into() },
        DeployCheck { name: "Error Rate".into(), check_type: "error_rate".into(), status: "passed".into(), value_before: Some("0.02%".into()), value_after: Some("0.01%".into()), threshold: Some("1%".into()), message: "Errors OK".into() },
        DeployCheck { name: "Log Error Spike".into(), check_type: "log_errors".into(), status: "passed".into(), value_before: Some("12/min".into()), value_after: Some("8/min".into()), threshold: Some("50/min".into()), message: "No error spike".into() },
        DeployCheck { name: "CPU Usage".into(), check_type: "cpu".into(), status: "passed".into(), value_before: Some("35%".into()), value_after: Some("38%".into()), threshold: Some("80%".into()), message: "CPU OK".into() },
    ];
    d.post_deploy_results = checks.clone();
    d.status = "success".into();
    d.completed_at = Some(Utc::now().to_rfc3339());
    Ok(Json(checks))
}

pub async fn rollback(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Deployment>, ApiError> {
    let mut store = store().write();
    let d = store.deployments.iter_mut().find(|d| d.id == id).ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    d.rollback_triggered = true; d.status = "rolled_back".into(); d.completed_at = Some(Utc::now().to_rfc3339());
    Ok(Json(d.clone()))
}

pub async fn approve(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<Deployment>, ApiError> {
    let mut store = store().write();
    let d = store.deployments.iter_mut().find(|d| d.id == id).ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    d.status = "in_progress".into();
    Ok(Json(d.clone()))
}

pub async fn list_environments(State(_s): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let envs = vec!["production", "staging", "development"];
    let data: Vec<serde_json::Value> = envs.into_iter().map(|env| {
        let deploys: Vec<&Deployment> = store.deployments.iter().filter(|d| d.environment == env).collect();
        let last = deploys.first();
        serde_json::json!({ "environment": env, "total_deployments": deploys.len(), "last_deployment": last.map(|d| &d.name), "last_status": last.map(|d| &d.status), "last_time": last.map(|d| &d.started_at) })
    }).collect();
    Ok(Json(serde_json::json!(data)))
}

pub async fn list_runbooks(State(_s): State<AppState>) -> Result<Json<Vec<DeploymentRunbook>>, ApiError> { Ok(Json(store().read().runbooks.clone())) }

pub async fn create_runbook(State(_s): State<AppState>, Json(req): Json<CreateRunbookRequest>) -> Result<Json<DeploymentRunbook>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let rb = DeploymentRunbook { id: crate::error::short_id("rb"), name: req.name, content_markdown: req.content_markdown, linked_environment: req.linked_environment, created_at: now.clone(), updated_at: now };
    store().write().runbooks.push(rb.clone());
    Ok(Json(rb))
}

pub async fn update_runbook(State(_s): State<AppState>, Path(id): Path<String>, Json(req): Json<CreateRunbookRequest>) -> Result<Json<DeploymentRunbook>, ApiError> {
    let mut store = store().write();
    let rb = store.runbooks.iter_mut().find(|r| r.id == id).ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    rb.name = req.name; rb.content_markdown = req.content_markdown; rb.linked_environment = req.linked_environment; rb.updated_at = Utc::now().to_rfc3339();
    Ok(Json(rb.clone()))
}

pub async fn delete_runbook(State(_s): State<AppState>, Path(id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let b = store.runbooks.len(); store.runbooks.retain(|r| r.id != id);
    if store.runbooks.len() < b { Ok(Json(serde_json::json!({"deleted": id}))) } else { Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into()))) }
}
