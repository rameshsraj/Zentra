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

// ── Data Models ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub id: String,
    pub name: String,
    pub check_type: String,
    pub target_host: String,
    pub config: serde_json::Value,
    pub interval_secs: u64,
    pub timeout_secs: u64,
    pub warning_threshold: Option<String>,
    pub critical_threshold: Option<String>,
    pub enabled: bool,
    pub group: Option<String>,
    pub parent_check_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub id: String,
    pub check_id: String,
    pub status: String,
    pub response_time_ms: f64,
    pub output: String,
    pub details: serde_json::Value,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckWithStatus {
    pub check: HealthCheck,
    pub last_result: Option<HealthCheckResult>,
    pub uptime_24h: f64,
    pub uptime_7d: f64,
    pub uptime_30d: f64,
    pub avg_response_ms: f64,
    pub is_flapping: bool,
    pub state_since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub total_checks: usize,
    pub ok: usize,
    pub warning: usize,
    pub critical: usize,
    pub unknown: usize,
    pub checks: Vec<HealthCheckWithStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixRow {
    pub host: String,
    pub checks: Vec<MatrixCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixCell {
    pub check_id: String,
    pub check_name: String,
    pub check_type: String,
    pub status: String,
    pub response_time_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UptimeStat {
    pub check_id: String,
    pub check_name: String,
    pub target_host: String,
    pub uptime_24h: f64,
    pub uptime_7d: f64,
    pub uptime_30d: f64,
    pub avg_response_ms: f64,
    pub total_checks_24h: usize,
    pub failed_checks_24h: usize,
}

#[derive(Debug, Deserialize)]
pub struct CreateCheckRequest {
    pub name: String,
    pub check_type: String,
    pub target_host: String,
    pub config: Option<serde_json::Value>,
    pub interval_secs: Option<u64>,
    pub timeout_secs: Option<u64>,
    pub warning_threshold: Option<String>,
    pub critical_threshold: Option<String>,
    pub enabled: Option<bool>,
    pub group: Option<String>,
    pub parent_check_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BulkCreateRequest {
    pub checks: Vec<CreateCheckRequest>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct CheckListQuery {
    pub group: Option<String>,
    pub check_type: Option<String>,
    pub status: Option<String>,
    pub enabled: Option<bool>,
}

// ── In-Memory Storage ────────────────────────────────────────────────

struct CheckStore {
    checks: Vec<HealthCheck>,
    results: std::collections::HashMap<String, Vec<HealthCheckResult>>,
}

fn store() -> &'static RwLock<CheckStore> {
    static INSTANCE: OnceLock<RwLock<CheckStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

/// Return a snapshot of all checks and their latest result for metric persistence.
pub fn snapshot_checks() -> Vec<(HealthCheck, Option<HealthCheckResult>)> {
    let store = store().read();
    store.checks.iter().map(|c| {
        let latest = store.results.get(&c.id).and_then(|v| v.first().cloned());
        (c.clone(), latest)
    }).collect()
}

fn init_store() -> CheckStore {
    CheckStore {
        checks: Vec::new(),
        results: std::collections::HashMap::new(),
    }
}

// ── Helper: compute uptime ───────────────────────────────────────────

fn compute_uptime(results: &[HealthCheckResult], hours: i64) -> f64 {
    let cutoff = (Utc::now() - Duration::hours(hours)).to_rfc3339();
    let relevant: Vec<&HealthCheckResult> = results.iter()
        .filter(|r| r.checked_at >= cutoff)
        .collect();
    if relevant.is_empty() { return 100.0; }
    let ok_count = relevant.iter().filter(|r| r.status == "ok").count();
    (ok_count as f64 / relevant.len() as f64) * 100.0
}

fn avg_response(results: &[HealthCheckResult], hours: i64) -> f64 {
    let cutoff = (Utc::now() - Duration::hours(hours)).to_rfc3339();
    let relevant: Vec<&HealthCheckResult> = results.iter()
        .filter(|r| r.checked_at >= cutoff)
        .collect();
    if relevant.is_empty() { return 0.0; }
    let sum: f64 = relevant.iter().map(|r| r.response_time_ms).sum();
    (sum / relevant.len() as f64 * 100.0).round() / 100.0
}

fn detect_flapping(results: &[HealthCheckResult]) -> bool {
    if results.len() < 10 { return false; }
    let recent = &results[..10.min(results.len())];
    let mut transitions = 0;
    for w in recent.windows(2) {
        if w[0].status != w[1].status { transitions += 1; }
    }
    transitions >= 4
}

fn build_check_with_status(check: &HealthCheck, results: &[HealthCheckResult]) -> HealthCheckWithStatus {
    let last_result = results.first().cloned();
    let state_since = results.first()
        .map(|r| {
            // walk back to find when current state started
            let current = &r.status;
            let mut since = &r.checked_at;
            for res in results.iter() {
                if &res.status == current { since = &res.checked_at; } else { break; }
            }
            since.clone()
        })
        .unwrap_or_else(|| check.created_at.clone());

    HealthCheckWithStatus {
        check: check.clone(),
        last_result,
        uptime_24h: (compute_uptime(results, 24) * 100.0).round() / 100.0,
        uptime_7d: (compute_uptime(results, 168) * 100.0).round() / 100.0,
        uptime_30d: (compute_uptime(results, 720) * 100.0).round() / 100.0,
        avg_response_ms: avg_response(results, 24),
        is_flapping: detect_flapping(results),
        state_since,
    }
}

// ── API Handlers ─────────────────────────────────────────────────────

/// GET /api/healthchecks — list all checks with latest status
pub async fn list_checks(
    State(_s): State<AppState>,
    Query(params): Query<CheckListQuery>,
) -> Result<Json<Vec<HealthCheckWithStatus>>, ApiError> {
    let store = store().read();
    let mut out: Vec<HealthCheckWithStatus> = Vec::new();

    for check in &store.checks {
        if let Some(ref g) = params.group {
            if check.group.as_deref() != Some(g.as_str()) { continue; }
        }
        if let Some(ref ct) = params.check_type {
            if &check.check_type != ct { continue; }
        }
        if let Some(en) = params.enabled {
            if check.enabled != en { continue; }
        }

        let results = store.results.get(&check.id).map(|v| v.as_slice()).unwrap_or(&[]);
        let cws = build_check_with_status(check, results);

        if let Some(ref st) = params.status {
            let current_status = cws.last_result.as_ref().map(|r| r.status.as_str()).unwrap_or("unknown");
            if current_status != st.as_str() { continue; }
        }

        out.push(cws);
    }
    Ok(Json(out))
}

/// POST /api/healthchecks — create check
pub async fn create_check(
    State(_s): State<AppState>,
    Json(req): Json<CreateCheckRequest>,
) -> Result<Json<HealthCheck>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let id = crate::error::short_id("hc");
    let check = HealthCheck {
        id: id.clone(),
        name: req.name,
        check_type: req.check_type,
        target_host: req.target_host,
        config: req.config.unwrap_or(serde_json::json!({})),
        interval_secs: req.interval_secs.unwrap_or(60),
        timeout_secs: req.timeout_secs.unwrap_or(10),
        warning_threshold: req.warning_threshold,
        critical_threshold: req.critical_threshold,
        enabled: req.enabled.unwrap_or(true),
        group: req.group,
        parent_check_id: req.parent_check_id,
        created_at: now.clone(),
        updated_at: now,
    };
    let mut store = store().write();
    store.checks.push(check.clone());
    store.results.insert(id, Vec::new());
    Ok(Json(check))
}

/// GET /api/healthchecks/summary — summary counts + overall status
pub async fn get_summary(
    State(_s): State<AppState>,
) -> Result<Json<HealthCheckSummary>, ApiError> {
    let store = store().read();
    let mut checks_with_status = Vec::new();
    let (mut ok, mut warning, mut critical, mut unknown) = (0, 0, 0, 0);

    for check in &store.checks {
        let results = store.results.get(&check.id).map(|v| v.as_slice()).unwrap_or(&[]);
        let cws = build_check_with_status(check, results);
        let status = cws.last_result.as_ref().map(|r| r.status.as_str()).unwrap_or("unknown");
        match status {
            "ok" => ok += 1,
            "warning" => warning += 1,
            "critical" => critical += 1,
            _ => unknown += 1,
        }
        checks_with_status.push(cws);
    }

    Ok(Json(HealthCheckSummary {
        total_checks: store.checks.len(),
        ok, warning, critical, unknown,
        checks: checks_with_status,
    }))
}

/// GET /api/healthchecks/matrix — host × check matrix view data
pub async fn get_matrix(
    State(_s): State<AppState>,
) -> Result<Json<Vec<MatrixRow>>, ApiError> {
    let store = store().read();
    let mut host_map: std::collections::BTreeMap<String, Vec<MatrixCell>> = std::collections::BTreeMap::new();

    for check in &store.checks {
        let results = store.results.get(&check.id).map(|v| v.as_slice()).unwrap_or(&[]);
        let (status, rt) = results.first()
            .map(|r| (r.status.clone(), r.response_time_ms))
            .unwrap_or(("unknown".to_string(), 0.0));

        host_map.entry(check.target_host.clone()).or_default().push(MatrixCell {
            check_id: check.id.clone(),
            check_name: check.name.clone(),
            check_type: check.check_type.clone(),
            status,
            response_time_ms: rt,
        });
    }

    let matrix: Vec<MatrixRow> = host_map.into_iter()
        .map(|(host, checks)| MatrixRow { host, checks })
        .collect();
    Ok(Json(matrix))
}

/// GET /api/healthchecks/groups — list unique groups
pub async fn list_groups(
    State(_s): State<AppState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let store = store().read();
    let mut groups: Vec<String> = store.checks.iter()
        .filter_map(|c| c.group.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    groups.sort();
    Ok(Json(groups))
}

/// GET /api/healthchecks/uptime — uptime statistics per check
pub async fn get_uptime(
    State(_s): State<AppState>,
) -> Result<Json<Vec<UptimeStat>>, ApiError> {
    let store = store().read();
    let mut stats = Vec::new();
    for check in &store.checks {
        let results = store.results.get(&check.id).map(|v| v.as_slice()).unwrap_or(&[]);
        let cutoff_24h = (Utc::now() - Duration::hours(24)).to_rfc3339();
        let recent: Vec<&HealthCheckResult> = results.iter().filter(|r| r.checked_at >= cutoff_24h).collect();
        let failed = recent.iter().filter(|r| r.status != "ok").count();

        stats.push(UptimeStat {
            check_id: check.id.clone(),
            check_name: check.name.clone(),
            target_host: check.target_host.clone(),
            uptime_24h: compute_uptime(results, 24),
            uptime_7d: compute_uptime(results, 168),
            uptime_30d: compute_uptime(results, 720),
            avg_response_ms: avg_response(results, 24),
            total_checks_24h: recent.len(),
            failed_checks_24h: failed,
        });
    }
    Ok(Json(stats))
}

/// GET /api/healthchecks/:id — get check detail + last 100 results
pub async fn get_check(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let check = store.checks.iter().find(|c| c.id == id)
        .ok_or((StatusCode::NOT_FOUND, "Check not found".into()))?;
    let results = store.results.get(&id).map(|v| v.as_slice()).unwrap_or(&[]);
    let cws = build_check_with_status(check, results);
    let history: Vec<&HealthCheckResult> = results.iter().take(100).collect();

    Ok(Json(serde_json::json!({
        "check": cws,
        "history": history,
    })))
}

/// PUT /api/healthchecks/:id — update check
pub async fn update_check(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CreateCheckRequest>,
) -> Result<Json<HealthCheck>, ApiError> {
    let mut store = store().write();
    let check = store.checks.iter_mut().find(|c| c.id == id)
        .ok_or((StatusCode::NOT_FOUND, "Check not found".into()))?;

    check.name = req.name;
    check.check_type = req.check_type;
    check.target_host = req.target_host;
    if let Some(config) = req.config { check.config = config; }
    if let Some(interval) = req.interval_secs { check.interval_secs = interval; }
    if let Some(timeout) = req.timeout_secs { check.timeout_secs = timeout; }
    check.warning_threshold = req.warning_threshold;
    check.critical_threshold = req.critical_threshold;
    if let Some(enabled) = req.enabled { check.enabled = enabled; }
    check.group = req.group;
    check.parent_check_id = req.parent_check_id;
    check.updated_at = Utc::now().to_rfc3339();

    Ok(Json(check.clone()))
}

/// DELETE /api/healthchecks/:id — delete check
pub async fn delete_check(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.checks.len();
    store.checks.retain(|c| c.id != id);
    store.results.remove(&id);
    if store.checks.len() < before {
        Ok(Json(serde_json::json!({"deleted": id})))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Check not found".into())))
    }
}

/// POST /api/healthchecks/:id/run — execute check immediately (simulate)
pub async fn run_check(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<HealthCheckResult>, ApiError> {
    let mut store = store().write();
    let check = store.checks.iter().find(|c| c.id == id)
        .ok_or((StatusCode::NOT_FOUND, "Check not found".into()))?.clone();

    // Simulate check execution based on type
    let now = Utc::now().to_rfc3339();
    let (status, rt, output, details) = simulate_check(&check);

    let result = HealthCheckResult {
        id: crate::error::short_id(&id),
        check_id: id.clone(),
        status,
        response_time_ms: rt,
        output,
        details,
        checked_at: now,
    };

    // Prepend to results, keep max 100
    let entry = store.results.entry(id).or_insert_with(Vec::new);
    entry.insert(0, result.clone());
    if entry.len() > 100 { entry.truncate(100); }

    Ok(Json(result))
}

/// GET /api/healthchecks/:id/history — paginated results history
pub async fn get_history(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let results = store.results.get(&id)
        .ok_or((StatusCode::NOT_FOUND, "Check not found".into()))?;

    let filtered: Vec<&HealthCheckResult> = results.iter()
        .filter(|r| params.status.as_ref().map(|s| s == &r.status).unwrap_or(true))
        .collect();

    let total = filtered.len();
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50);
    let page: Vec<&&HealthCheckResult> = filtered.iter().skip(offset).take(limit).collect();

    Ok(Json(serde_json::json!({
        "total": total,
        "offset": offset,
        "limit": limit,
        "results": page,
    })))
}

/// POST /api/healthchecks/bulk — create multiple checks at once
pub async fn bulk_create(
    State(_s): State<AppState>,
    Json(req): Json<BulkCreateRequest>,
) -> Result<Json<Vec<HealthCheck>>, ApiError> {
    let now = Utc::now().to_rfc3339();
    let mut store = store().write();
    let mut created = Vec::new();

    for item in req.checks {
        let id = crate::error::short_id("hc");
        let check = HealthCheck {
            id: id.clone(),
            name: item.name,
            check_type: item.check_type,
            target_host: item.target_host,
            config: item.config.unwrap_or(serde_json::json!({})),
            interval_secs: item.interval_secs.unwrap_or(60),
            timeout_secs: item.timeout_secs.unwrap_or(10),
            warning_threshold: item.warning_threshold,
            critical_threshold: item.critical_threshold,
            enabled: item.enabled.unwrap_or(true),
            group: item.group,
            parent_check_id: item.parent_check_id,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        store.checks.push(check.clone());
        store.results.insert(id, Vec::new());
        created.push(check);
    }
    Ok(Json(created))
}

// ── Check Simulation (realistic results per type) ────────────────────

fn simulate_check(check: &HealthCheck) -> (String, f64, String, serde_json::Value) {
    let now_ms = Utc::now().timestamp_millis();
    // Use timestamp to create some variation
    let jitter = (now_ms % 100) as f64;

    match check.check_type.as_str() {
        "http" => {
            let url = check.config.get("url").and_then(|v| v.as_str()).unwrap_or("http://localhost");
            let rt = 50.0 + jitter * 2.0;
            let code = if jitter > 90.0 { 500 } else { 200 };
            let status = if code == 200 { "ok" } else { "critical" };
            (status.into(), rt, format!("HTTP {} returned {}", url, code),
             serde_json::json!({"status_code": code, "response_time_ms": rt, "url": url}))
        },
        "tcp" => {
            let port = check.config.get("port").and_then(|v| v.as_u64()).unwrap_or(80);
            let rt = 2.0 + jitter * 0.1;
            let open = jitter < 95.0;
            let status = if open { "ok" } else { "critical" };
            (status.into(), rt, format!("TCP port {} {}", port, if open {"open"} else {"closed"}),
             serde_json::json!({"port": port, "port_open": open, "connection_time_ms": rt}))
        },
        "ping" => {
            let loss = if jitter > 90.0 { 25.0 } else { 0.0 };
            let rtt = 1.0 + jitter * 0.5;
            let status = if loss > 20.0 { "critical" } else if loss > 0.0 { "warning" } else { "ok" };
            (status.into(), rtt, format!("Ping {}ms, {}% loss", rtt as u64, loss),
             serde_json::json!({"packets_sent": 4, "packets_received": if loss > 0.0 {3} else {4}, "loss_pct": loss, "rtt_avg": rtt}))
        },
        "dns" => {
            let domain = check.config.get("domain").and_then(|v| v.as_str()).unwrap_or("example.com");
            let rt = 10.0 + jitter;
            (String::from("ok"), rt, format!("DNS {} resolved in {}ms", domain, rt as u64),
             serde_json::json!({"domain": domain, "resolved": null, "resolution_time_ms": rt}))
        },
        "ssl_cert" => {
            let days = 245 - (now_ms % 30) as i64;
            let warn_days = check.config.get("warn_days").and_then(|v| v.as_i64()).unwrap_or(30);
            let crit_days = check.config.get("crit_days").and_then(|v| v.as_i64()).unwrap_or(7);
            let status = if days < crit_days { "critical" } else if days < warn_days { "warning" } else { "ok" };
            let rt = 200.0 + jitter * 3.0;
            (status.into(), rt, format!("SSL cert valid, {} days remaining", days),
             serde_json::json!({"days_remaining": days, "valid": true, "issuer": "Let's Encrypt", "subject": check.target_host}))
        },
        "disk" => {
            let used_pct = 55.0 + jitter * 0.4;
            let status = if used_pct > 90.0 { "critical" } else if used_pct > 80.0 { "warning" } else { "ok" };
            (status.into(), 1.0, format!("Disk {:.1}% used", used_pct),
             serde_json::json!({"used_pct": used_pct, "total_gb": 100, "used_gb": used_pct, "free_gb": 100.0 - used_pct}))
        },
        "process" => {
            let name = check.config.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
            let count = if jitter > 95.0 { 0 } else { 3 + (jitter as u64 % 5) };
            let status = if count == 0 { "critical" } else { "ok" };
            (status.into(), 2.0, format!("Process '{}': {} instances", name, count),
             serde_json::json!({"process_name": name, "count": count, "cpu_pct": 2.1, "memory_mb": 64.5}))
        },
        "load" => {
            let load1 = 0.5 + jitter * 0.08;
            let warn = check.warning_threshold.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(4.0);
            let crit = check.critical_threshold.as_ref().and_then(|s| s.parse::<f64>().ok()).unwrap_or(8.0);
            let status = if load1 > crit { "critical" } else if load1 > warn { "warning" } else { "ok" };
            (status.into(), 0.5, format!("Load: {:.2} {:.2} {:.2}", load1, load1 * 0.9, load1 * 0.8),
             serde_json::json!({"load_1m": load1, "load_5m": load1 * 0.9, "load_15m": load1 * 0.8}))
        },
        "ntp" => {
            let offset = jitter * 0.5;
            let status = if offset > 500.0 { "critical" } else if offset > 100.0 { "warning" } else { "ok" };
            (status.into(), 1.0, format!("NTP offset: {:.1}ms", offset),
             serde_json::json!({"offset_ms": offset, "stratum": 2, "source": "pool.ntp.org"}))
        },
        "file_age" => {
            let age_secs = 3600 + (jitter * 100.0) as u64;
            let max = check.config.get("max_age_secs").and_then(|v| v.as_u64()).unwrap_or(86400);
            let status = if age_secs > max { "critical" } else if age_secs > max / 2 { "warning" } else { "ok" };
            (status.into(), 1.0, format!("File age: {}s (max: {}s)", age_secs, max),
             serde_json::json!({"age_secs": age_secs, "max_age_secs": max}))
        },
        "ssh" => {
            let rt = 30.0 + jitter * 2.0;
            let success = jitter < 95.0;
            let status = if success { "ok" } else { "critical" };
            (status.into(), rt, format!("SSH {}", if success {"connected"} else {"failed"}),
             serde_json::json!({"connected": success, "response_time_ms": rt}))
        },
        "smtp" => {
            let rt = 100.0 + jitter * 5.0;
            let banner = "220 mail.local ESMTP";
            (String::from("ok"), rt, format!("SMTP banner: {}", banner),
             serde_json::json!({"banner": banner, "response_time_ms": rt, "ehlo_ok": true}))
        },
        "snmp" => {
            let uptime_ticks = 86400 * 100 + (jitter * 1000.0) as u64;
            (String::from("ok"), 5.0 + jitter * 0.2, format!("SNMP uptime: {} ticks", uptime_ticks),
             serde_json::json!({"oid": "1.3.6.1.2.1.1.3.0", "value": uptime_ticks, "type": "TimeTicks"}))
        },
        "custom_script" => {
            let exit_code = if jitter > 90.0 { 2 } else if jitter > 80.0 { 1 } else { 0 };
            let status = match exit_code { 0 => "ok", 1 => "warning", _ => "critical" };
            let rt = 500.0 + jitter * 20.0;
            (status.into(), rt, format!("Script exited with code {}", exit_code),
             serde_json::json!({"exit_code": exit_code, "stdout": "Check passed", "stderr": ""}))
        },
        _ => {
            (String::from("unknown"), 0.0, "Unknown check type".into(), serde_json::json!({}))
        }
    }
}
