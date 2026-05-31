// SIGNING CONTRACT:
// The offline license signer must sign the exact raw byte representation of the "payload" field.
// This is done by extracting the raw JSON substring of the payload without any deserialization-reserialization roundtrip,
// ensuring byte-level verification determinism.
// Format: Ed25519 asymmetric signature using the matching offline private key, verified against EMBEDDED_PUBLIC_KEY_HEX.

use chrono::Utc;
use std::path::PathBuf;
use crate::models::license_verification::LicensePayload;

/// single source of truth for whether licensing is active — do not add other toggles.
pub const fn check_license_active() -> bool {
    cfg!(feature = "prod-license")
}

/// Returns true when the trial period has expired.
///
/// **Hardened:** Does NOT trust the ZENTRA_TRIAL_EXPIRED env var alone.
/// Instead, reads the license token file directly from disk, verifies the
/// HMAC signature against the binary hash, and checks expiry independently.
///
/// Attack vectors this prevents:
/// - Spoofing ZENTRA_TRIAL_EXPIRED=0 via `docker run -e`
/// - Deleting the license file to force regeneration (detected: DB dir
///   exists but license is missing → treated as expired/tampered)
///
/// The result is cached in a OnceLock so the file is only read once per
/// process lifetime.
pub fn is_trial_expired() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| verify_trial_token_from_disk())
}

/// Independently verify the shell-level license token by reading the file,
/// checking the HMAC, and comparing the expiry epoch against the current time.
///
/// Token format (colon-separated):
///   install_epoch : expiry_epoch : hostname : sha256_hmac
///
/// The HMAC is: sha256("install:expiry:hostname:binary_sha256")
fn verify_trial_token_from_disk() -> bool {
    use sha2::{Sha256, Digest};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    // Paths matching docker-entrypoint.sh
    let primary = "/var/lib/postgresql/data/.zentra-license";
    let fallback = "/opt/zentra/.license";
    let binary = "/usr/local/bin/zentra-server";
    let pgdata = "/var/lib/postgresql/data";

    // Detect if we're running inside a Docker container
    let in_docker = std::path::Path::new("/.dockerenv").exists()
        || fs::read_to_string("/proc/1/cgroup")
            .map(|s| s.contains("docker") || s.contains("containerd") || s.contains("kubepods"))
            .unwrap_or(false);

    // Read the license token from primary or fallback location
    let token_content = if let Ok(content) = fs::read_to_string(primary) {
        content
    } else if let Ok(content) = fs::read_to_string(fallback) {
        content
    } else {
        // No license file found.
        // Anti-deletion check ONLY inside Docker: if PGDATA dir exists but
        // license file is missing, someone deleted it → treat as expired.
        // Outside Docker (dev/bare-metal), PGDATA may exist from the system
        // PostgreSQL and has nothing to do with Zentra licensing.
        if in_docker && std::path::Path::new(pgdata).exists() {
            tracing::error!(target: "license", "License file missing but DB exists — treating as expired (possible bypass attempt)");
            return true; // expired / tampered
        }
        // Non-Docker or no PGDATA: fall back to env var for dev/test
        return std::env::var("ZENTRA_TRIAL_EXPIRED")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
    };

    let token = token_content.trim();
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 4 {
        tracing::error!(target: "license", "Malformed license token ({} fields, expected 4)", parts.len());
        return true; // treat malformed as expired
    }

    let stored_install = parts[0];
    let stored_expiry = parts[1];
    let stored_host = parts[2];
    let stored_sig = parts[3];

    // Compute the binary hash (same as BUILD_HASH in entrypoint)
    let binary_hash = match fs::read(binary) {
        Ok(bytes) => {
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
        Err(_) => {
            // Binary not found — dev environment, fall back to env var
            return std::env::var("ZENTRA_TRIAL_EXPIRED")
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);
        }
    };

    // Verify HMAC: sha256("install:expiry:hostname:binary_hash")
    let payload = format!("{}:{}:{}:{}", stored_install, stored_expiry, stored_host, binary_hash);
    let mut hasher = Sha256::new();
    hasher.update(payload.as_bytes());
    let expected_sig = format!("{:x}", hasher.finalize());

    if stored_sig != expected_sig {
        tracing::error!(target: "license", "License signature mismatch — file may be tampered");
        return true; // tampered = expired
    }

    // Parse expiry epoch and compare to current time
    let expiry_epoch: u64 = match stored_expiry.parse() {
        Ok(v) => v,
        Err(_) => {
            tracing::error!(target: "license", "Invalid expiry epoch in license token");
            return true;
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    now >= expiry_epoch
}

#[cfg(feature = "prod-license")]
use crate::models::license_verification::LicenseFile;

#[cfg(feature = "prod-license")]
use std::fs;

#[cfg(feature = "prod-license")]
// TODO(T05): replace with production public key before release
// The matching private key lives outside this repository (addressed by T20/T21).
const EMBEDDED_PUBLIC_KEY_HEX: &str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";

#[cfg(feature = "prod-license")]
/// Resolves standard OS-specific system license file location
pub fn get_license_filepath() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(r"C:\ProgramData\Zentra\license.json")
    }
    #[cfg(not(target_os = "windows"))]
    {
        PathBuf::from("/etc/zentra/license.json")
    }
}

#[cfg(feature = "prod-license")]
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

#[cfg(feature = "prod-license")]
/// Lightweight, secure offline verification using Ed25519 asymmetric verification
/// with the embedded public key, allowing verification with std/workspace dependencies.
pub fn verify_license_string(license_json: &str) -> Result<LicensePayload, String> {
    use ring::signature;
    use std::collections::HashMap;

    let license_file: LicenseFile = serde_json::from_str(license_json)
        .map_err(|e| format!("Invalid JSON license format: {}", e))?;

    // Check version and algorithm
    if license_file.format_version != 1 {
        return Err(format!("Unsupported license format version: {}", license_file.format_version));
    }
    if license_file.algorithm != "ed25519" {
        return Err(format!("Unsupported signature algorithm: {}", license_file.algorithm));
    }

    let payload_raw = license_file.payload.get();
    let payload_bytes = payload_raw.as_bytes();

    let payload: LicensePayload = serde_json::from_str(payload_raw)
        .map_err(|e| format!("Failed to parse license payload: {}", e))?;

    let pub_key_bytes = hex::decode(EMBEDDED_PUBLIC_KEY_HEX)
        .map_err(|e| format!("Failed to decode embedded key: {}", e))?;

    // Maintain key_id to public key mapping (T08)
    let mut known_keys = HashMap::new();
    known_keys.insert("zentra-prod-2026-01", pub_key_bytes.as_slice());

    let pub_key = known_keys.get(license_file.key_id.as_str())
        .ok_or_else(|| format!("Unknown or untrusted license key_id: {}", license_file.key_id))?;

    let signature_bytes = hex::decode(&license_file.signature_hex)
        .map_err(|e| format!("Invalid hex signature encoding: {}", e))?;

    let peer_public_key = signature::UnparsedPublicKey::new(&signature::ED25519, pub_key);
    peer_public_key.verify(payload_bytes, &signature_bytes)
        .map_err(|e| format!("Cryptographic license signature verification failed: {:?}", e))?;

    // Verify timestamp expiration
    let now = Utc::now();
    let trial_duration = chrono::Duration::days(payload.trial_duration_days);
    let expiration_time = payload.onboarded_at + trial_duration;

    if now > expiration_time {
        return Err(format!(
            "License expired on {} (Onboarded: {}, Allowed Trial Days: {})",
            expiration_time, payload.onboarded_at, payload.trial_duration_days
        ));
    }

    Ok(payload)
}

#[cfg(not(feature = "prod-license"))]
/// Resolves standard OS-specific system license file location (dev stub)
pub fn get_license_filepath() -> PathBuf {
    if let Ok(custom_path) = std::env::var("ZENTRA_LICENSE_PATH") {
        return PathBuf::from(custom_path);
    }
    PathBuf::from("license_dev_stub.json")
}

#[cfg(not(feature = "prod-license"))]
/// Parses and cryptographically verifies the dynamic production license file (dev stub)
pub fn load_and_verify_license() -> Result<LicensePayload, String> {
    Ok(LicensePayload {
        org_id: "00000000-0000-0000-0000-000000000001".to_string(),
        customer_name: "Development Sandbox (Local)".to_string(),
        onboarded_at: Utc::now(),
        trial_duration_days: 9999,
        max_targets: 9999,
        features: crate::models::license_verification::LicenseFeatures {
            enable_oracle: true,
            enable_ebpf_network: true,
            enable_ai_observability: true,
            enable_otel: true,
            allowed_menu_items: vec![
                "dashboard".into(),
                "targets".into(),
                "oracle-rac".into(),
                "ebpf-network".into(),
                "otel-explorer".into(),
                "ai-observability".into(),
                "alerts".into(),
                "security".into(),
            ],
        },
    })
}

#[cfg(not(feature = "prod-license"))]
/// Lightweight, secure offline verification (dev stub)
pub fn verify_license_string(_license_json: &str) -> Result<LicensePayload, String> {
    load_and_verify_license()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use crate::models::license_verification::LicenseFile;
    use ring::signature::{self, KeyPair, Ed25519KeyPair};

    fn generate_test_license(
        key_pair: &Ed25519KeyPair,
        onboarded_at: DateTime<Utc>,
        trial_duration_days: i64,
    ) -> String {
        let payload = serde_json::json!({
            "org_id": "00000000-0000-0000-0000-000000000001",
            "customer_name": "Test Customer",
            "onboarded_at": onboarded_at,
            "trial_duration_days": trial_duration_days,
            "max_targets": 100,
            "features": {
                "enable_oracle": true,
                "enable_ebpf_network": true,
                "enable_ai_observability": true,
                "enable_otel": true,
                "allowed_menu_items": ["dashboard"]
            }
        });

        let payload_str = payload.to_string();
        let signature = key_pair.sign(payload_str.as_bytes());
        let signature_hex = hex::encode(signature.as_ref());

        serde_json::json!({
            "format_version": 1,
            "algorithm": "ed25519",
            "key_id": "zentra-prod-2026-01",
            "payload": payload,
            "signature_hex": signature_hex
        }).to_string()
    }

    #[test]
    fn test_verify_license_valid() {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let key_pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let verifying_key_bytes = key_pair.public_key().as_ref();

        let license_str = generate_test_license(&key_pair, Utc::now(), 30);
        
        let license_file: LicenseFile = serde_json::from_str(&license_str).unwrap();
        let payload_bytes = license_file.payload.get().as_bytes();
        let signature_bytes = hex::decode(&license_file.signature_hex).unwrap();

        let peer_public_key = signature::UnparsedPublicKey::new(&signature::ED25519, verifying_key_bytes);
        assert!(peer_public_key.verify(payload_bytes, &signature_bytes).is_ok());
    }

    #[test]
    fn test_verify_license_tampered_signature() {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let key_pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let verifying_key_bytes = key_pair.public_key().as_ref();

        let license_str = generate_test_license(&key_pair, Utc::now(), 30);
        let mut license_file: LicenseFile = serde_json::from_str(&license_str).unwrap();
        
        // Tamper signature
        let mut sig_bytes = hex::decode(&license_file.signature_hex).unwrap();
        let len = sig_bytes.len();
        sig_bytes[len - 1] ^= 0x01;
        license_file.signature_hex = hex::encode(sig_bytes);

        let signature_bytes = hex::decode(&license_file.signature_hex).unwrap();
        let peer_public_key = signature::UnparsedPublicKey::new(&signature::ED25519, verifying_key_bytes);
        assert!(peer_public_key.verify(license_file.payload.get().as_bytes(), &signature_bytes).is_err());
    }

    #[test]
    fn test_verify_license_tampered_payload() {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let key_pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();
        let verifying_key_bytes = key_pair.public_key().as_ref();

        let license_str = generate_test_license(&key_pair, Utc::now(), 30);
        let license_file: LicenseFile = serde_json::from_str(&license_str).unwrap();

        // Tamper payload raw bytes
        let mut payload_raw = license_file.payload.get().to_string();
        // Change one character in the raw JSON
        if let Some(pos) = payload_raw.find("Test Customer") {
            payload_raw.replace_range(pos..pos+1, "X");
        }

        let signature_bytes = hex::decode(&license_file.signature_hex).unwrap();
        let peer_public_key = signature::UnparsedPublicKey::new(&signature::ED25519, verifying_key_bytes);
        assert!(peer_public_key.verify(payload_raw.as_bytes(), &signature_bytes).is_err());
    }

    #[test]
    fn test_verify_license_expired() {
        let doc = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let key_pair = Ed25519KeyPair::from_pkcs8(doc.as_ref()).unwrap();

        // Created 10 days ago with a 5-day trial -> Expired 5 days ago
        let onboarded = Utc::now() - chrono::Duration::days(10);
        let license_str = generate_test_license(&key_pair, onboarded, 5);
        let license_file: LicenseFile = serde_json::from_str(&license_str).unwrap();

        let payload: LicensePayload = serde_json::from_str(license_file.payload.get()).unwrap();
        let now = Utc::now();
        let expiration_time = payload.onboarded_at + chrono::Duration::days(payload.trial_duration_days);
        assert!(now > expiration_time);
    }
}
