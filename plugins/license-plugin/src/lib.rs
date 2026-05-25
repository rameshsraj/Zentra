//! Zentra License Plugin
//!
//! Provides scheduled license management capabilities:
//! - Expiry check scheduler (daily scan for expiring/expired licenses)
//! - Seat utilization calculator (compliance scoring)
//! - Compliance reporter (generates compliance events and metrics)
//!
//! This plugin runs as a background service alongside the main Zentra agent.

use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, debug, error};
use uuid::Uuid;

use zentra_core::models::license::{
    ComplianceStatus, License, LicenseStatus,
};
use zentra_core::models::metric::Metric;

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePluginConfig {
    /// How often to run the expiry check (seconds).
    pub check_interval_secs: u64,
    /// Days before expiry to start warning.
    pub warning_threshold_days: i64,
    /// Days before expiry to escalate to critical.
    pub critical_threshold_days: i64,
    /// Enable automatic status transition (active → expired).
    pub auto_expire: bool,
    /// Enable compliance metric emission.
    pub emit_metrics: bool,
}

impl Default for LicensePluginConfig {
    fn default() -> Self {
        Self {
            check_interval_secs: 86400, // daily
            warning_threshold_days: 90,
            critical_threshold_days: 30,
            auto_expire: true,
            emit_metrics: true,
        }
    }
}

// ============================================================================
// Expiry Check Result
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ExpiryCheckResult {
    pub license_id: Uuid,
    pub license_name: String,
    pub vendor: String,
    pub expiry_date: NaiveDate,
    pub days_remaining: i64,
    pub severity: ExpirySeverity,
    pub current_status: LicenseStatus,
    pub recommended_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpirySeverity {
    Ok,
    Warning,
    Critical,
    Expired,
}

impl ExpirySeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warning => "warning",
            Self::Critical => "critical",
            Self::Expired => "expired",
        }
    }
}

// ============================================================================
// Compliance Report
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct ComplianceReport {
    pub org_id: Uuid,
    pub generated_at: chrono::DateTime<Utc>,
    pub total_licenses: usize,
    pub active_licenses: usize,
    pub expired_licenses: usize,
    pub expiring_soon: usize,
    pub total_seats: i64,
    pub assigned_seats: i64,
    pub free_seats: i64,
    pub over_licensed_count: usize,
    pub under_utilized_count: usize,
    pub compliance_score: f64,
    pub total_annual_cost: f64,
    pub wasted_cost: f64,
    pub per_license: Vec<LicenseComplianceEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LicenseComplianceEntry {
    pub license_id: Uuid,
    pub name: String,
    pub vendor: String,
    pub seat_count: i32,
    pub assigned_seats: i32,
    pub utilization_pct: f64,
    pub compliance_status: ComplianceStatus,
    pub cost_per_seat: f64,
    pub wasted_seats: i32,
    pub wasted_cost: f64,
}

// ============================================================================
// License Plugin Engine
// ============================================================================

pub struct LicensePlugin {
    config: LicensePluginConfig,
    org_id: Uuid,
    last_check: Arc<RwLock<Option<chrono::DateTime<Utc>>>>,
    last_report: Arc<RwLock<Option<ComplianceReport>>>,
}

impl LicensePlugin {
    pub fn new(org_id: Uuid, config: LicensePluginConfig) -> Self {
        Self {
            config,
            org_id,
            last_check: Arc::new(RwLock::new(None)),
            last_report: Arc::new(RwLock::new(None)),
        }
    }

    /// Check all licenses for expiry and return results.
    pub fn check_expiry(&self, licenses: &[License]) -> Vec<ExpiryCheckResult> {
        let today = Utc::now().date_naive();
        let mut results = Vec::new();

        for lic in licenses {
            let expiry = match lic.expiry_date {
                Some(d) => d,
                None => continue, // perpetual license, no expiry
            };

            let days_remaining = (expiry - today).num_days();

            let severity = if days_remaining < 0 {
                ExpirySeverity::Expired
            } else if days_remaining <= self.config.critical_threshold_days {
                ExpirySeverity::Critical
            } else if days_remaining <= self.config.warning_threshold_days {
                ExpirySeverity::Warning
            } else {
                ExpirySeverity::Ok
            };

            let recommended_action = match severity {
                ExpirySeverity::Expired => format!(
                    "License '{}' expired {} days ago. Renew immediately or decommission.",
                    lic.name, -days_remaining
                ),
                ExpirySeverity::Critical => format!(
                    "License '{}' expires in {} days. Initiate renewal process urgently.",
                    lic.name, days_remaining
                ),
                ExpirySeverity::Warning => format!(
                    "License '{}' expires in {} days. Plan renewal with vendor {}.",
                    lic.name, days_remaining, lic.vendor
                ),
                ExpirySeverity::Ok => String::new(),
            };

            if severity != ExpirySeverity::Ok {
                results.push(ExpiryCheckResult {
                    license_id: lic.license_id,
                    license_name: lic.name.clone(),
                    vendor: lic.vendor.clone(),
                    expiry_date: expiry,
                    days_remaining,
                    severity,
                    current_status: lic.status,
                    recommended_action,
                });
            }
        }

        // Sort by urgency: expired first, then by days_remaining ascending
        results.sort_by_key(|r| r.days_remaining);
        results
    }

    /// Calculate seat utilization and compliance for each license.
    pub fn calculate_compliance(&self, licenses: &[License]) -> ComplianceReport {
        let now = Utc::now();
        let mut total_seats: i64 = 0;
        let mut assigned_seats: i64 = 0;
        let mut over_licensed = 0usize;
        let mut under_utilized = 0usize;
        let mut active_count = 0usize;
        let mut expired_count = 0usize;
        let mut total_annual_cost = 0.0f64;
        let mut wasted_cost = 0.0f64;
        let mut per_license = Vec::new();

        let expiring_soon = self.check_expiry(licenses)
            .iter()
            .filter(|r| r.severity == ExpirySeverity::Warning || r.severity == ExpirySeverity::Critical)
            .count();

        for lic in licenses {
            let seats = lic.seat_count as i64;
            let assigned = lic.assigned_seats as i64;
            total_seats += seats;
            assigned_seats += assigned;

            match lic.status {
                LicenseStatus::Active => active_count += 1,
                LicenseStatus::Expired => expired_count += 1,
                _ => {}
            }

            let utilization = if seats > 0 {
                (assigned as f64 / seats as f64) * 100.0
            } else {
                0.0
            };

            let compliance_status = if assigned > seats as i64 {
                over_licensed += 1;
                ComplianceStatus::OverLicensed
            } else if seats > 0 && utilization < 50.0 {
                under_utilized += 1;
                ComplianceStatus::UnderLicensed
            } else if assigned == seats as i64 && seats > 0 {
                ComplianceStatus::FullyUtilized
            } else {
                ComplianceStatus::Ok
            };

            let cost_per_seat = lic.cost_per_seat.unwrap_or(0.0);
            let wasted_seats = (lic.seat_count - lic.assigned_seats).max(0);
            let seat_waste = wasted_seats as f64 * cost_per_seat;

            total_annual_cost += lic.total_cost.unwrap_or(seats as f64 * cost_per_seat);
            wasted_cost += seat_waste;

            per_license.push(LicenseComplianceEntry {
                license_id: lic.license_id,
                name: lic.name.clone(),
                vendor: lic.vendor.clone(),
                seat_count: lic.seat_count,
                assigned_seats: lic.assigned_seats,
                utilization_pct: utilization,
                compliance_status,
                cost_per_seat,
                wasted_seats,
                wasted_cost: seat_waste,
            });
        }

        let free_seats = total_seats - assigned_seats;
        let compliance_score = if licenses.is_empty() {
            100.0
        } else {
            let compliant = licenses.len() - over_licensed - under_utilized;
            (compliant as f64 / licenses.len() as f64) * 100.0
        };

        ComplianceReport {
            org_id: self.org_id,
            generated_at: now,
            total_licenses: licenses.len(),
            active_licenses: active_count,
            expired_licenses: expired_count,
            expiring_soon,
            total_seats,
            assigned_seats,
            free_seats,
            over_licensed_count: over_licensed,
            under_utilized_count: under_utilized,
            compliance_score,
            total_annual_cost,
            wasted_cost,
            per_license,
        }
    }

    /// Emit license metrics for the monitoring pipeline.
    pub fn emit_metrics(&self, licenses: &[License]) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let report = self.calculate_compliance(licenses);

        metrics.push(
            Metric::new("license.total_count", report.total_licenses as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.active_count", report.active_licenses as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.expired_count", report.expired_licenses as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.expiring_soon_count", report.expiring_soon as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.total_seats", report.total_seats as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.assigned_seats", report.assigned_seats as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.free_seats", report.free_seats as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.compliance_score", report.compliance_score, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
                .with_unit("percent")
        );
        metrics.push(
            Metric::new("license.over_licensed_count", report.over_licensed_count as f64, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
        );
        metrics.push(
            Metric::new("license.annual_cost", report.total_annual_cost, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
                .with_unit("usd")
        );
        metrics.push(
            Metric::new("license.wasted_cost", report.wasted_cost, "licenses")
                .with_context(self.org_id, Uuid::nil(), "license-plugin")
                .with_unit("usd")
        );

        // Per-license utilization metrics
        for entry in &report.per_license {
            let resource = format!("license:{}", entry.license_id);
            metrics.push(
                Metric::new("license.utilization_percent", entry.utilization_pct, &resource)
                    .with_context(self.org_id, Uuid::nil(), "license-plugin")
                    .with_unit("percent")
            );
        }

        metrics
    }

    /// Run the scheduled expiry check loop. Call from tokio::spawn.
    pub async fn run_scheduler(self: Arc<Self>, mut license_provider: Box<dyn LicenseProvider>) {
        info!(org_id = %self.org_id, interval = self.config.check_interval_secs, "License plugin scheduler started");

        loop {
            match license_provider.fetch_licenses(self.org_id).await {
                Ok(licenses) => {
                    let expiry_results = self.check_expiry(&licenses);
                    let report = self.calculate_compliance(&licenses);

                    // Log warnings
                    for result in &expiry_results {
                        match result.severity {
                            ExpirySeverity::Expired => {
                                error!(
                                    license = %result.license_name,
                                    vendor = %result.vendor,
                                    days_overdue = -result.days_remaining,
                                    "License EXPIRED"
                                );
                            }
                            ExpirySeverity::Critical => {
                                warn!(
                                    license = %result.license_name,
                                    vendor = %result.vendor,
                                    days_remaining = result.days_remaining,
                                    "License expiring CRITICALLY SOON"
                                );
                            }
                            ExpirySeverity::Warning => {
                                info!(
                                    license = %result.license_name,
                                    vendor = %result.vendor,
                                    days_remaining = result.days_remaining,
                                    "License expiring soon"
                                );
                            }
                            _ => {}
                        }
                    }

                    info!(
                        total = report.total_licenses,
                        active = report.active_licenses,
                        expired = report.expired_licenses,
                        expiring = report.expiring_soon,
                        compliance = format!("{:.1}%", report.compliance_score),
                        "License compliance check completed"
                    );

                    // Emit metrics if enabled
                    if self.config.emit_metrics {
                        let metrics = self.emit_metrics(&licenses);
                        debug!(count = metrics.len(), "Emitted license metrics");
                        // In production: push metrics to ingestion pipeline
                    }

                    // Auto-expire licenses if enabled
                    if self.config.auto_expire {
                        let expired_ids: Vec<Uuid> = expiry_results
                            .iter()
                            .filter(|r| r.severity == ExpirySeverity::Expired && r.current_status == LicenseStatus::Active)
                            .map(|r| r.license_id)
                            .collect();

                        if !expired_ids.is_empty() {
                            info!(count = expired_ids.len(), "Auto-expiring licenses");
                            if let Err(e) = license_provider.mark_expired(expired_ids).await {
                                error!(error = %e, "Failed to auto-expire licenses");
                            }
                        }
                    }

                    // Store last results
                    *self.last_check.write().await = Some(Utc::now());
                    *self.last_report.write().await = Some(report);
                }
                Err(e) => {
                    error!(error = %e, "Failed to fetch licenses for compliance check");
                }
            }

            tokio::time::sleep(Duration::from_secs(self.config.check_interval_secs)).await;
        }
    }

    /// Get the last compliance report (if any).
    pub async fn last_report(&self) -> Option<ComplianceReport> {
        self.last_report.read().await.clone()
    }

    /// Get the last check timestamp.
    pub async fn last_check_time(&self) -> Option<chrono::DateTime<Utc>> {
        *self.last_check.read().await
    }
}

// ============================================================================
// License Provider trait (for scheduler to fetch licenses)
// ============================================================================

/// Abstraction over license data source (DB, API, etc.)
#[async_trait::async_trait]
pub trait LicenseProvider: Send + Sync {
    async fn fetch_licenses(&mut self, org_id: Uuid) -> Result<Vec<License>, Box<dyn std::error::Error + Send + Sync>>;
    async fn mark_expired(&mut self, ids: Vec<Uuid>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_license(name: &str, seats: i32, assigned: i32, expiry_days: Option<i64>) -> License {
        let mut lic = License::new(Uuid::nil(), name, "TestProduct", "TestVendor", seats);
        lic.assigned_seats = assigned;
        lic.free_seats = seats - assigned;
        if let Some(days) = expiry_days {
            lic.expiry_date = Some(Utc::now().date_naive() + chrono::Duration::days(days));
        }
        lic
    }

    #[test]
    fn test_expiry_check_expired() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![make_license("Expired License", 10, 5, Some(-15))];
        let results = plugin.check_expiry(&licenses);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, ExpirySeverity::Expired);
        assert_eq!(results[0].days_remaining, -15);
    }

    #[test]
    fn test_expiry_check_critical() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![make_license("Critical License", 10, 5, Some(15))];
        let results = plugin.check_expiry(&licenses);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, ExpirySeverity::Critical);
    }

    #[test]
    fn test_expiry_check_warning() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![make_license("Warning License", 10, 5, Some(60))];
        let results = plugin.check_expiry(&licenses);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].severity, ExpirySeverity::Warning);
    }

    #[test]
    fn test_expiry_check_ok_not_reported() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![make_license("Healthy License", 10, 5, Some(365))];
        let results = plugin.check_expiry(&licenses);
        assert!(results.is_empty());
    }

    #[test]
    fn test_expiry_check_perpetual_skipped() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![make_license("Perpetual License", 10, 5, None)];
        let results = plugin.check_expiry(&licenses);
        assert!(results.is_empty());
    }

    #[test]
    fn test_compliance_report_basic() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![
            make_license("Full", 10, 10, Some(365)),
            make_license("Under", 100, 20, Some(365)),
            make_license("Over", 5, 8, Some(365)),
            make_license("Normal", 50, 30, Some(365)),
        ];
        let report = plugin.calculate_compliance(&licenses);

        assert_eq!(report.total_licenses, 4);
        assert_eq!(report.total_seats, 165);
        assert_eq!(report.assigned_seats, 68);
        assert_eq!(report.free_seats, 97);
        assert_eq!(report.over_licensed_count, 1);
        assert_eq!(report.under_utilized_count, 1); // "Under" has 20% util
    }

    #[test]
    fn test_compliance_score() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        // All compliant
        let licenses = vec![
            make_license("A", 10, 7, Some(365)),
            make_license("B", 20, 15, Some(365)),
        ];
        let report = plugin.calculate_compliance(&licenses);
        assert_eq!(report.compliance_score, 100.0);
    }

    #[test]
    fn test_emit_metrics() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let licenses = vec![
            make_license("A", 10, 7, Some(365)),
            make_license("B", 20, 15, Some(60)),
        ];
        let metrics = plugin.emit_metrics(&licenses);
        // 11 aggregate + 2 per-license utilization = 13
        assert!(metrics.len() >= 13);

        let names: Vec<&str> = metrics.iter().map(|m| m.metric_name.as_str()).collect();
        assert!(names.contains(&"license.total_count"));
        assert!(names.contains(&"license.compliance_score"));
        assert!(names.contains(&"license.utilization_percent"));
    }

    #[test]
    fn test_wasted_cost_calculation() {
        let plugin = LicensePlugin::new(Uuid::nil(), LicensePluginConfig::default());
        let mut lic = make_license("Expensive", 100, 30, Some(365));
        lic.cost_per_seat = Some(500.0);
        lic.total_cost = Some(50000.0);
        let report = plugin.calculate_compliance(&[lic]);

        assert_eq!(report.per_license[0].wasted_seats, 70);
        assert!((report.per_license[0].wasted_cost - 35000.0).abs() < 0.01);
        assert!((report.wasted_cost - 35000.0).abs() < 0.01);
    }
}
