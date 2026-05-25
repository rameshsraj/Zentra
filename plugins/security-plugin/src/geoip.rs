use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// GeoIP lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoIpResult {
    pub ip: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub asn: Option<u32>,
    pub as_org: Option<String>,
}

/// GeoIP lookup service with in-memory cache.
/// In production, this would use MaxMind GeoLite2/GeoIP2 databases.
/// For now, provides a cache + API-based fallback structure.
pub struct GeoIpService {
    cache: RwLock<HashMap<String, GeoIpResult>>,
}

impl GeoIpService {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Lookup IP geolocation
    pub fn lookup(&self, ip: &str) -> GeoIpResult {
        // Check cache first
        if let Ok(cache) = self.cache.read() {
            if let Some(result) = cache.get(ip) {
                return result.clone();
            }
        }

        // For RFC1918 addresses, return local
        if is_private_ip(ip) {
            let result = GeoIpResult {
                ip: ip.to_string(),
                country_code: Some("XX".into()),
                country_name: Some("Private Network".into()),
                city: None,
                latitude: None,
                longitude: None,
                asn: None,
                as_org: None,
            };
            self.cache_result(ip, &result);
            return result;
        }

        // Placeholder — in production, query MaxMind DB or external API
        let result = GeoIpResult {
            ip: ip.to_string(),
            country_code: None,
            country_name: None,
            city: None,
            latitude: None,
            longitude: None,
            asn: None,
            as_org: None,
        };
        self.cache_result(ip, &result);
        result
    }

    /// Batch lookup for multiple IPs
    pub fn batch_lookup(&self, ips: &[&str]) -> Vec<GeoIpResult> {
        ips.iter().map(|ip| self.lookup(ip)).collect()
    }

    fn cache_result(&self, ip: &str, result: &GeoIpResult) {
        if let Ok(mut cache) = self.cache.write() {
            // Limit cache size
            if cache.len() > 100_000 {
                cache.clear();
            }
            cache.insert(ip.to_string(), result.clone());
        }
    }

    pub fn cache_size(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }
}

impl Default for GeoIpService {
    fn default() -> Self { Self::new() }
}

/// Check if IP is RFC1918 private
fn is_private_ip(ip: &str) -> bool {
    ip.starts_with("10.")
        || ip.starts_with("172.16.") || ip.starts_with("172.17.") || ip.starts_with("172.18.")
        || ip.starts_with("172.19.") || ip.starts_with("172.20.") || ip.starts_with("172.21.")
        || ip.starts_with("172.22.") || ip.starts_with("172.23.") || ip.starts_with("172.24.")
        || ip.starts_with("172.25.") || ip.starts_with("172.26.") || ip.starts_with("172.27.")
        || ip.starts_with("172.28.") || ip.starts_with("172.29.") || ip.starts_with("172.30.")
        || ip.starts_with("172.31.")
        || ip.starts_with("192.168.")
        || ip == "127.0.0.1"
        || ip == "::1"
}
