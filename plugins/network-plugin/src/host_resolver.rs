//! Host Resolution Engine
//! Provides reverse DNS lookup and Geo-IP enrichment for captured IPs.
//! Uses dns-lookup for rDNS and a built-in private/reserved IP database.
//! For public IPs, uses ip-api.com (free, no registration) for Geo + ASN.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tracing::{debug, warn};

use crate::network_types::{AutonomousSystem, ResolvedHost, classify_ip_reservation};

/// Cache entry for resolved host info
#[derive(Clone, Debug)]
struct CacheEntry {
    host: ResolvedHost,
    resolved_at: Instant,
}

/// Configuration for host resolution
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// How long to cache resolved hosts
    pub cache_ttl: Duration,
    /// Maximum concurrent DNS lookups
    pub max_concurrent: usize,
    /// Whether to do rDNS lookups
    pub enable_rdns: bool,
    /// Whether to do Geo-IP lookups (uses ip-api.com, free tier)
    pub enable_geoip: bool,
    /// Rate limit for Geo-IP API calls (per minute)
    pub geoip_rate_limit: u32,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            cache_ttl: Duration::from_secs(3600), // 1 hour
            max_concurrent: 8,
            enable_rdns: true,
            enable_geoip: true,
            geoip_rate_limit: 45, // ip-api.com free tier: 45/minute
        }
    }
}

/// Thread-safe host resolver with caching
pub struct HostResolver {
    cache: Arc<RwLock<HashMap<IpAddr, CacheEntry>>>,
    config: ResolverConfig,
    /// Track API calls for rate limiting
    api_calls_this_minute: Arc<std::sync::atomic::AtomicU32>,
    minute_start: Arc<RwLock<Instant>>,
}

impl HostResolver {
    pub fn new(config: ResolverConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            api_calls_this_minute: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            minute_start: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Resolve a single IP address (cached)
    pub async fn resolve(&self, ip: IpAddr) -> ResolvedHost {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(&ip) {
                if entry.resolved_at.elapsed() < self.config.cache_ttl {
                    return entry.host.clone();
                }
            }
        }

        // Build resolution
        let mut host = ResolvedHost {
            ip: ip.to_string(),
            ..Default::default()
        };

        // Check if reserved/bogon
        if let Some(bogon) = classify_ip_reservation(&ip) {
            host.country_name = bogon.category.clone();
            host.country_code = "LO".to_string(); // Local
        }

        // Reverse DNS
        if self.config.enable_rdns {
            host.rdns_hostname = self.reverse_dns(ip).await;
        }

        // Geo-IP for public IPs only
        let is_private = classify_ip_reservation(&ip).is_some();
        if self.config.enable_geoip && !is_private && self.can_make_api_call() {
            if let Some(geo) = self.geoip_lookup(ip).await {
                host.country_code = geo.country_code;
                host.country_name = geo.country_name;
                host.city = geo.city;
                host.asn = AutonomousSystem {
                    asn_number: geo.asn_number,
                    name: geo.asn_name,
                };
            }
        }

        // Cache the result
        {
            let mut cache = self.cache.write();
            cache.insert(ip, CacheEntry {
                host: host.clone(),
                resolved_at: Instant::now(),
            });
        }

        host
    }

    /// Batch resolve multiple IPs
    pub async fn resolve_batch(&self, ips: &[IpAddr]) -> HashMap<IpAddr, ResolvedHost> {
        let mut results = HashMap::new();
        // Process in chunks to limit concurrency
        for chunk in ips.chunks(self.config.max_concurrent) {
            let futures: Vec<_> = chunk.iter().map(|ip| {
                let ip = *ip;
                async move { (ip, self.resolve(ip).await) }
            }).collect();

            for fut in futures {
                let (ip, host) = fut.await;
                results.insert(ip, host);
            }
        }
        results
    }

    /// Reverse DNS lookup
    async fn reverse_dns(&self, ip: IpAddr) -> String {
        let ip_clone = ip;
        // Run blocking DNS lookup in a spawn_blocking context
        match tokio::task::spawn_blocking(move || {
            dns_lookup::lookup_addr(&ip_clone)
        }).await {
            Ok(Ok(hostname)) => {
                debug!(ip = %ip, hostname = %hostname, "rDNS resolved");
                hostname
            }
            Ok(Err(_)) | Err(_) => String::new(),
        }
    }

    /// Geo-IP lookup using ip-api.com (free, no registration needed)
    async fn geoip_lookup(&self, ip: IpAddr) -> Option<GeoResult> {
        self.increment_api_counter();

        let url = format!("http://ip-api.com/json/{}?fields=status,country,countryCode,city,as,asname,org", ip);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .ok()?;

        match client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if data.get("status").and_then(|s| s.as_str()) == Some("success") {
                        let as_str = data.get("as").and_then(|v| v.as_str()).unwrap_or("");
                        let asn_number = as_str
                            .strip_prefix("AS")
                            .and_then(|s| s.split_whitespace().next())
                            .and_then(|n| n.parse::<u32>().ok())
                            .unwrap_or(0);

                        return Some(GeoResult {
                            country_code: data.get("countryCode").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            country_name: data.get("country").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            city: data.get("city").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            asn_number,
                            asn_name: data.get("asname").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        });
                    }
                }
                None
            }
            Err(e) => {
                warn!(ip = %ip, error = %e, "Geo-IP lookup failed");
                None
            }
        }
    }

    /// Check rate limit
    fn can_make_api_call(&self) -> bool {
        let mut start = self.minute_start.write();
        if start.elapsed() >= Duration::from_secs(60) {
            *start = Instant::now();
            self.api_calls_this_minute.store(0, std::sync::atomic::Ordering::Relaxed);
        }
        let count = self.api_calls_this_minute.load(std::sync::atomic::Ordering::Relaxed);
        count < self.config.geoip_rate_limit
    }

    fn increment_api_counter(&self) {
        self.api_calls_this_minute.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get cache stats
    pub fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read();
        let total = cache.len();
        let expired = cache.values().filter(|e| e.resolved_at.elapsed() >= self.config.cache_ttl).count();
        (total, total - expired)
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        self.cache.write().clear();
    }
}

#[derive(Debug)]
struct GeoResult {
    country_code: String,
    country_name: String,
    city: String,
    asn_number: u32,
    asn_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_config_defaults() {
        let cfg = ResolverConfig::default();
        assert_eq!(cfg.geoip_rate_limit, 45);
        assert!(cfg.enable_rdns);
        assert!(cfg.enable_geoip);
    }

    #[tokio::test]
    async fn test_resolve_private_ip() {
        let resolver = HostResolver::new(ResolverConfig {
            enable_geoip: false, // Skip API calls in tests
            ..Default::default()
        });

        let host = resolver.resolve("192.168.1.1".parse().unwrap()).await;
        assert_eq!(host.ip, "192.168.1.1");
        assert_eq!(host.country_code, "LO");
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let resolver = HostResolver::new(ResolverConfig {
            enable_rdns: false,
            enable_geoip: false,
            ..Default::default()
        });

        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        let h1 = resolver.resolve(ip).await;
        let h2 = resolver.resolve(ip).await;
        assert_eq!(h1.ip, h2.ip);

        let (total, active) = resolver.cache_stats();
        assert_eq!(total, 1);
        assert_eq!(active, 1);
    }

    #[test]
    fn test_rate_limiting() {
        let resolver = HostResolver::new(ResolverConfig {
            geoip_rate_limit: 2,
            ..Default::default()
        });

        assert!(resolver.can_make_api_call());
        resolver.increment_api_counter();
        assert!(resolver.can_make_api_call());
        resolver.increment_api_counter();
        assert!(!resolver.can_make_api_call());
    }
}
