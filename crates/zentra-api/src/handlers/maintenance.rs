use axum::{
    extract::{State, Path, Query, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use uuid::Uuid;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

// ── Maintenance Windows ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceWindow {
    pub id: String,
    pub name: String,
    pub scope_type: String,
    pub scope_targets: Vec<String>,
    pub schedule_type: String,
    pub start_time: String,
    pub end_time: String,
    pub recurrence: Option<String>,
    pub suppress_alerts: bool,
    pub created_by: String,
    pub created_at: String,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateMaintenanceRequest {
    pub name: String,
    pub scope_type: Option<String>,
    pub scope_targets: Option<Vec<String>>,
    pub schedule_type: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub recurrence: Option<String>,
    pub suppress_alerts: Option<bool>,
    pub created_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MaintenanceListQuery {
    pub filter: Option<String>,
}

// ── SLA Reports ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAReport {
    pub check_id: String,
    pub check_name: String,
    pub target_host: String,
    pub sla_target_pct: f64,
    pub uptime_24h: f64,
    pub uptime_7d: f64,
    pub uptime_30d: f64,
    pub uptime_90d: f64,
    pub total_outages_30d: usize,
    pub total_downtime_mins_30d: f64,
    pub longest_outage_mins: f64,
    pub compliance: bool,
    pub outages: Vec<OutageEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutageEntry {
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_mins: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLATarget {
    pub check_id: String,
    pub target_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLASummary {
    pub total_checks: usize,
    pub compliant: usize,
    pub violated: usize,
    pub avg_uptime_30d: f64,
    pub worst_check: Option<String>,
    pub worst_uptime: f64,
}

// ── In-Memory Storage ────────────────────────────────────────────────

struct MaintenanceStore {
    windows: Vec<MaintenanceWindow>,
    sla_targets: std::collections::HashMap<String, f64>,
}

fn store() -> &'static RwLock<MaintenanceStore> {
    static INSTANCE: OnceLock<RwLock<MaintenanceStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> MaintenanceStore {
    MaintenanceStore {
        windows: vec![],
        sla_targets: std::collections::HashMap::new(),
    }
}

fn is_window_active(w: &MaintenanceWindow) -> bool {
    let now = Utc::now().to_rfc3339();
    w.start_time <= now && w.end_time >= now
}

// ── Maintenance API Handlers ─────────────────────────────────────────

pub async fn list_windows(
    State(_s): State<AppState>,
    Query(params): Query<MaintenanceListQuery>,
) -> Result<Json<Vec<MaintenanceWindow>>, ApiError> {
    let store = store().read();
    let now = Utc::now().to_rfc3339();
    let mut out: Vec<MaintenanceWindow> = store.windows.iter().map(|w| {
        let mut w = w.clone();
        w.is_active = is_window_active(&w);
        w
    }).collect();

    if let Some(ref f) = params.filter {
        match f.as_str() {
            "active" => out.retain(|w| w.is_active),
            "upcoming" => out.retain(|w| w.start_time > now),
            "past" => out.retain(|w| w.end_time < now),
            _ => {}
        }
    }
    Ok(Json(out))
}

pub async fn create_window(
    State(_s): State<AppState>,
    Json(req): Json<CreateMaintenanceRequest>,
) -> Result<Json<MaintenanceWindow>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let w = MaintenanceWindow {
        id: crate::error::short_id("mw"),
        name: req.name,
        scope_type: req.scope_type.unwrap_or("host".into()),
        scope_targets: req.scope_targets.unwrap_or_default(),
        schedule_type: req.schedule_type.unwrap_or("once".into()),
        start_time: req.start_time,
        end_time: req.end_time,
        recurrence: req.recurrence,
        suppress_alerts: req.suppress_alerts.unwrap_or(true),
        created_by: req.created_by.unwrap_or("admin".into()),
        created_at: now,
        is_active: false,
    };
    let mut store = store().write();
    store.windows.push(w.clone());
    Ok(Json(w))
}

pub async fn update_window(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateMaintenanceRequest>,
) -> Result<Json<MaintenanceWindow>, ApiError> {
    let mut store = store().write();
    let w = store.windows.iter_mut().find(|w| w.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Window not found".into())))?;
    w.name = req.name;
    if let Some(st) = req.scope_type { w.scope_type = st; }
    if let Some(targets) = req.scope_targets { w.scope_targets = targets; }
    if let Some(sched) = req.schedule_type { w.schedule_type = sched; }
    w.start_time = req.start_time;
    w.end_time = req.end_time;
    w.recurrence = req.recurrence;
    if let Some(sa) = req.suppress_alerts { w.suppress_alerts = sa; }
    if let Some(cb) = req.created_by { w.created_by = cb; }
    Ok(Json(w.clone()))
}

pub async fn delete_window(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.windows.len();
    store.windows.retain(|w| w.id != id);
    if store.windows.len() < before {
        Ok(Json(serde_json::json!({"deleted": id})))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Window not found".into())))
    }
}

pub async fn active_windows(
    State(_s): State<AppState>,
) -> Result<Json<Vec<MaintenanceWindow>>, ApiError> {
    let store = store().read();
    let active: Vec<MaintenanceWindow> = store.windows.iter()
        .map(|w| { let mut w = w.clone(); w.is_active = is_window_active(&w); w })
        .filter(|w| w.is_active)
        .collect();
    Ok(Json(active))
}

pub async fn check_target_maintenance(
    State(_s): State<AppState>,
    Path(target_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let in_maintenance = store.windows.iter().any(|w| {
        is_window_active(w) && (w.scope_type == "all" || w.scope_targets.contains(&target_id))
    });
    let matching: Vec<&MaintenanceWindow> = store.windows.iter()
        .filter(|w| is_window_active(w) && (w.scope_type == "all" || w.scope_targets.contains(&target_id)))
        .collect();
    Ok(Json(serde_json::json!({
        "target_id": target_id,
        "in_maintenance": in_maintenance,
        "windows": matching,
    })))
}

// ── SLA API Handlers ─────────────────────────────────────────────────

pub async fn sla_reports(
    State(_s): State<AppState>,
) -> Result<Json<Vec<SLAReport>>, ApiError> {
    Ok(Json(vec![]))
}

pub async fn sla_report_detail(
    State(_s): State<AppState>,
    Path(check_id): Path<String>,
) -> Result<Json<SLAReport>, ApiError> {
    let reports = sla_reports(State(_s)).await?;
    reports.0.into_iter().find(|r| r.check_id == check_id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Check not found".into())))
        .map(Json)
}

pub async fn set_sla_targets(
    State(_s): State<AppState>,
    Json(targets): Json<Vec<SLATarget>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    for t in &targets {
        store.sla_targets.insert(t.check_id.clone(), t.target_pct);
    }
    Ok(Json(serde_json::json!({"updated": targets.len()})))
}

pub async fn sla_summary(
    State(_s): State<AppState>,
) -> Result<Json<SLASummary>, ApiError> {
    let reports = sla_reports(State(_s)).await?;
    let all = &reports.0;
    let compliant = all.iter().filter(|r| r.compliance).count();
    let violated = all.len() - compliant;
    let avg = if all.is_empty() { 0.0 } else { all.iter().map(|r| r.uptime_30d).sum::<f64>() / all.len() as f64 };
    let worst = all.iter().min_by(|a, b| a.uptime_30d.partial_cmp(&b.uptime_30d).unwrap_or(std::cmp::Ordering::Equal));

    Ok(Json(SLASummary {
        total_checks: all.len(),
        compliant,
        violated,
        avg_uptime_30d: (avg * 100.0).round() / 100.0,
        worst_check: worst.map(|w| w.check_name.clone()),
        worst_uptime: worst.map(|w| w.uptime_30d).unwrap_or(100.0),
    }))
}
