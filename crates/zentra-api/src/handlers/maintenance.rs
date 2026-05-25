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
    let now = Utc::now();
    let windows = vec![
        MaintenanceWindow {
            id: "mw-001".into(),
            name: "Weekend DB Patching".into(),
            scope_type: "group".into(),
            scope_targets: vec!["databases".into()],
            schedule_type: "recurring".into(),
            start_time: (now + Duration::days(2)).to_rfc3339(),
            end_time: (now + Duration::days(2) + Duration::hours(4)).to_rfc3339(),
            recurrence: Some("0 2 * * 6".into()),
            suppress_alerts: true,
            created_by: "admin".into(),
            created_at: (now - Duration::days(7)).to_rfc3339(),
            is_active: false,
        },
        MaintenanceWindow {
            id: "mw-002".into(),
            name: "Network Switch Upgrade".into(),
            scope_type: "host".into(),
            scope_targets: vec!["10.88.11.1".into(), "10.88.11.2".into()],
            schedule_type: "once".into(),
            start_time: (now - Duration::hours(1)).to_rfc3339(),
            end_time: (now + Duration::hours(3)).to_rfc3339(),
            recurrence: None,
            suppress_alerts: true,
            created_by: "netops".into(),
            created_at: (now - Duration::days(1)).to_rfc3339(),
            is_active: true,
        },
        MaintenanceWindow {
            id: "mw-003".into(),
            name: "SSL Certificate Renewal".into(),
            scope_type: "service".into(),
            scope_targets: vec!["web-services".into()],
            schedule_type: "once".into(),
            start_time: (now + Duration::days(5)).to_rfc3339(),
            end_time: (now + Duration::days(5) + Duration::hours(1)).to_rfc3339(),
            recurrence: None,
            suppress_alerts: false,
            created_by: "security".into(),
            created_at: now.to_rfc3339(),
            is_active: false,
        },
        MaintenanceWindow {
            id: "mw-004".into(),
            name: "Monthly Server Reboot".into(),
            scope_type: "all".into(),
            scope_targets: vec![],
            schedule_type: "recurring".into(),
            start_time: (now - Duration::days(15)).to_rfc3339(),
            end_time: (now - Duration::days(15) + Duration::hours(2)).to_rfc3339(),
            recurrence: Some("0 3 1 * *".into()),
            suppress_alerts: true,
            created_by: "admin".into(),
            created_at: (now - Duration::days(30)).to_rfc3339(),
            is_active: false,
        },
    ];

    let mut sla_targets = std::collections::HashMap::new();
    sla_targets.insert("hc-http-google".into(), 99.9);
    sla_targets.insert("hc-http-api".into(), 99.95);
    sla_targets.insert("hc-tcp-ssh".into(), 99.99);
    sla_targets.insert("hc-ping-gw".into(), 99.99);
    sla_targets.insert("hc-dns-resolve".into(), 99.9);
    sla_targets.insert("hc-ssl-cert".into(), 99.5);

    MaintenanceStore { windows, sla_targets }
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
    let store = store().read();
    let checks = vec![
        ("hc-http-google", "Google HTTP Check", "google.com", 99.92, 99.88, 99.91, 99.90),
        ("hc-http-api", "API Health Endpoint", "10.88.11.162", 100.0, 99.97, 99.95, 99.94),
        ("hc-tcp-ssh", "SSH Port Check", "10.88.11.162", 100.0, 100.0, 99.99, 99.99),
        ("hc-tcp-pg", "PostgreSQL Port", "10.88.11.50", 100.0, 99.95, 99.93, 99.92),
        ("hc-tcp-mysql", "MySQL Port", "10.88.11.51", 100.0, 99.90, 99.88, 99.85),
        ("hc-ping-gw", "Gateway Ping", "10.88.11.1", 100.0, 100.0, 99.98, 99.97),
        ("hc-ping-dns", "DNS Server Ping", "8.8.8.8", 100.0, 99.99, 99.99, 99.99),
        ("hc-dns-resolve", "DNS Resolution", "8.8.8.8", 100.0, 99.95, 99.92, 99.90),
        ("hc-ssl-cert", "SSL Certificate Check", "google.com", 100.0, 100.0, 100.0, 100.0),
        ("hc-disk-root", "Root Disk Space", "10.88.11.162", 100.0, 99.80, 99.75, 99.70),
        ("hc-disk-data", "Data Disk Space", "10.88.11.162", 94.0, 96.5, 97.2, 97.8),
        ("hc-proc-nginx", "Nginx Process", "10.88.11.162", 100.0, 99.95, 99.90, 99.88),
        ("hc-load-avg", "Load Average", "10.88.11.162", 100.0, 99.85, 99.80, 99.75),
        ("hc-ntp-sync", "NTP Sync Check", "10.88.11.162", 100.0, 99.98, 99.95, 99.93),
        ("hc-snmp-uptime", "SNMP System Uptime", "10.88.11.1", 98.0, 97.5, 98.2, 98.5),
    ];

    let reports: Vec<SLAReport> = checks.into_iter().map(|(id, name, host, u24, u7, u30, u90)| {
        let target = store.sla_targets.get(id).copied().unwrap_or(99.9);
        let outage_count = if u30 < 99.9 { 3 } else if u30 < 99.99 { 1 } else { 0 };
        let downtime = (100.0 - u30) * 30.0 * 24.0 * 60.0 / 100.0;
        let longest = if outage_count > 0 { downtime / outage_count as f64 } else { 0.0 };

        let outages: Vec<OutageEntry> = if outage_count > 0 {
            (0..outage_count).map(|i| {
                let start = Utc::now() - Duration::days(10 * (i as i64 + 1));
                OutageEntry {
                    started_at: start.to_rfc3339(),
                    ended_at: Some((start + Duration::minutes(longest as i64)).to_rfc3339()),
                    duration_mins: (longest * 100.0).round() / 100.0,
                    reason: format!("Auto-detected: {} degradation", if i == 0 { "response time" } else { "connectivity" }),
                }
            }).collect()
        } else { vec![] };

        SLAReport {
            check_id: id.into(),
            check_name: name.into(),
            target_host: host.into(),
            sla_target_pct: target,
            uptime_24h: u24,
            uptime_7d: u7,
            uptime_30d: u30,
            uptime_90d: u90,
            total_outages_30d: outage_count,
            total_downtime_mins_30d: (downtime * 100.0).round() / 100.0,
            longest_outage_mins: (longest * 100.0).round() / 100.0,
            compliance: u30 >= target,
            outages,
        }
    }).collect();

    Ok(Json(reports))
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
