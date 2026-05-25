use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed SSL/TLS log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslLogEntry {
    pub timestamp: DateTime<Utc>,
    pub uid: Option<String>,
    pub src_ip: String,
    pub dest_ip: String,
    pub server_name: Option<String>,
    pub version: Option<String>,
    pub cipher: Option<String>,
    pub ja3: Option<String>,
    pub ja3s: Option<String>,
    pub cert_chain: Vec<serde_json::Value>,
    pub validation_status: Option<String>,
    pub not_valid_before: Option<DateTime<Utc>>,
    pub not_valid_after: Option<DateTime<Utc>>,
}

/// SSL analytics
#[derive(Debug, Clone, Serialize)]
pub struct SslAnalytics {
    pub total_connections: u64,
    pub version_dist: HashMap<String, u64>,
    pub cipher_dist: HashMap<String, u64>,
    pub ja3_fingerprints: Vec<(String, u64)>,
    pub expiring_certs: Vec<ExpiringCert>,
    pub weak_ciphers: Vec<WeakCipher>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpiringCert {
    pub server_name: String,
    pub not_valid_after: DateTime<Utc>,
    pub days_remaining: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeakCipher {
    pub cipher: String,
    pub server_name: String,
    pub count: u64,
}

/// Known weak ciphers
const WEAK_CIPHERS: &[&str] = &[
    "TLS_RSA_WITH_RC4_128_SHA",
    "TLS_RSA_WITH_RC4_128_MD5",
    "TLS_RSA_WITH_3DES_EDE_CBC_SHA",
    "TLS_RSA_WITH_DES_CBC_SHA",
    "SSL_RSA_WITH_RC4_128_SHA",
    "SSL_RSA_WITH_RC4_128_MD5",
];

/// SSL Analyzer with JA3/JA3S fingerprint tracking
pub struct SslAnalyzer {
    version_counts: HashMap<String, u64>,
    cipher_counts: HashMap<String, u64>,
    ja3_counts: HashMap<String, u64>,
    expiring: Vec<ExpiringCert>,
    weak: Vec<WeakCipher>,
    total: u64,
}

impl SslAnalyzer {
    pub fn new() -> Self {
        Self {
            version_counts: HashMap::new(),
            cipher_counts: HashMap::new(),
            ja3_counts: HashMap::new(),
            expiring: Vec::new(),
            weak: Vec::new(),
            total: 0,
        }
    }

    pub fn process(&mut self, entry: &SslLogEntry) {
        self.total += 1;

        if let Some(v) = &entry.version {
            *self.version_counts.entry(v.clone()).or_insert(0) += 1;
        }
        if let Some(c) = &entry.cipher {
            *self.cipher_counts.entry(c.clone()).or_insert(0) += 1;

            // Check for weak ciphers
            if WEAK_CIPHERS.iter().any(|wc| c.contains(wc)) {
                self.weak.push(WeakCipher {
                    cipher: c.clone(),
                    server_name: entry.server_name.clone().unwrap_or_default(),
                    count: 1,
                });
            }
        }
        if let Some(j) = &entry.ja3 {
            *self.ja3_counts.entry(j.clone()).or_insert(0) += 1;
        }

        // Check certificate expiry
        if let Some(expiry) = entry.not_valid_after {
            let days = (expiry - Utc::now()).num_days();
            if days < 30 && days > 0 {
                self.expiring.push(ExpiringCert {
                    server_name: entry.server_name.clone().unwrap_or_default(),
                    not_valid_after: expiry,
                    days_remaining: days,
                });
            }
        }
    }

    pub fn analytics(&self) -> SslAnalytics {
        let mut ja3: Vec<_> = self.ja3_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        ja3.sort_by(|a, b| b.1.cmp(&a.1));
        ja3.truncate(20);

        SslAnalytics {
            total_connections: self.total,
            version_dist: self.version_counts.clone(),
            cipher_dist: self.cipher_counts.clone(),
            ja3_fingerprints: ja3,
            expiring_certs: self.expiring.clone(),
            weak_ciphers: self.weak.clone(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for SslAnalyzer {
    fn default() -> Self { Self::new() }
}
