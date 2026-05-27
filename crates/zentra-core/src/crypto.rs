//! AES-256-GCM envelope encryption for credentials at rest.
//!
//! Master key is read from `ZENTRA_MASTER_KEY` (hex-encoded, 32 bytes).
//! Each encrypt call generates a random 12-byte nonce. Ciphertext is stored as
//! `base64(nonce || ciphertext || tag)`.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::RngCore;

pub mod license_verifier;

/// Minimum expected length: 12-byte nonce + 16-byte tag = 28 bytes decoded.
const MIN_DECODED_LEN: usize = 12 + 16;

/// Default insecure key for local development only (32 bytes, hex = 64 chars).
const DEV_KEY_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";

// ═══════════════════════════════════════════════════════════════════════════
// Key management
// ═══════════════════════════════════════════════════════════════════════════

fn master_key_bytes() -> Result<[u8; 32], String> {
    let hex = match std::env::var("ZENTRA_MASTER_KEY") {
        Ok(val) if !val.trim().is_empty() => val,
        _ => {
            // Allow dev key only if explicitly opted-in for local development
            if std::env::var("ZENTRA_ALLOW_INSECURE_JWT").as_deref() == Ok("1") {
                tracing::warn!("ZENTRA_MASTER_KEY not set — using insecure dev key. DO NOT use in production.");
                DEV_KEY_HEX.to_string()
            } else {
                return Err(
                    "ZENTRA_MASTER_KEY is required in production. Set a 64-char hex string (32 bytes). \
                     To bypass for local dev, set ZENTRA_ALLOW_INSECURE_JWT=1"
                        .to_string(),
                );
            }
        }
    };
    let decoded = hex::decode(hex.trim()).map_err(|e| format!("bad ZENTRA_MASTER_KEY hex: {e}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "ZENTRA_MASTER_KEY must be 32 bytes (64 hex chars), got {} bytes",
            decoded.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

// ═══════════════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════════════

/// Encrypt a plaintext string → base64(nonce ‖ ciphertext ‖ tag).
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    let key = master_key_bytes()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher init: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("encrypt: {e}"))?;

    let mut blob = Vec::with_capacity(12 + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);

    Ok(B64.encode(&blob))
}

/// Decrypt a base64(nonce ‖ ciphertext ‖ tag) → plaintext string.
pub fn decrypt(encoded: &str) -> Result<String, String> {
    if encoded.is_empty() {
        return Ok(String::new());
    }
    let blob = B64.decode(encoded).map_err(|e| format!("base64 decode: {e}"))?;
    if blob.len() < MIN_DECODED_LEN {
        return Err("ciphertext too short".into());
    }
    let key = master_key_bytes()?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher init: {e}"))?;

    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "decryption failed — wrong key or corrupted data".to_string())?;

    String::from_utf8(plaintext).map_err(|e| format!("utf8: {e}"))
}

/// Check if a string looks like it's already encrypted (base64 with valid structure).
pub fn is_encrypted(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    match B64.decode(value) {
        Ok(blob) => blob.len() >= MIN_DECODED_LEN,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        std::env::set_var("ZENTRA_MASTER_KEY", DEV_KEY_HEX);
        let secret = "my-database-password-123!";
        let enc = encrypt(secret).unwrap();
        assert_ne!(enc, secret);
        assert!(is_encrypted(&enc));
        let dec = decrypt(&enc).unwrap();
        assert_eq!(dec, secret);
    }

    #[test]
    fn empty_round_trip() {
        let enc = encrypt("").unwrap();
        assert_eq!(enc, "");
        let dec = decrypt("").unwrap();
        assert_eq!(dec, "");
    }

    #[test]
    fn different_nonces() {
        std::env::set_var("ZENTRA_MASTER_KEY", DEV_KEY_HEX);
        let a = encrypt("same").unwrap();
        let b = encrypt("same").unwrap();
        assert_ne!(a, b); // different nonce each time
        assert_eq!(decrypt(&a).unwrap(), "same");
        assert_eq!(decrypt(&b).unwrap(), "same");
    }
}
