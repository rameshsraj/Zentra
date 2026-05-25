use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// License category
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseCategory {
    Software,
    Hardware,
    Cloud,
    Saas,
    Perpetual,
}

impl LicenseCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::Hardware => "hardware",
            Self::Cloud => "cloud",
            Self::Saas => "saas",
            Self::Perpetual => "perpetual",
        }
    }
}

/// License type / entitlement model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseType {
    PerSeat,
    PerDevice,
    Site,
    Enterprise,
    Concurrent,
    Subscription,
}

impl LicenseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PerSeat => "per_seat",
            Self::PerDevice => "per_device",
            Self::Site => "site",
            Self::Enterprise => "enterprise",
            Self::Concurrent => "concurrent",
            Self::Subscription => "subscription",
        }
    }
}

/// License status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LicenseStatus {
    Active,
    Expired,
    Terminated,
    Pending,
    Suspended,
}

impl LicenseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Expired => "expired",
            Self::Terminated => "terminated",
            Self::Pending => "pending",
            Self::Suspended => "suspended",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "active" => Self::Active,
            "expired" => Self::Expired,
            "terminated" => Self::Terminated,
            "pending" => Self::Pending,
            "suspended" => Self::Suspended,
            _ => Self::Active,
        }
    }
}

/// Compliance status derived from seat utilization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceStatus {
    Ok,
    FullyUtilized,
    OverLicensed,
    UnderLicensed,
}

/// A software/hardware license record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub license_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub serial_number: Option<String>,
    pub product_name: String,
    pub product_version: Option<String>,
    pub vendor: String,
    pub category: LicenseCategory,
    pub license_type: LicenseType,
    pub seat_count: i32,
    pub cost_per_seat: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency: String,
    pub purchase_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub renewal_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub po_number: Option<String>,
    pub order_number: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_email: Option<String>,
    pub supplier_phone: Option<String>,
    pub contract_url: Option<String>,
    pub notes: Option<String>,
    pub is_maintained: bool,
    pub is_reassignable: bool,
    pub status: LicenseStatus,
    pub environment: String,
    pub tags: BTreeMap<String, String>,
    pub metadata: serde_json::Value,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // computed fields (not persisted directly)
    #[serde(default)]
    pub assigned_seats: i32,
    #[serde(default)]
    pub free_seats: i32,
}

impl License {
    pub fn new(
        org_id: Uuid,
        name: impl Into<String>,
        product_name: impl Into<String>,
        vendor: impl Into<String>,
        seat_count: i32,
    ) -> Self {
        let now = Utc::now();
        Self {
            license_id: Uuid::now_v7(),
            org_id,
            name: name.into(),
            serial_number: None,
            product_name: product_name.into(),
            product_version: None,
            vendor: vendor.into(),
            category: LicenseCategory::Software,
            license_type: LicenseType::PerSeat,
            seat_count,
            cost_per_seat: None,
            total_cost: None,
            currency: "USD".into(),
            purchase_date: None,
            expiry_date: None,
            renewal_date: None,
            termination_date: None,
            po_number: None,
            order_number: None,
            supplier_name: None,
            supplier_email: None,
            supplier_phone: None,
            contract_url: None,
            notes: None,
            is_maintained: false,
            is_reassignable: true,
            status: LicenseStatus::Active,
            environment: "production".into(),
            tags: BTreeMap::new(),
            metadata: serde_json::json!({}),
            created_by: None,
            created_at: now,
            updated_at: now,
            assigned_seats: 0,
            free_seats: seat_count,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expiry_date {
            exp <= Utc::now().date_naive()
        } else {
            false
        }
    }

    pub fn is_terminated(&self) -> bool {
        if let Some(term) = self.termination_date {
            term <= Utc::now().date_naive()
        } else {
            false
        }
    }

    pub fn is_inactive(&self) -> bool {
        self.is_expired() || self.is_terminated()
    }

    pub fn days_until_expiry(&self) -> Option<i64> {
        self.expiry_date.map(|exp| {
            (exp - Utc::now().date_naive()).num_days()
        })
    }

    pub fn utilization_percent(&self) -> f64 {
        if self.seat_count <= 0 {
            return 0.0;
        }
        (self.assigned_seats as f64 / self.seat_count as f64) * 100.0
    }

    pub fn compliance_status(&self) -> ComplianceStatus {
        if self.assigned_seats > self.seat_count {
            ComplianceStatus::OverLicensed
        } else if self.assigned_seats == self.seat_count {
            ComplianceStatus::FullyUtilized
        } else if self.seat_count > 0 && (self.assigned_seats as f64 / self.seat_count as f64) < 0.5 {
            ComplianceStatus::UnderLicensed
        } else {
            ComplianceStatus::Ok
        }
    }
}

/// Individual seat/entitlement within a license
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseSeat {
    pub seat_id: Uuid,
    pub license_id: Uuid,
    pub org_id: Uuid,
    pub assigned_to: Option<String>,
    pub assigned_user_id: Option<Uuid>,
    pub assigned_asset: Option<String>,
    pub assigned_target_id: Option<Uuid>,
    pub notes: Option<String>,
    pub checkout_at: Option<DateTime<Utc>>,
    pub checkin_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LicenseSeat {
    pub fn new(license_id: Uuid, org_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            seat_id: Uuid::now_v7(),
            license_id,
            org_id,
            assigned_to: None,
            assigned_user_id: None,
            assigned_asset: None,
            assigned_target_id: None,
            notes: None,
            checkout_at: None,
            checkin_at: None,
            is_active: true,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_available(&self) -> bool {
        self.is_active && self.assigned_to.is_none()
    }

    pub fn checkout(&mut self, user: impl Into<String>) {
        self.assigned_to = Some(user.into());
        self.checkout_at = Some(Utc::now());
        self.checkin_at = None;
        self.updated_at = Utc::now();
    }

    pub fn checkin(&mut self) {
        self.assigned_to = None;
        self.assigned_user_id = None;
        self.assigned_asset = None;
        self.assigned_target_id = None;
        self.checkin_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// Assignment of a license to a target/user/device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseAssignment {
    pub assignment_id: Uuid,
    pub license_id: Uuid,
    pub seat_id: Option<Uuid>,
    pub org_id: Uuid,
    pub target_id: Option<Uuid>,
    pub assigned_to: String,
    pub assignment_type: String,
    pub assigned_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl LicenseAssignment {
    pub fn new(
        license_id: Uuid,
        org_id: Uuid,
        assigned_to: impl Into<String>,
        assignment_type: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            assignment_id: Uuid::now_v7(),
            license_id,
            seat_id: None,
            org_id,
            target_id: None,
            assigned_to: assigned_to.into(),
            assignment_type: assignment_type.into(),
            assigned_at: now,
            released_at: None,
            notes: None,
            created_at: now,
        }
    }
}

/// Immutable audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseAuditEntry {
    pub audit_id: Uuid,
    pub license_id: Uuid,
    pub org_id: Uuid,
    pub action: String,
    pub actor: String,
    pub changes: serde_json::Value,
    pub details: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl LicenseAuditEntry {
    pub fn new(
        license_id: Uuid,
        org_id: Uuid,
        action: impl Into<String>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            audit_id: Uuid::now_v7(),
            license_id,
            org_id,
            action: action.into(),
            actor: actor.into(),
            changes: serde_json::json!({}),
            details: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_changes(mut self, changes: serde_json::Value) -> Self {
        self.changes = changes;
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

/// Request DTO for creating a license
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLicenseRequest {
    pub name: String,
    pub serial_number: Option<String>,
    pub product_name: String,
    pub product_version: Option<String>,
    pub vendor: String,
    pub category: Option<String>,
    pub license_type: Option<String>,
    pub seat_count: i32,
    pub cost_per_seat: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency: Option<String>,
    pub purchase_date: Option<String>,
    pub expiry_date: Option<String>,
    pub renewal_date: Option<String>,
    pub po_number: Option<String>,
    pub order_number: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_email: Option<String>,
    pub supplier_phone: Option<String>,
    pub contract_url: Option<String>,
    pub notes: Option<String>,
    pub is_maintained: Option<bool>,
    pub is_reassignable: Option<bool>,
    pub environment: Option<String>,
    pub tags: Option<BTreeMap<String, String>>,
}

/// Request DTO for updating a license
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLicenseRequest {
    pub name: Option<String>,
    pub serial_number: Option<String>,
    pub product_name: Option<String>,
    pub product_version: Option<String>,
    pub vendor: Option<String>,
    pub category: Option<String>,
    pub license_type: Option<String>,
    pub seat_count: Option<i32>,
    pub cost_per_seat: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency: Option<String>,
    pub purchase_date: Option<String>,
    pub expiry_date: Option<String>,
    pub renewal_date: Option<String>,
    pub po_number: Option<String>,
    pub order_number: Option<String>,
    pub supplier_name: Option<String>,
    pub supplier_email: Option<String>,
    pub supplier_phone: Option<String>,
    pub contract_url: Option<String>,
    pub notes: Option<String>,
    pub is_maintained: Option<bool>,
    pub is_reassignable: Option<bool>,
    pub status: Option<String>,
    pub environment: Option<String>,
    pub tags: Option<BTreeMap<String, String>>,
}

/// Request DTO for assigning a seat
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignSeatRequest {
    pub assigned_to: String,
    pub assigned_asset: Option<String>,
    pub assigned_target_id: Option<Uuid>,
    pub notes: Option<String>,
}

/// Compliance summary DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub total_licenses: i64,
    pub active_licenses: i64,
    pub expired_licenses: i64,
    pub expiring_soon: i64,
    pub total_seats: i64,
    pub assigned_seats: i64,
    pub free_seats: i64,
    pub over_licensed: i64,
    pub under_licensed: i64,
    pub total_cost: f64,
    pub annual_cost: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_creation() {
        let org = Uuid::new_v4();
        let lic = License::new(org, "Office 365", "Microsoft 365 E3", "Microsoft", 50);
        assert_eq!(lic.seat_count, 50);
        assert_eq!(lic.free_seats, 50);
        assert_eq!(lic.assigned_seats, 0);
        assert_eq!(lic.status, LicenseStatus::Active);
        assert!(!lic.is_expired());
    }

    #[test]
    fn test_license_compliance() {
        let org = Uuid::new_v4();
        let mut lic = License::new(org, "Test", "Product", "Vendor", 10);

        lic.assigned_seats = 5;
        lic.free_seats = 5;
        assert_eq!(lic.compliance_status(), ComplianceStatus::Ok);

        lic.assigned_seats = 10;
        lic.free_seats = 0;
        assert_eq!(lic.compliance_status(), ComplianceStatus::FullyUtilized);

        lic.assigned_seats = 15;
        assert_eq!(lic.compliance_status(), ComplianceStatus::OverLicensed);

        lic.assigned_seats = 2;
        assert_eq!(lic.compliance_status(), ComplianceStatus::UnderLicensed);
    }

    #[test]
    fn test_license_expiry() {
        let org = Uuid::new_v4();
        let mut lic = License::new(org, "Test", "Product", "Vendor", 1);
        assert!(lic.days_until_expiry().is_none());

        lic.expiry_date = Some(Utc::now().date_naive() + chrono::Duration::days(30));
        assert_eq!(lic.days_until_expiry(), Some(30));
        assert!(!lic.is_expired());

        lic.expiry_date = Some(Utc::now().date_naive() - chrono::Duration::days(1));
        assert!(lic.is_expired());
    }

    #[test]
    fn test_seat_checkout_checkin() {
        let org = Uuid::new_v4();
        let lic_id = Uuid::new_v4();
        let mut seat = LicenseSeat::new(lic_id, org);
        assert!(seat.is_available());

        seat.checkout("john@example.com");
        assert!(!seat.is_available());
        assert_eq!(seat.assigned_to, Some("john@example.com".into()));

        seat.checkin();
        assert!(seat.is_available());
        assert!(seat.assigned_to.is_none());
    }

    #[test]
    fn test_audit_entry() {
        let lic_id = Uuid::new_v4();
        let org = Uuid::new_v4();
        let entry = LicenseAuditEntry::new(lic_id, org, "created", "admin")
            .with_changes(serde_json::json!({"seat_count": 50}))
            .with_details("Initial license creation");
        assert_eq!(entry.action, "created");
        assert_eq!(entry.actor, "admin");
    }
}
