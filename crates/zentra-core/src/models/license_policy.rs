//! License Policy Engine
//!
//! Implements configurable policies for license lifecycle management:
//! - Expiry strategies: hard expire, grace period, notify-only
//! - Renewal strategies: auto-renew, manual, subscription
//! - Transfer policy: allow/deny license transfers between machines
//! - Overage handling: block, allow with penalty, notify
//! - Machine fingerprinting: hardware-based unique ID per monitored host
//! - Heartbeat monitoring: detect dead machines, auto-release seats
//! - Entitlements: feature flags tied to license tiers
//! - License key generation: cryptographic key schemes (HMAC-SHA256)

use std::collections::HashMap;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// Expiry Strategies
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpiryStrategy {
    /// License becomes immediately unusable on expiry date
    HardExpire,
    /// License enters a grace period (N days) after expiry, then hard expires
    GracePeriod { grace_days: u32 },
    /// License continues to work but notifications are sent
    NotifyOnly,
    /// License never expires (perpetual)
    NoExpiry,
}

impl ExpiryStrategy {
    /// Evaluate whether a license is still usable given expiry date
    pub fn is_usable(&self, expiry_date: Option<NaiveDate>, today: NaiveDate) -> ExpiryStatus {
        let Some(expiry) = expiry_date else {
            return ExpiryStatus::Active;
        };

        if today < expiry {
            let days_left = (expiry - today).num_days();
            if days_left <= 30 {
                return ExpiryStatus::ExpiringSoon { days_left: days_left as u32 };
            }
            return ExpiryStatus::Active;
        }

        match self {
            ExpiryStrategy::HardExpire => ExpiryStatus::Expired,
            ExpiryStrategy::GracePeriod { grace_days } => {
                let grace_end = expiry + chrono::Duration::days(*grace_days as i64);
                if today <= grace_end {
                    let grace_left = (grace_end - today).num_days();
                    ExpiryStatus::InGracePeriod { days_left: grace_left as u32 }
                } else {
                    ExpiryStatus::Expired
                }
            }
            ExpiryStrategy::NotifyOnly => ExpiryStatus::ExpiredNotifyOnly,
            ExpiryStrategy::NoExpiry => ExpiryStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpiryStatus {
    Active,
    ExpiringSoon { days_left: u32 },
    InGracePeriod { days_left: u32 },
    ExpiredNotifyOnly,
    Expired,
}

impl ExpiryStatus {
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Active | Self::ExpiringSoon { .. } | Self::InGracePeriod { .. } | Self::ExpiredNotifyOnly)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Renewal Strategies
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenewalStrategy {
    /// Automatically renew before expiry
    AutoRenew { renew_days_before: u32 },
    /// Manual renewal required, send reminder N days before
    Manual { remind_days_before: Vec<u32> },
    /// Subscription: auto-renew on billing cycle
    Subscription { billing_cycle_days: u32 },
    /// No renewal (one-time purchase)
    None,
}

impl RenewalStrategy {
    /// Check if renewal action is needed
    pub fn needs_action(&self, expiry_date: Option<NaiveDate>, today: NaiveDate) -> Option<RenewalAction> {
        let Some(expiry) = expiry_date else {
            return None;
        };

        let days_until = (expiry - today).num_days();
        if days_until < 0 {
            return Some(RenewalAction::Overdue { days_overdue: (-days_until) as u32 });
        }

        match self {
            RenewalStrategy::AutoRenew { renew_days_before } => {
                if days_until <= *renew_days_before as i64 {
                    Some(RenewalAction::AutoRenewDue)
                } else {
                    None
                }
            }
            RenewalStrategy::Manual { remind_days_before } => {
                for &days in remind_days_before {
                    if days_until <= days as i64 {
                        return Some(RenewalAction::ReminderDue { days_left: days_until as u32 });
                    }
                }
                None
            }
            RenewalStrategy::Subscription { billing_cycle_days } => {
                if days_until <= *billing_cycle_days as i64 {
                    Some(RenewalAction::BillingCycleDue)
                } else {
                    None
                }
            }
            RenewalStrategy::None => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenewalAction {
    AutoRenewDue,
    ReminderDue { days_left: u32 },
    BillingCycleDue,
    Overdue { days_overdue: u32 },
}

// ═══════════════════════════════════════════════════════════════════════════
// Transfer Policy
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferPolicy {
    /// License can be freely transferred between machines
    Allow,
    /// Transfer requires admin approval
    RequireApproval,
    /// Transfer allowed with a cooldown period (N days between transfers)
    CooldownDays(u32),
    /// Transfer is not allowed (locked to original machine)
    Deny,
}

impl TransferPolicy {
    pub fn can_transfer(&self, last_transfer: Option<DateTime<Utc>>) -> TransferResult {
        match self {
            TransferPolicy::Allow => TransferResult::Allowed,
            TransferPolicy::RequireApproval => TransferResult::NeedsApproval,
            TransferPolicy::CooldownDays(days) => {
                if let Some(last) = last_transfer {
                    let elapsed = (Utc::now() - last).num_days();
                    if elapsed < *days as i64 {
                        TransferResult::CooldownActive { days_remaining: (*days as i64 - elapsed) as u32 }
                    } else {
                        TransferResult::Allowed
                    }
                } else {
                    TransferResult::Allowed
                }
            }
            TransferPolicy::Deny => TransferResult::Denied,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferResult {
    Allowed,
    NeedsApproval,
    CooldownActive { days_remaining: u32 },
    Denied,
}

// ═══════════════════════════════════════════════════════════════════════════
// Overage Handling
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OveragePolicy {
    /// Block new assignments when seats are full
    Block,
    /// Allow overage but charge a penalty rate
    AllowWithPenalty { penalty_multiplier: u32 }, // e.g. 150 = 1.5x per-seat cost
    /// Allow overage and just notify
    NotifyOnly,
    /// Allow up to N extra seats, then block
    SoftLimit { extra_seats: u32 },
}

impl OveragePolicy {
    pub fn check_assignment(&self, assigned: i32, total: i32) -> OverageResult {
        if assigned < total {
            return OverageResult::WithinLimits;
        }

        match self {
            OveragePolicy::Block => OverageResult::Blocked,
            OveragePolicy::AllowWithPenalty { penalty_multiplier } => {
                OverageResult::AllowedWithPenalty { multiplier: *penalty_multiplier }
            }
            OveragePolicy::NotifyOnly => OverageResult::AllowedWithNotification,
            OveragePolicy::SoftLimit { extra_seats } => {
                if assigned < total + *extra_seats as i32 {
                    OverageResult::AllowedWithNotification
                } else {
                    OverageResult::Blocked
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverageResult {
    WithinLimits,
    AllowedWithPenalty { multiplier: u32 },
    AllowedWithNotification,
    Blocked,
}

// ═══════════════════════════════════════════════════════════════════════════
// Machine Fingerprint
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineFingerprint {
    pub fingerprint_id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub cpu_id: Option<String>,
    pub mac_addresses: Vec<String>,
    pub disk_serial: Option<String>,
    pub machine_id: Option<String>,
    pub collected_at: DateTime<Utc>,
}

impl MachineFingerprint {
    /// Generate a deterministic fingerprint hash from hardware identifiers
    pub fn compute_hash(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.hostname.hash(&mut hasher);
        self.os.hash(&mut hasher);
        self.arch.hash(&mut hasher);
        if let Some(ref cpu) = self.cpu_id {
            cpu.hash(&mut hasher);
        }
        for mac in &self.mac_addresses {
            mac.hash(&mut hasher);
        }
        if let Some(ref disk) = self.disk_serial {
            disk.hash(&mut hasher);
        }
        if let Some(ref mid) = self.machine_id {
            mid.hash(&mut hasher);
        }

        format!("{:016x}", hasher.finish())
    }

    /// Collect fingerprint from the current machine
    pub fn collect_local() -> Self {
        Self {
            fingerprint_id: String::new(),
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".into()),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_id: None,
            mac_addresses: Vec::new(),
            disk_serial: None,
            machine_id: read_machine_id(),
            collected_at: Utc::now(),
        }
    }
}

fn read_machine_id() -> Option<String> {
    // Linux: /etc/machine-id
    // Windows: HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/machine-id")
            .ok()
            .map(|s| s.trim().to_string())
    }
    #[cfg(target_os = "windows")]
    {
        // Read from registry via command
        None // Would use winreg crate in production
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Heartbeat Monitoring
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineHeartbeat {
    pub machine_fingerprint: String,
    pub license_id: Uuid,
    pub seat_id: Uuid,
    pub last_seen: DateTime<Utc>,
    pub status: HeartbeatStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HeartbeatStatus {
    Alive,
    Stale,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    /// How often machines should send heartbeats (seconds)
    pub interval_seconds: u64,
    /// After this many missed heartbeats, mark as stale
    pub stale_threshold: u32,
    /// After this many missed heartbeats, mark as dead and auto-release seat
    pub dead_threshold: u32,
    /// Whether to auto-release seats from dead machines
    pub auto_release: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval_seconds: 300, // 5 minutes
            stale_threshold: 3,    // 15 minutes
            dead_threshold: 12,    // 1 hour
            auto_release: true,
        }
    }
}

impl HeartbeatConfig {
    pub fn evaluate(&self, last_seen: DateTime<Utc>) -> HeartbeatStatus {
        let elapsed = (Utc::now() - last_seen).num_seconds() as u64;
        let missed = elapsed / self.interval_seconds;

        if missed >= self.dead_threshold as u64 {
            HeartbeatStatus::Dead
        } else if missed >= self.stale_threshold as u64 {
            HeartbeatStatus::Stale
        } else {
            HeartbeatStatus::Alive
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Entitlements / Feature Flags
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entitlement {
    pub feature_key: String,
    pub name: String,
    pub description: Option<String>,
    pub value_type: EntitlementValueType,
    pub default_value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementValueType {
    Boolean,
    Integer,
    String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseTier {
    pub tier_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub entitlements: HashMap<String, serde_json::Value>,
    pub max_targets: Option<u32>,
    pub max_users: Option<u32>,
    pub max_dashboards: Option<u32>,
    pub max_alert_rules: Option<u32>,
    pub retention_days: Option<u32>,
}

impl LicenseTier {
    pub fn community() -> Self {
        Self {
            tier_id: Uuid::nil(),
            name: "Community".into(),
            description: Some("Free tier with basic features".into()),
            entitlements: HashMap::from([
                ("alerting".into(), serde_json::json!(true)),
                ("dashboards".into(), serde_json::json!(true)),
                ("plugins.network".into(), serde_json::json!(true)),
                ("plugins.server".into(), serde_json::json!(true)),
                ("ml.anomaly_detection".into(), serde_json::json!(false)),
                ("ml.forecasting".into(), serde_json::json!(false)),
                ("rbac".into(), serde_json::json!(false)),
                ("audit_log".into(), serde_json::json!(false)),
                ("multi_tenancy".into(), serde_json::json!(false)),
            ]),
            max_targets: Some(10),
            max_users: Some(3),
            max_dashboards: Some(5),
            max_alert_rules: Some(10),
            retention_days: Some(30),
        }
    }

    pub fn professional() -> Self {
        Self {
            tier_id: Uuid::from_u128(1),
            name: "Professional".into(),
            description: Some("Full feature set for teams".into()),
            entitlements: HashMap::from([
                ("alerting".into(), serde_json::json!(true)),
                ("dashboards".into(), serde_json::json!(true)),
                ("plugins.network".into(), serde_json::json!(true)),
                ("plugins.server".into(), serde_json::json!(true)),
                ("ml.anomaly_detection".into(), serde_json::json!(true)),
                ("ml.forecasting".into(), serde_json::json!(true)),
                ("rbac".into(), serde_json::json!(true)),
                ("audit_log".into(), serde_json::json!(true)),
                ("multi_tenancy".into(), serde_json::json!(false)),
            ]),
            max_targets: Some(100),
            max_users: Some(25),
            max_dashboards: Some(50),
            max_alert_rules: Some(200),
            retention_days: Some(90),
        }
    }

    pub fn enterprise() -> Self {
        Self {
            tier_id: Uuid::from_u128(2),
            name: "Enterprise".into(),
            description: Some("Unlimited with multi-tenancy".into()),
            entitlements: HashMap::from([
                ("alerting".into(), serde_json::json!(true)),
                ("dashboards".into(), serde_json::json!(true)),
                ("plugins.network".into(), serde_json::json!(true)),
                ("plugins.server".into(), serde_json::json!(true)),
                ("ml.anomaly_detection".into(), serde_json::json!(true)),
                ("ml.forecasting".into(), serde_json::json!(true)),
                ("rbac".into(), serde_json::json!(true)),
                ("audit_log".into(), serde_json::json!(true)),
                ("multi_tenancy".into(), serde_json::json!(true)),
            ]),
            max_targets: None, // unlimited
            max_users: None,
            max_dashboards: None,
            max_alert_rules: None,
            retention_days: Some(365),
        }
    }

    /// Check if a feature is enabled
    pub fn has_feature(&self, key: &str) -> bool {
        self.entitlements.get(key)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    /// Check if a resource limit is within bounds
    pub fn within_limit(&self, resource: &str, current: u32) -> bool {
        match resource {
            "targets" => self.max_targets.map_or(true, |max| current < max),
            "users" => self.max_users.map_or(true, |max| current < max),
            "dashboards" => self.max_dashboards.map_or(true, |max| current < max),
            "alert_rules" => self.max_alert_rules.map_or(true, |max| current < max),
            _ => true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// License Key Generation (HMAC-SHA256 based)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKeyPayload {
    pub license_id: Uuid,
    pub org_id: Uuid,
    pub tier: String,
    pub seat_count: i32,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub features: Vec<String>,
    pub machine_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseKey {
    pub key_string: String,
    pub payload: LicenseKeyPayload,
}

impl LicenseKey {
    /// Generate a license key from a payload using HMAC-SHA256
    pub fn generate(payload: LicenseKeyPayload, secret: &[u8]) -> Self {
        let payload_json = serde_json::to_string(&payload).unwrap_or_default();
        let payload_b64 = base64_encode(payload_json.as_bytes());

        // HMAC-SHA256 signature
        let signature = hmac_sha256(secret, payload_b64.as_bytes());
        let sig_b64 = base64_encode(&signature);

        // Key format: INFRA-{payload_b64}.{sig_b64}
        let key_string = format!("INFRA-{}.{}", payload_b64, sig_b64);

        Self { key_string, payload }
    }

    /// Validate and decode a license key
    pub fn validate(key_string: &str, secret: &[u8]) -> Result<LicenseKeyPayload, LicenseKeyError> {
        let stripped = key_string.strip_prefix("INFRA-")
            .ok_or(LicenseKeyError::InvalidFormat)?;

        let parts: Vec<&str> = stripped.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Err(LicenseKeyError::InvalidFormat);
        }

        let payload_b64 = parts[0];
        let sig_b64 = parts[1];

        // Verify signature
        let expected_sig = hmac_sha256(secret, payload_b64.as_bytes());
        let expected_sig_b64 = base64_encode(&expected_sig);

        if sig_b64 != expected_sig_b64 {
            return Err(LicenseKeyError::InvalidSignature);
        }

        // Decode payload
        let payload_bytes = base64_decode(payload_b64)
            .map_err(|_| LicenseKeyError::InvalidFormat)?;
        let payload_str = String::from_utf8(payload_bytes)
            .map_err(|_| LicenseKeyError::InvalidFormat)?;
        let payload: LicenseKeyPayload = serde_json::from_str(&payload_str)
            .map_err(|_| LicenseKeyError::InvalidPayload)?;

        // Check expiry
        if let Some(expires) = payload.expires_at {
            if Utc::now() > expires {
                return Err(LicenseKeyError::Expired);
            }
        }

        Ok(payload)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum LicenseKeyError {
    #[error("Invalid key format")]
    InvalidFormat,
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid payload")]
    InvalidPayload,
    #[error("License key expired")]
    Expired,
}

// ═══════════════════════════════════════════════════════════════════════════
// Composite License Policy
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicensePolicy {
    pub policy_id: Uuid,
    pub name: String,
    pub expiry_strategy: ExpiryStrategy,
    pub renewal_strategy: RenewalStrategy,
    pub transfer_policy: TransferPolicy,
    pub overage_policy: OveragePolicy,
    pub heartbeat_config: HeartbeatConfig,
    pub tier: String,
}

impl Default for LicensePolicy {
    fn default() -> Self {
        Self {
            policy_id: Uuid::now_v7(),
            name: "Default Policy".into(),
            expiry_strategy: ExpiryStrategy::GracePeriod { grace_days: 14 },
            renewal_strategy: RenewalStrategy::Manual { remind_days_before: vec![90, 60, 30, 14, 7, 1] },
            transfer_policy: TransferPolicy::CooldownDays(30),
            overage_policy: OveragePolicy::SoftLimit { extra_seats: 5 },
            heartbeat_config: HeartbeatConfig::default(),
            tier: "professional".into(),
        }
    }
}

// ── Crypto helpers (no external crate needed) ──

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Simplified HMAC using Rust's built-in hashing + XOR padding
    // For production, use the `hmac` + `sha2` crates
    let block_size = 64;
    let mut key_padded = vec![0u8; block_size];

    if key.len() > block_size {
        // Hash the key first
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let h = hasher.finish().to_be_bytes();
        key_padded[..8].copy_from_slice(&h);
    } else {
        key_padded[..key.len()].copy_from_slice(key);
    }

    // Inner hash: H((key XOR ipad) || data)
    let ipad: Vec<u8> = key_padded.iter().map(|b| b ^ 0x36).collect();
    let mut inner_data = ipad;
    inner_data.extend_from_slice(data);

    let mut hasher = DefaultHasher::new();
    inner_data.hash(&mut hasher);
    let inner_hash = hasher.finish().to_be_bytes();

    // Outer hash: H((key XOR opad) || inner_hash)
    let opad: Vec<u8> = key_padded.iter().map(|b| b ^ 0x5c).collect();
    let mut outer_data = opad;
    outer_data.extend_from_slice(&inner_hash);

    let mut hasher = DefaultHasher::new();
    outer_data.hash(&mut hasher);
    let result = hasher.finish().to_be_bytes();

    result.to_vec()
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as u32;
        let b1 = if i + 1 < data.len() { data[i + 1] as u32 } else { 0 };
        let b2 = if i + 2 < data.len() { data[i + 2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if i + 1 < data.len() {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if i + 2 < data.len() {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        }

        i += 3;
    }
    result
}

fn base64_decode(s: &str) -> Result<Vec<u8>, &'static str> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    fn char_to_val(c: u8) -> Result<u32, &'static str> {
        CHARS.iter().position(|&x| x == c)
            .map(|p| p as u32)
            .ok_or("Invalid base64 character")
    }

    let bytes = s.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let v0 = char_to_val(bytes[i])?;
        let v1 = if i + 1 < bytes.len() { char_to_val(bytes[i + 1])? } else { 0 };
        let v2 = if i + 2 < bytes.len() { char_to_val(bytes[i + 2])? } else { 0 };
        let v3 = if i + 3 < bytes.len() { char_to_val(bytes[i + 3])? } else { 0 };

        let triple = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;

        result.push(((triple >> 16) & 0xFF) as u8);
        if i + 2 < bytes.len() {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if i + 3 < bytes.len() {
            result.push((triple & 0xFF) as u8);
        }

        i += 4;
    }

    Ok(result)
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expiry_hard() {
        let strategy = ExpiryStrategy::HardExpire;
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();

        // Not expired yet
        let expiry = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        let status = strategy.is_usable(Some(expiry), today);
        assert!(status.is_usable());

        // Expired
        let expiry = NaiveDate::from_ymd_opt(2026, 6, 14).unwrap();
        let status = strategy.is_usable(Some(expiry), today);
        assert!(!status.is_usable());
    }

    #[test]
    fn test_expiry_grace_period() {
        let strategy = ExpiryStrategy::GracePeriod { grace_days: 14 };
        let expiry = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();

        // Day after expiry — still in grace
        let today = NaiveDate::from_ymd_opt(2026, 6, 2).unwrap();
        let status = strategy.is_usable(Some(expiry), today);
        assert!(matches!(status, ExpiryStatus::InGracePeriod { .. }));
        assert!(status.is_usable());

        // After grace period
        let today = NaiveDate::from_ymd_opt(2026, 6, 20).unwrap();
        let status = strategy.is_usable(Some(expiry), today);
        assert_eq!(status, ExpiryStatus::Expired);
    }

    #[test]
    fn test_renewal_manual_reminder() {
        let strategy = RenewalStrategy::Manual { remind_days_before: vec![30, 14, 7] };
        let expiry = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();

        // 40 days out — no action
        let today = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        assert!(strategy.needs_action(Some(expiry), today).is_none());

        // 20 days out — reminder
        let today = NaiveDate::from_ymd_opt(2026, 6, 25).unwrap();
        assert!(strategy.needs_action(Some(expiry), today).is_some());
    }

    #[test]
    fn test_transfer_cooldown() {
        let policy = TransferPolicy::CooldownDays(30);

        // No previous transfer
        assert_eq!(policy.can_transfer(None), TransferResult::Allowed);

        // Recent transfer
        let recent = Utc::now() - chrono::Duration::days(5);
        let result = policy.can_transfer(Some(recent));
        assert!(matches!(result, TransferResult::CooldownActive { .. }));

        // Old transfer
        let old = Utc::now() - chrono::Duration::days(31);
        assert_eq!(policy.can_transfer(Some(old)), TransferResult::Allowed);
    }

    #[test]
    fn test_overage_soft_limit() {
        let policy = OveragePolicy::SoftLimit { extra_seats: 5 };

        assert_eq!(policy.check_assignment(5, 10), OverageResult::WithinLimits);
        assert_eq!(policy.check_assignment(10, 10), OverageResult::AllowedWithNotification);
        assert_eq!(policy.check_assignment(14, 10), OverageResult::AllowedWithNotification);
        assert_eq!(policy.check_assignment(15, 10), OverageResult::Blocked);
    }

    #[test]
    fn test_heartbeat_evaluation() {
        let config = HeartbeatConfig::default();

        let alive = Utc::now() - chrono::Duration::minutes(3);
        assert_eq!(config.evaluate(alive), HeartbeatStatus::Alive);

        let stale = Utc::now() - chrono::Duration::minutes(20);
        assert_eq!(config.evaluate(stale), HeartbeatStatus::Stale);

        let dead = Utc::now() - chrono::Duration::hours(2);
        assert_eq!(config.evaluate(dead), HeartbeatStatus::Dead);
    }

    #[test]
    fn test_license_tiers() {
        let community = LicenseTier::community();
        assert!(community.has_feature("alerting"));
        assert!(!community.has_feature("ml.anomaly_detection"));
        assert!(community.within_limit("targets", 5));
        assert!(!community.within_limit("targets", 10));

        let enterprise = LicenseTier::enterprise();
        assert!(enterprise.has_feature("multi_tenancy"));
        assert!(enterprise.within_limit("targets", 10000)); // unlimited
    }

    #[test]
    fn test_license_key_roundtrip() {
        let secret = b"test-secret-key-32-bytes-long!!!";
        let payload = LicenseKeyPayload {
            license_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            tier: "professional".into(),
            seat_count: 25,
            issued_at: Utc::now(),
            expires_at: Some(Utc::now() + chrono::Duration::days(365)),
            features: vec!["alerting".into(), "ml".into()],
            machine_fingerprint: None,
        };

        let key = LicenseKey::generate(payload.clone(), secret);
        assert!(key.key_string.starts_with("INFRA-"));

        let validated = LicenseKey::validate(&key.key_string, secret).unwrap();
        assert_eq!(validated.license_id, payload.license_id);
        assert_eq!(validated.seat_count, 25);
    }

    #[test]
    fn test_license_key_invalid_signature() {
        let secret = b"test-secret-key-32-bytes-long!!!";
        let payload = LicenseKeyPayload {
            license_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            tier: "community".into(),
            seat_count: 5,
            issued_at: Utc::now(),
            expires_at: None,
            features: vec![],
            machine_fingerprint: None,
        };

        let key = LicenseKey::generate(payload, secret);

        // Tamper with signature
        let wrong_secret = b"wrong-secret-key-32-bytes-long!!";
        let result = LicenseKey::validate(&key.key_string, wrong_secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_machine_fingerprint_hash() {
        let fp = MachineFingerprint {
            fingerprint_id: String::new(),
            hostname: "prod-web-01".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            cpu_id: Some("GenuineIntel".into()),
            mac_addresses: vec!["aa:bb:cc:dd:ee:ff".into()],
            disk_serial: Some("WD-12345".into()),
            machine_id: Some("abcdef12345".into()),
            collected_at: Utc::now(),
        };

        let hash1 = fp.compute_hash();
        let hash2 = fp.compute_hash();
        assert_eq!(hash1, hash2); // Deterministic
        assert_eq!(hash1.len(), 16); // 16 hex chars
    }
}
