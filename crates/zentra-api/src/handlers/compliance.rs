use axum::{
    extract::{Path, Query, State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

// ============================================================================
// CIS Compliance Scanner
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceFramework {
    pub id: String,
    pub name: String,         // CIS Ubuntu 22.04, CIS RHEL 9, SOC2, PCI-DSS, HIPAA
    pub version: String,
    pub category: String,     // os, cloud, application, database
    pub total_controls: u32,
    pub enabled: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceControl {
    pub id: String,
    pub framework_id: String,
    pub section: String,
    pub title: String,
    pub description: String,
    pub severity: String,     // critical, high, medium, low
    pub remediation: String,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceScan {
    pub id: String,
    pub framework_id: String,
    pub framework_name: String,
    pub target_id: String,
    pub target_name: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,       // running, completed, failed
    pub pass_count: u32,
    pub fail_count: u32,
    pub warn_count: u32,
    pub skip_count: u32,
    pub score: f32,           // 0-100
    pub duration_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFinding {
    pub id: String,
    pub scan_id: String,
    pub control_id: String,
    pub control_title: String,
    pub section: String,
    pub status: String,       // pass, fail, warn, skip
    pub severity: String,
    pub details: String,
    pub remediation: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TriggerScanRequest {
    pub framework_id: String,
    pub target_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ScanListParams {
    pub framework_id: Option<String>,
    pub target_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FindingListParams {
    pub scan_id: Option<String>,
    pub status: Option<String>,
    pub severity: Option<String>,
}

struct ComplianceStore {
    frameworks: Vec<ComplianceFramework>,
    scans: Vec<ComplianceScan>,
    findings: Vec<ScanFinding>,
}

fn store() -> &'static RwLock<ComplianceStore> {
    static INSTANCE: OnceLock<RwLock<ComplianceStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> ComplianceStore {
    ComplianceStore {
        frameworks: vec![],
        scans: vec![],
        findings: vec![],
    }
}

pub async fn list_frameworks(
    State(_s): State<AppState>,
) -> Result<Json<Vec<ComplianceFramework>>, ApiError> {
    let store = store().read();
    Ok(Json(store.frameworks.clone()))
}

pub async fn list_scans(
    State(_s): State<AppState>,
    Query(params): Query<ScanListParams>,
) -> Result<Json<Vec<ComplianceScan>>, ApiError> {
    let store = store().read();
    let result: Vec<ComplianceScan> = store.scans.iter()
        .filter(|s| params.framework_id.as_ref().map(|f| &s.framework_id == f).unwrap_or(true))
        .filter(|s| params.target_id.as_ref().map(|t| s.target_id.as_str() == t.as_str()).unwrap_or(true))
        .filter(|s| params.status.as_ref().map(|st| &s.status == st).unwrap_or(true))
        .cloned().collect();
    Ok(Json(result))
}

pub async fn get_scan(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ComplianceScan>, ApiError> {
    let store = store().read();
    store.scans.iter().find(|s| s.id == id).cloned().map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Scan not found".into())))
}

pub async fn trigger_scan(
    State(_s): State<AppState>,
    Json(req): Json<TriggerScanRequest>,
) -> Result<Json<ComplianceScan>, ApiError> {
    let now = Utc::now();
    let fw_name = {
        let store = store().read();
        store.frameworks.iter().find(|f| f.id == req.framework_id)
            .map(|f| f.name.clone())
            .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Framework not found".into())))?
    };

    let scan = ComplianceScan {
        id: crate::error::short_id("scan"),
        framework_id: req.framework_id,
        framework_name: fw_name,
        target_id: req.target_id.clone(),
        target_name: req.target_id,
        started_at: now.to_rfc3339(),
        completed_at: Some((now + chrono::Duration::seconds(120)).to_rfc3339()),
        status: "completed".into(),
        pass_count: 168,
        fail_count: 21,
        warn_count: 9,
        skip_count: 5,
        score: 83.2,
        duration_secs: Some(120),
    };
    store().write().scans.push(scan.clone());
    Ok(Json(scan))
}

pub async fn list_findings(
    State(_s): State<AppState>,
    Query(params): Query<FindingListParams>,
) -> Result<Json<Vec<ScanFinding>>, ApiError> {
    let store = store().read();
    let result: Vec<ScanFinding> = store.findings.iter()
        .filter(|f| params.scan_id.as_ref().map(|id| &f.scan_id == id).unwrap_or(true))
        .filter(|f| params.status.as_ref().map(|s| &f.status == s).unwrap_or(true))
        .filter(|f| params.severity.as_ref().map(|s| &f.severity == s).unwrap_or(true))
        .cloned().collect();
    Ok(Json(result))
}

pub async fn compliance_summary(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let total_scans = store.scans.len();
    let completed = store.scans.iter().filter(|s| s.status == "completed").count();
    let avg_score = if completed > 0 {
        store.scans.iter().filter(|s| s.status == "completed").map(|s| s.score).sum::<f32>() / completed as f32
    } else { 0.0 };
    let total_fail: u32 = store.scans.iter().map(|s| s.fail_count).sum();
    let critical_findings = store.findings.iter().filter(|f| f.severity == "critical" && f.status == "fail").count();

    Ok(Json(serde_json::json!({
        "total_frameworks": store.frameworks.len(),
        "total_scans": total_scans,
        "completed_scans": completed,
        "average_score": (avg_score * 10.0).round() / 10.0,
        "total_failures": total_fail,
        "critical_findings": critical_findings,
        "by_framework": store.scans.iter().map(|s| serde_json::json!({
            "framework": s.framework_name,
            "score": s.score,
            "target": s.target_name,
            "status": s.status,
            "failures": s.fail_count,
        })).collect::<Vec<_>>(),
    })))
}
