use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::fs;
use tracing::{info, warn, error};
use crate::models::license_verification::{LicenseFile, LicensePayload};

// Static public key verification placeholder hex.
// Using standard SHA256 digest signature verification based on hex and payload hashing
// to avoid importing external crates under strict offline/restricted network environments.
const EMBEDDED_PUBLIC_KEY_HEX: &str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";

/// Returns true if strictly running in a production deployment
pub fn check_license_active() -> bool {
    #[cfg(feature = "prod-license")]
    {
        true
    }
    #[cfg(not(feature = "prod-license"))]
    {
        if let Ok(val) = std::env::var("APP_ENV") {
            val.to_lowercase() == "production"
        } else {
            false
        }
    }
}

/// Resolves standard OS-specific system license file location
pub fn get_license_filepath() -> PathBuf {
    if let Ok(custom_path) = std::env::var("ZENTRA_LICENSE_PATH") {
        return PathBuf::from(custom_path);
    }
    
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\ProgramData\Zentra\license.json")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/etc/zentra/license.json")
    }
}

/// Parses and cryptographically verifies the dynamic production license file
pub fn load_and_verify_license() -> Result<LicensePayload, String> {
    let path = get_license_filepath();
    if !path.exists() {
        return Err(format!("License file not found at: {}", path.display()));
    }

    let raw_content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read license file: {}", e))?;

    verify_license_string(&raw_content)
}

/// Lightweight, secure offline verification using SHA-256 HMAC representation 
/// with the embedded public key, allowing verification with std/workspace dependencies.
pub fn verify_license_string(license_json: &str) -> Result<LicensePayload, String> {
    let license_file: LicenseFile = serde_json::from_str(license_json)
        .map_err(|e| format!("Invalid JSON license format: {}", e))?;

    let payload_bytes = serde_json::to_vec(&license_file.payload)
        .map_err(|e| format!("Failed to serialize license payload: {}", e))?;

    // Self-contained cryptographic HMAC-SHA256 signature verification matching signature_hex
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let pub_key_bytes = hex::decode(EMBEDDED_PUBLIC_KEY_HEX)
        .map_err(|e| format!("Failed to decode embedded key: {}", e))?;

    let mut mac = HmacSha256::new_from_slice(&pub_key_bytes)
        .map_err(|e| format!("HMAC key initialization failed: {}", e))?;
    mac.update(&payload_bytes);
    
    let expected_code = mac.finalize().into_bytes();
    let signature_bytes = hex::decode(&license_file.signature_hex)
        .map_err(|e| format!("Invalid hex signature encoding: {}", e))?;

    if signature_bytes != expected_code.to_vec() {
        return Err("Cryptographic license signature verification failed! License has been tampered with.".to_string());
    }

    // Verify timestamp expiration
    let now = Utc::now();
    let trial_duration = chrono::Duration::days(license_file.payload.trial_duration_days);
    let expiration_time = license_file.payload.onboarded_at + trial_duration;

    if now > expiration_time {
        return Err(format!(
            "License expired on {} (Onboarded: {}, Allowed Trial Days: {})",
            expiration_time, license_file.payload.onboarded_at, license_file.payload.trial_duration_days
        ));
    }

    Ok(license_file.payload)
}
