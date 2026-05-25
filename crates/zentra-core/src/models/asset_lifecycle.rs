//! Asset Lifecycle: Depreciation + Maintenance Scheduling
//!
//! Reference: Snipe-IT `app/Models/Depreciation.php`, `Maintenance.php`
//! Reference: Keygen `app/models/machine_process.rb`, `machine_component.rb`

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// Depreciation
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepreciationSchedule {
    pub schedule_id: Uuid,
    pub name: String,
    pub method: DepreciationMethod,
    pub useful_life_months: u32,
    pub salvage_value_percent: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DepreciationMethod {
    StraightLine,
    DecliningBalance,
    SumOfYearsDigits,
    UnitsOfProduction,
}

impl DepreciationSchedule {
    pub fn new(name: impl Into<String>, method: DepreciationMethod, useful_life_months: u32) -> Self {
        Self {
            schedule_id: Uuid::now_v7(),
            name: name.into(),
            method,
            useful_life_months,
            salvage_value_percent: 10.0,
            created_at: Utc::now(),
        }
    }

    /// Calculate current value given purchase cost and purchase date
    pub fn current_value(&self, purchase_cost: f64, purchase_date: NaiveDate) -> f64 {
        let today = Utc::now().date_naive();
        let months_elapsed = ((today - purchase_date).num_days() as f64 / 30.44).floor() as u32;
        let salvage = purchase_cost * (self.salvage_value_percent / 100.0);
        let depreciable = purchase_cost - salvage;

        if months_elapsed >= self.useful_life_months {
            return salvage;
        }

        match self.method {
            DepreciationMethod::StraightLine => {
                let monthly_depreciation = depreciable / self.useful_life_months as f64;
                purchase_cost - (monthly_depreciation * months_elapsed as f64)
            }
            DepreciationMethod::DecliningBalance => {
                let rate = 2.0 / self.useful_life_months as f64;
                let mut value = purchase_cost;
                for _ in 0..months_elapsed {
                    value -= value * rate;
                    if value < salvage {
                        return salvage;
                    }
                }
                value
            }
            DepreciationMethod::SumOfYearsDigits => {
                let n = self.useful_life_months;
                let sum = n * (n + 1) / 2;
                let mut accumulated = 0.0;
                for i in 0..months_elapsed.min(n) {
                    let remaining = n - i;
                    accumulated += depreciable * (remaining as f64 / sum as f64);
                }
                purchase_cost - accumulated
            }
            DepreciationMethod::UnitsOfProduction => {
                // Simplified: treat as straight-line (needs actual usage data)
                let monthly_depreciation = depreciable / self.useful_life_months as f64;
                purchase_cost - (monthly_depreciation * months_elapsed as f64)
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Maintenance
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceRecord {
    pub maintenance_id: Uuid,
    pub asset_id: Uuid,
    pub asset_type: String,      // "license", "target", "machine"
    pub maintenance_type: MaintenanceType,
    pub title: String,
    pub description: Option<String>,
    pub supplier: Option<String>,
    pub cost: Option<f64>,
    pub start_date: NaiveDate,
    pub completion_date: Option<NaiveDate>,
    pub is_warranty: bool,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceType {
    Maintenance,
    Repair,
    Upgrade,
    HardwareSupport,
    SoftwareSupport,
    Testing,
    Calibration,
}

impl MaintenanceRecord {
    pub fn new(
        asset_id: Uuid,
        asset_type: impl Into<String>,
        maintenance_type: MaintenanceType,
        title: impl Into<String>,
        start_date: NaiveDate,
    ) -> Self {
        Self {
            maintenance_id: Uuid::now_v7(),
            asset_id,
            asset_type: asset_type.into(),
            maintenance_type,
            title: title.into(),
            description: None,
            supplier: None,
            cost: None,
            start_date,
            completion_date: None,
            is_warranty: false,
            notes: None,
            created_at: Utc::now(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.completion_date.is_some()
    }

    pub fn duration_days(&self) -> Option<i64> {
        self.completion_date.map(|end| (end - self.start_date).num_days())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Machine Processes + Components (Keygen-inspired)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineProcess {
    pub process_id: Uuid,
    pub machine_fingerprint: String,
    pub pid: u32,
    pub name: String,
    pub status: ProcessStatus,
    pub metadata: serde_json::Value,
    pub last_heartbeat_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Alive,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineComponent {
    pub component_id: Uuid,
    pub machine_fingerprint: String,
    pub name: String,
    pub fingerprint: String,    // unique component identifier
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_straight_line_depreciation() {
        let schedule = DepreciationSchedule::new(
            "IT Equipment",
            DepreciationMethod::StraightLine,
            36, // 3 years
        );

        let purchase_date = Utc::now().date_naive() - chrono::Duration::days(365); // 1 year ago
        let value = schedule.current_value(10_000.0, purchase_date);

        // After 12 months of 36: depreciated ~1/3 of depreciable amount
        assert!(value < 10_000.0);
        assert!(value > 5_000.0);
    }

    #[test]
    fn test_fully_depreciated() {
        let schedule = DepreciationSchedule::new(
            "Old Equipment",
            DepreciationMethod::StraightLine,
            12,
        );

        let purchase_date = Utc::now().date_naive() - chrono::Duration::days(730); // 2 years ago
        let value = schedule.current_value(10_000.0, purchase_date);

        // Should be at salvage value (10% of 10k = 1k)
        assert!((value - 1_000.0).abs() < 0.01);
    }

    #[test]
    fn test_maintenance_record() {
        let record = MaintenanceRecord::new(
            Uuid::new_v4(),
            "license",
            MaintenanceType::SoftwareSupport,
            "Annual License Renewal",
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        );

        assert!(!record.is_complete());
        assert!(record.duration_days().is_none());
    }

    #[test]
    fn test_maintenance_complete() {
        let mut record = MaintenanceRecord::new(
            Uuid::new_v4(),
            "target",
            MaintenanceType::Upgrade,
            "Server RAM Upgrade",
            NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
        );
        record.completion_date = Some(NaiveDate::from_ymd_opt(2026, 3, 5).unwrap());

        assert!(record.is_complete());
        assert_eq!(record.duration_days(), Some(4));
    }
}
