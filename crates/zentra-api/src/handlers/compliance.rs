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
    let now = Utc::now();

    let frameworks = vec![
        ComplianceFramework {
            id: "fw-cis-ubuntu".into(),
            name: "CIS Ubuntu 22.04 LTS".into(),
            version: "2.0.0".into(),
            category: "os".into(),
            total_controls: 234,
            enabled: true,
            description: "Center for Internet Security benchmark for Ubuntu Linux 22.04 LTS".into(),
        },
        ComplianceFramework {
            id: "fw-cis-rhel".into(),
            name: "CIS RHEL 9".into(),
            version: "1.0.0".into(),
            category: "os".into(),
            total_controls: 218,
            enabled: true,
            description: "CIS benchmark for Red Hat Enterprise Linux 9".into(),
        },
        ComplianceFramework {
            id: "fw-cis-aws".into(),
            name: "CIS AWS Foundations".into(),
            version: "3.0.0".into(),
            category: "cloud".into(),
            total_controls: 58,
            enabled: true,
            description: "CIS benchmark for AWS account security posture".into(),
        },
        ComplianceFramework {
            id: "fw-pci-dss".into(),
            name: "PCI-DSS v4.0".into(),
            version: "4.0".into(),
            category: "application".into(),
            total_controls: 251,
            enabled: true,
            description: "Payment Card Industry Data Security Standard".into(),
        },
        ComplianceFramework {
            id: "fw-soc2".into(),
            name: "SOC 2 Type II".into(),
            version: "2023".into(),
            category: "application".into(),
            total_controls: 64,
            enabled: true,
            description: "Service Organization Control 2 — Security, Availability, Confidentiality".into(),
        },
    ];

    let scan1_id = "scan-001";
    let scan2_id = "scan-002";

    let scans = vec![
        ComplianceScan {
            id: scan1_id.into(),
            framework_id: "fw-cis-ubuntu".into(),
            framework_name: "CIS Ubuntu 22.04 LTS".into(),
            target_id: "target-001".into(),
            target_name: "app-server-01".into(),
            started_at: (now - chrono::Duration::hours(3)).to_rfc3339(),
            completed_at: Some((now - chrono::Duration::hours(2)).to_rfc3339()),
            status: "completed".into(),
            pass_count: 187,
            fail_count: 28,
            warn_count: 12,
            skip_count: 7,
            score: 80.3,
            duration_secs: Some(142),
        },
        ComplianceScan {
            id: scan2_id.into(),
            framework_id: "fw-cis-aws".into(),
            framework_name: "CIS AWS Foundations".into(),
            target_id: "aws-prod-account".into(),
            target_name: "AWS Production Account".into(),
            started_at: (now - chrono::Duration::hours(1)).to_rfc3339(),
            completed_at: Some((now - chrono::Duration::minutes(50)).to_rfc3339()),
            status: "completed".into(),
            pass_count: 44,
            fail_count: 9,
            warn_count: 3,
            skip_count: 2,
            score: 75.9,
            duration_secs: Some(38),
        },
    ];

    let findings = vec![
        ScanFinding {
            id: "finding-001".into(),
            scan_id: scan1_id.into(),
            control_id: "CIS-1.1.1".into(),
            control_title: "Ensure mounting of cramfs filesystems is disabled".into(),
            section: "1. Initial Setup".into(),
            status: "pass".into(),
            severity: "low".into(),
            details: "cramfs kernel module is blacklisted via /etc/modprobe.d/".into(),
            remediation: "No action required.".into(),
            evidence: Some("Module not loaded. Blacklist entry confirmed.".into()),
        },
        ScanFinding {
            id: "finding-002".into(),
            scan_id: scan1_id.into(),
            control_id: "CIS-1.3.1".into(),
            control_title: "Ensure sudo is installed".into(),
            section: "1. Initial Setup".into(),
            status: "pass".into(),
            severity: "medium".into(),
            details: "sudo package version 1.9.9p2-3ubuntu2 is installed".into(),
            remediation: "No action required.".into(),
            evidence: Some("apt-get install sudo=1.9.9p2-3ubuntu2".into()),
        },
        ScanFinding {
            id: "finding-003".into(),
            scan_id: scan1_id.into(),
            control_id: "CIS-2.1.1".into(),
            control_title: "Ensure time synchronization is in use".into(),
            section: "2. Services".into(),
            status: "fail".into(),
            severity: "medium".into(),
            details: "systemd-timesyncd is not active. No NTP daemon running.".into(),
            remediation: "Install and enable chrony: apt install chrony && systemctl enable chrony".into(),
            evidence: None,
        },
        ScanFinding {
            id: "finding-004".into(),
            scan_id: scan1_id.into(),
            control_id: "CIS-3.1.1".into(),
            control_title: "Ensure IP forwarding is disabled".into(),
            section: "3. Network Configuration".into(),
            status: "fail".into(),
            severity: "high".into(),
            details: "net.ipv4.ip_forward = 1 — IP forwarding is enabled. This host is not a router.".into(),
            remediation: "Run: sysctl -w net.ipv4.ip_forward=0 && echo 'net.ipv4.ip_forward = 0' >> /etc/sysctl.d/99-cis.conf".into(),
            evidence: Some("sysctl net.ipv4.ip_forward => 1".into()),
        },
        ScanFinding {
            id: "finding-005".into(),
            scan_id: scan1_id.into(),
            control_id: "CIS-5.2.1".into(),
            control_title: "Ensure SSH Protocol is set to 2".into(),
            section: "5. Access Control".into(),
            status: "pass".into(),
            severity: "high".into(),
            details: "Protocol 2 is default in OpenSSH 8.0+. Config confirmed.".into(),
            remediation: "No action required.".into(),
            evidence: Some("sshd_config: Protocol not explicitly set (defaults to 2)".into()),
        },
        ScanFinding {
            id: "finding-006".into(),
            scan_id: scan1_id.into(),
            control_id: "CIS-5.3.1".into(),
            control_title: "Ensure password hashing algorithm is SHA-512".into(),
            section: "5. Access Control".into(),
            status: "fail".into(),
            severity: "high".into(),
            details: "ENCRYPT_METHOD in /etc/login.defs is set to MD5.".into(),
            remediation: "Update /etc/login.defs: ENCRYPT_METHOD SHA512".into(),
            evidence: Some("grep ENCRYPT_METHOD /etc/login.defs => ENCRYPT_METHOD MD5".into()),
        },
        ScanFinding {
            id: "finding-007".into(),
            scan_id: scan2_id.into(),
            control_id: "CIS-AWS-1.4".into(),
            control_title: "Ensure no root user access key exists".into(),
            section: "1. Identity and Access Management".into(),
            status: "pass".into(),
            severity: "critical".into(),
            details: "No access keys found for the root account.".into(),
            remediation: "No action required.".into(),
            evidence: Some("IAM ListAccessKeys for root: empty".into()),
        },
        ScanFinding {
            id: "finding-008".into(),
            scan_id: scan2_id.into(),
            control_id: "CIS-AWS-2.1.1".into(),
            control_title: "Ensure S3 bucket policy is set to deny HTTP requests".into(),
            section: "2. Storage".into(),
            status: "fail".into(),
            severity: "high".into(),
            details: "S3 bucket 'data-archive-prod' does not enforce HTTPS-only access.".into(),
            remediation: "Add bucket policy with aws:SecureTransport condition set to false → Deny.".into(),
            evidence: Some("Bucket: data-archive-prod — no SecureTransport policy found".into()),
        },
        ScanFinding {
            id: "finding-009".into(),
            scan_id: scan2_id.into(),
            control_id: "CIS-AWS-3.3".into(),
            control_title: "Ensure the S3 bucket used to store CloudTrail logs is not publicly accessible".into(),
            section: "3. Logging".into(),
            status: "pass".into(),
            severity: "critical".into(),
            details: "CloudTrail log bucket has Block Public Access enabled.".into(),
            remediation: "No action required.".into(),
            evidence: None,
        },
    ];

    ComplianceStore { frameworks, scans, findings }
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
