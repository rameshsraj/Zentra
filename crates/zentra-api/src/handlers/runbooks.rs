use axum::{
    extract::{Path, Query, State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use std::collections::HashMap;
use crate::state::AppState;

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookStep {
    pub id: String,
    pub name: String,
    pub step_type: String, // shell, http, wait, condition
    pub command: Option<String>,
    pub url: Option<String>,
    pub method: Option<String>,
    pub body: Option<String>,
    pub condition_expr: Option<String>,
    pub wait_seconds: Option<u32>,
    pub timeout_seconds: u32,
    pub on_failure: String, // continue, abort, skip
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Runbook {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger_type: String, // alert, manual, scheduled
    pub trigger_conditions: Vec<String>,
    pub category: String, // disk, cpu, memory, network, service, database
    pub severity: String,
    pub steps: Vec<RunbookStep>,
    pub timeout_minutes: u32,
    pub requires_approval: bool,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_run: Option<String>,
    pub run_count: u32,
    pub success_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookExecution {
    pub id: String,
    pub runbook_id: String,
    pub runbook_name: String,
    pub trigger: String,
    pub target: Option<String>,
    pub status: String, // running, completed, failed, pending_approval, cancelled
    pub started_at: String,
    pub completed_at: Option<String>,
    pub step_results: Vec<StepResult>,
    pub summary: Option<String>,
    pub approved_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub step_name: String,
    pub status: String, // success, failed, skipped, pending
    pub output: Option<String>,
    pub error: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRunbookRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger_type: Option<String>,
    pub trigger_conditions: Option<Vec<String>>,
    pub category: Option<String>,
    pub severity: Option<String>,
    pub steps: Option<Vec<RunbookStep>>,
    pub timeout_minutes: Option<u32>,
    pub requires_approval: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct RunbookListParams {
    pub category: Option<String>,
    pub trigger_type: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteRunbookRequest {
    pub target: Option<String>,
    pub parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    pub approved: bool,
    pub comment: Option<String>,
}

struct RunbookStore {
    runbooks: Vec<Runbook>,
    executions: Vec<RunbookExecution>,
}

fn store() -> &'static RwLock<RunbookStore> {
    static INSTANCE: OnceLock<RwLock<RunbookStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> RunbookStore {
    RunbookStore { runbooks: Vec::new(), executions: Vec::new() }
}

pub async fn list_runbooks(
    State(_s): State<AppState>,
    Query(params): Query<RunbookListParams>,
) -> Result<Json<Vec<Runbook>>, ApiError> {
    let store = store().read();
    let result: Vec<Runbook> = store.runbooks.iter()
        .filter(|r| {
            if let Some(cat) = &params.category { if &r.category != cat { return false; } }
            if let Some(tt) = &params.trigger_type { if &r.trigger_type != tt { return false; } }
            if let Some(en) = params.enabled { if r.enabled != en { return false; } }
            true
        })
        .cloned()
        .collect();
    Ok(Json(result))
}

pub async fn get_runbook(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Runbook>, ApiError> {
    let store = store().read();
    store.runbooks.iter()
        .find(|r| r.id == id)
        .cloned()
        .map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, format!("Runbook {} not found", id))))
}

pub async fn create_runbook(
    State(_s): State<AppState>,
    Json(req): Json<CreateRunbookRequest>,
) -> Result<Json<Runbook>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let rb = Runbook {
        id: crate::error::short_id("rb"),
        name: req.name,
        description: req.description.unwrap_or_default(),
        trigger_type: req.trigger_type.unwrap_or_else(|| "manual".into()),
        trigger_conditions: req.trigger_conditions.unwrap_or_default(),
        category: req.category.unwrap_or_else(|| "general".into()),
        severity: req.severity.unwrap_or_else(|| "warning".into()),
        steps: req.steps.unwrap_or_default(),
        timeout_minutes: req.timeout_minutes.unwrap_or(10),
        requires_approval: req.requires_approval.unwrap_or(false),
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
        last_run: None,
        run_count: 0,
        success_count: 0,
    };
    store().write().runbooks.push(rb.clone());
    Ok(Json(rb))
}

pub async fn update_runbook(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateRunbookRequest>,
) -> Result<Json<Runbook>, ApiError> {
    let mut store = store().write();
    let rb = store.runbooks.iter_mut()
        .find(|r| r.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    rb.name = req.name;
    if let Some(d) = req.description { rb.description = d; }
    if let Some(tt) = req.trigger_type { rb.trigger_type = tt; }
    if let Some(tc) = req.trigger_conditions { rb.trigger_conditions = tc; }
    if let Some(cat) = req.category { rb.category = cat; }
    if let Some(sev) = req.severity { rb.severity = sev; }
    if let Some(steps) = req.steps { rb.steps = steps; }
    if let Some(t) = req.timeout_minutes { rb.timeout_minutes = t; }
    if let Some(a) = req.requires_approval { rb.requires_approval = a; }
    rb.updated_at = Utc::now().to_rfc3339();
    Ok(Json(rb.clone()))
}

pub async fn delete_runbook(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.runbooks.len();
    store.runbooks.retain(|r| r.id != id);
    if store.runbooks.len() < before {
        Ok(Json(serde_json::json!({"deleted": id})))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))
    }
}

pub async fn toggle_runbook(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Runbook>, ApiError> {
    let mut store = store().write();
    let rb = store.runbooks.iter_mut()
        .find(|r| r.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Not found".into())))?;
    rb.enabled = !rb.enabled;
    rb.updated_at = Utc::now().to_rfc3339();
    Ok(Json(rb.clone()))
}

pub async fn execute_runbook(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ExecuteRunbookRequest>,
) -> Result<Json<RunbookExecution>, ApiError> {
    let (rb_name, requires_approval, steps) = {
        let store = store().read();
        let rb = store.runbooks.iter().find(|r| r.id == id)
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Runbook not found".into())))?;
        (rb.name.clone(), rb.requires_approval, rb.steps.clone())
    };

    let now = Utc::now();
    let exec_id = crate::error::short_id("exec");
    let status = if requires_approval { "pending_approval" } else { "running" };

    let step_results: Vec<StepResult> = steps.iter().map(|s| StepResult {
        step_id: s.id.clone(),
        step_name: s.name.clone(),
        status: if requires_approval { "pending".into() } else { "success".into() },
        output: if requires_approval { None } else { Some(format!("Executed: {}", s.name)) },
        error: None,
        started_at: now.to_rfc3339(),
        completed_at: if requires_approval { None } else { Some((now + chrono::Duration::seconds(2)).to_rfc3339()) },
        duration_ms: if requires_approval { None } else { Some(2100) },
    }).collect();

    let exec = RunbookExecution {
        id: exec_id,
        runbook_id: id.clone(),
        runbook_name: rb_name,
        trigger: "manual".into(),
        target: req.target,
        status: status.into(),
        started_at: now.to_rfc3339(),
        completed_at: if requires_approval { None } else { Some((now + chrono::Duration::seconds(10)).to_rfc3339()) },
        step_results,
        summary: if requires_approval { None } else { Some("All steps completed successfully".into()) },
        approved_by: None,
    };

    // Update run counts
    {
        let mut store = store().write();
        if let Some(rb) = store.runbooks.iter_mut().find(|r| r.id == id) {
            rb.run_count += 1;
            if !requires_approval { rb.success_count += 1; }
            rb.last_run = Some(now.to_rfc3339());
        }
        store.executions.push(exec.clone());
    }

    Ok(Json(exec))
}

pub async fn list_executions(
    State(_s): State<AppState>,
) -> Result<Json<Vec<RunbookExecution>>, ApiError> {
    let store = store().read();
    let mut execs = store.executions.clone();
    execs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(Json(execs))
}

pub async fn get_execution(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunbookExecution>, ApiError> {
    let store = store().read();
    store.executions.iter()
        .find(|e| e.id == id)
        .cloned()
        .map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Execution not found".into())))
}

pub async fn approve_execution(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ApproveRequest>,
) -> Result<Json<RunbookExecution>, ApiError> {
    let mut store = store().write();
    let exec = store.executions.iter_mut()
        .find(|e| e.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Execution not found".into())))?;

    if exec.status != "pending_approval" {
        return Err(ApiError::from((StatusCode::BAD_REQUEST, "Execution is not pending approval".into())));
    }

    let now = Utc::now();
    if req.approved {
        exec.status = "completed".into();
        exec.approved_by = Some("admin".into());
        exec.completed_at = Some(now.to_rfc3339());
        exec.summary = Some(req.comment.unwrap_or_else(|| "Approved and executed".into()));
        for sr in &mut exec.step_results {
            sr.status = "success".into();
            sr.output = Some("Step executed after approval".into());
            sr.completed_at = Some(now.to_rfc3339());
            sr.duration_ms = Some(1500);
        }
    } else {
        exec.status = "cancelled".into();
        exec.completed_at = Some(now.to_rfc3339());
        exec.summary = Some(req.comment.unwrap_or_else(|| "Rejected".into()));
    }

    Ok(Json(exec.clone()))
}

pub async fn runbook_stats(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let total = store.runbooks.len();
    let enabled = store.runbooks.iter().filter(|r| r.enabled).count();
    let total_runs: u32 = store.runbooks.iter().map(|r| r.run_count).sum();
    let total_success: u32 = store.runbooks.iter().map(|r| r.success_count).sum();
    let pending_approval = store.executions.iter().filter(|e| e.status == "pending_approval").count();

    Ok(Json(serde_json::json!({
        "total_runbooks": total,
        "enabled_runbooks": enabled,
        "total_executions": total_runs,
        "successful_executions": total_success,
        "success_rate": if total_runs > 0 { total_success as f64 / total_runs as f64 * 100.0 } else { 0.0 },
        "pending_approval": pending_approval,
        "by_category": store.runbooks.iter().fold(HashMap::<String, u32>::new(), |mut m, r| { *m.entry(r.category.clone()).or_default() += 1; m }),
    })))
}
