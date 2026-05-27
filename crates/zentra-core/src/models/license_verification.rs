use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFeatures {
    pub enable_oracle: bool,
    pub enable_ebpf_network: bool,
    pub enable_ai_observability: bool,
    pub enable_otel: bool,
    pub allowed_menu_items: Vec<String>, // List of menu item IDs the user can see/access
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePayload {
    pub org_id: String,
    pub customer_name: String,
    pub onboarded_at: DateTime<Utc>,
    pub trial_duration_days: i64, // e.g. 5, 7, 30, 90 days
    pub max_targets: usize,
    pub features: LicenseFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseFile {
    pub payload: LicensePayload,
    pub signature_hex: String,
}
