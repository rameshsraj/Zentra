//! Symmetric secret encryption for stored credentials.
//!
//! Wire format: `v1:<base64-nonce>:<base64-ciphertext>`.
//!
//! Reads its 32-byte key from `ZENTRA_SECRET_KEY` (hex or base64).
//! If the env is unset, [`SecretProvider::from_env`] returns a *passthrough* implementation
//! that logs a warning and stores plaintext — used only for local dev.
//!
//! Migration: a value that does NOT start with `v1:` is treated as legacy plaintext
//! by [`SecretProvider::decrypt`], so existing rows continue to work until they are re-saved.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::sync::Arc;
use tracing::warn;

const VERSION_PREFIX: &str = "v1:";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("invalid key length: expected 32 bytes")]
    InvalidKeyLength,
    #[error("base64 decode error: {0}")]
    Base64(String),
    #[error("hex decode error: {0}")]
    Hex(String),
    #[error("aead error: {0}")]
    Aead(String),
}

pub trait SecretProvider: Send + Sync {
    fn encrypt(&self, plain: &str) -> Result<String, SecretError>;
    fn decrypt(&self, stored: &str) -> Result<String, SecretError>;
}

/// AES-256-GCM provider.
pub struct AesGcmProvider {
    cipher: Aes256Gcm,
}

impl AesGcmProvider {
    pub fn new(key: [u8; 32]) -> Self {
        let key = Key::<Aes256Gcm>::from_slice(&key);
        Self { cipher: Aes256Gcm::new(key) }
    }

    /// Parse `ZENTRA_SECRET_KEY` env (hex or standard base64). Returns None if absent.
    pub fn from_env_var(name: &str) -> Option<Result<Self, SecretError>> {
        let v = std::env::var(name).ok()?;
        Some(Self::from_str(&v))
    }

    pub fn from_str(s: &str) -> Result<Self, SecretError> {
        let bytes = if let Ok(h) = hex::decode(s) {
            h
        } else if let Ok(b) = B64.decode(s) {
            b
        } else {
            return Err(SecretError::Hex("not hex or base64".into()));
        };
        if bytes.len() != 32 {
            return Err(SecretError::InvalidKeyLength);
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(Self::new(key))
    }
}

impl SecretProvider for AesGcmProvider {
    fn encrypt(&self, plain: &str) -> Result<String, SecretError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = self
            .cipher
            .encrypt(&nonce, plain.as_bytes())
            .map_err(|e| SecretError::Aead(e.to_string()))?;
        Ok(format!(
            "{}{}:{}",
            VERSION_PREFIX,
            B64.encode(nonce),
            B64.encode(ct)
        ))
    }

    fn decrypt(&self, stored: &str) -> Result<String, SecretError> {
        // Backward compat: legacy plaintext.
        let Some(rest) = stored.strip_prefix(VERSION_PREFIX) else {
            return Ok(stored.to_string());
        };
        let mut parts = rest.splitn(2, ':');
        let nonce_b64 = parts.next().unwrap_or("");
        let ct_b64 = parts.next().unwrap_or("");
        let nonce_bytes = B64
            .decode(nonce_b64)
            .map_err(|e| SecretError::Base64(e.to_string()))?;
        let ct = B64
            .decode(ct_b64)
            .map_err(|e| SecretError::Base64(e.to_string()))?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let pt = self
            .cipher
            .decrypt(nonce, ct.as_ref())
            .map_err(|e| SecretError::Aead(e.to_string()))?;
        String::from_utf8(pt).map_err(|e| SecretError::Aead(e.to_string()))
    }
}

/// Passthrough — emits a one-time warning. Used only for dev when no key is configured.
pub struct PassthroughProvider;

impl SecretProvider for PassthroughProvider {
    fn encrypt(&self, plain: &str) -> Result<String, SecretError> {
        warn!("PassthroughProvider in use — credentials stored as plaintext");
        Ok(plain.to_string())
    }
    fn decrypt(&self, stored: &str) -> Result<String, SecretError> {
        Ok(stored.to_string())
    }
}

/// Default factory used by callers — picks AES-GCM if key configured, else passthrough.
pub fn default_provider() -> Arc<dyn SecretProvider> {
    match AesGcmProvider::from_env_var("ZENTRA_SECRET_KEY") {
        Some(Ok(p)) => Arc::new(p) as _,
        Some(Err(e)) => {
            warn!(error = %e, "ZENTRA_SECRET_KEY parse failed — falling back to plaintext");
            Arc::new(PassthroughProvider)
        }
        None => {
            warn!("ZENTRA_SECRET_KEY not set — using PassthroughProvider (dev only)");
            Arc::new(PassthroughProvider)
        }
    }
}
