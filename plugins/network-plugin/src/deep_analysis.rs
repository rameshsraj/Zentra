//! Zentra Deep Packet Analysis
//! Enriches captured packets with service detection, bogon classification,
//! IP threat-list matching, ICMP classification, and host resolution.
//! Inspired by Sniffnet parse_packets / manage_packets modules.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::capture::{CaptureState, ConnectionKey, Protocol};
use crate::network_types::{
    classify_ip_reservation, identify_service, BogonResult, ConnectionEndpoint,
    FavoritesList, HostTrafficEntry, IcmpClassification, IpThreatList,
    NetworkService, ResolvedHost, TrafficCounters, TrafficDirection,
    TrafficSnapshot, TransportProtocol,
};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the deep analysis engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeepAnalysisConfig {
    pub enable_rdns: bool,
    pub enable_bogon_detection: bool,
    pub enable_threat_list: bool,
    pub enable_service_detection: bool,
    pub rdns_timeout_ms: u64,
    pub threat_list_path: Option<String>,
    pub top_talkers_limit: usize,
}

impl Default for DeepAnalysisConfig {
    fn default() -> Self {
        Self {
            enable_rdns: true,
            enable_bogon_detection: true,
            enable_threat_list: true,
            enable_service_detection: true,
            rdns_timeout_ms: 2000,
            threat_list_path: None,
            top_talkers_limit: 20,
        }
    }
}

// ============================================================================
// Enriched Packet
// ============================================================================

/// Result of deep-analyzing a single packet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnrichedPacket {
    pub endpoint: ConnectionEndpoint,
    pub service: NetworkService,
    pub src_bogon: Option<BogonResult>,
    pub dst_bogon: Option<BogonResult>,
    pub src_threat: bool,
    pub dst_threat: bool,
    pub icmp_type: Option<IcmpClassification>,
    pub packet_bytes: u64,
    pub direction: TrafficDirection,
    pub timestamp_ms: i64,
}

// ============================================================================
// rDNS Cache
// ============================================================================

/// In-memory reverse DNS cache (IP → hostname).
#[derive(Clone, Default)]
pub struct RdnsCache {
    cache: Arc<RwLock<HashMap<IpAddr, String>>>,
}

impl RdnsCache {
    pub fn new() -> Self {
        Self { cache: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub fn lookup(&self, ip: &IpAddr) -> Option<String> {
        self.cache.read().get(ip).cloned()
    }

    pub fn insert(&self, ip: IpAddr, hostname: String) {
        self.cache.write().insert(ip, hostname);
    }

    /// Resolve via system DNS (blocking — should be called from spawn_blocking).
    pub fn resolve_blocking(&self, ip: &IpAddr) -> Option<String> {
        if let Some(cached) = self.lookup(ip) {
            return Some(cached);
        }
        match dns_lookup::lookup_addr(ip) {
            Ok(hostname) => {
                // Don't cache if it resolved to the IP itself
                if hostname != ip.to_string() {
                    self.insert(*ip, hostname.clone());
                    Some(hostname)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }
}

// ============================================================================
// Deep Analysis Engine
// ============================================================================

/// The deep analysis engine enriches raw packet data.
pub struct DeepAnalysisEngine {
    config: DeepAnalysisConfig,
    rdns_cache: RdnsCache,
    threat_list: Arc<RwLock<IpThreatList>>,
    favorites: Arc<RwLock<FavoritesList>>,
    /// Per-host enriched counters
    host_entries: Arc<RwLock<HashMap<IpAddr, HostTrafficEntry>>>,
    /// Service breakdown counters
    service_counts: Arc<RwLock<HashMap<String, u64>>>,
    /// ICMP type counters
    icmp_counts: Arc<RwLock<HashMap<String, u64>>>,
    /// Threat hit counter
    threat_hits: Arc<std::sync::atomic::AtomicU64>,
    /// Bogon hit counter
    bogon_hits: Arc<std::sync::atomic::AtomicU64>,
}

impl DeepAnalysisEngine {
    pub fn new(config: DeepAnalysisConfig) -> Self {
        Self {
            config,
            rdns_cache: RdnsCache::new(),
            threat_list: Arc::new(RwLock::new(IpThreatList::default())),
            favorites: Arc::new(RwLock::new(FavoritesList::default())),
            host_entries: Arc::new(RwLock::new(HashMap::new())),
            service_counts: Arc::new(RwLock::new(HashMap::new())),
            icmp_counts: Arc::new(RwLock::new(HashMap::new())),
            threat_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            bogon_hits: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// Load threat list from file.
    pub async fn load_threat_list(&self, path: &str) {
        let loaded = IpThreatList::load_from_file(path, "file").await;
        debug!("Loaded {} IPs into threat list from {}", loaded.count(), path);
        *self.threat_list.write() = loaded;
    }

    /// Map capture Protocol to network_types TransportProtocol.
    fn map_protocol(proto: &Protocol) -> TransportProtocol {
        match proto {
            Protocol::Tcp => TransportProtocol::Tcp,
            Protocol::Udp => TransportProtocol::Udp,
            Protocol::Icmp => TransportProtocol::Icmp,
            Protocol::Other => TransportProtocol::Arp,
        }
    }

    /// Analyze a packet and return enriched data.
    pub fn analyze_packet(
        &self,
        src_ip: IpAddr,
        dst_ip: IpAddr,
        src_port: u16,
        dst_port: u16,
        protocol: &Protocol,
        packet_bytes: u64,
        is_outbound: bool,
        icmp_type_byte: Option<u8>,
        is_ipv6: bool,
    ) -> EnrichedPacket {
        use std::sync::atomic::Ordering;

        let now = Utc::now().timestamp_millis();
        let transport = Self::map_protocol(protocol);
        let direction = if is_outbound { TrafficDirection::Outbound } else { TrafficDirection::Inbound };

        // Service detection
        let service = if self.config.enable_service_detection {
            let port = if is_outbound { dst_port } else { src_port };
            identify_service(port, transport)
        } else {
            NetworkService::Unrecognized
        };

        // Bogon detection
        let src_bogon = if self.config.enable_bogon_detection {
            classify_ip_reservation(&src_ip)
        } else {
            None
        };
        let dst_bogon = if self.config.enable_bogon_detection {
            classify_ip_reservation(&dst_ip)
        } else {
            None
        };

        if src_bogon.is_some() || dst_bogon.is_some() {
            self.bogon_hits.fetch_add(1, Ordering::Relaxed);
        }

        // Threat list check
        let threat_list = self.threat_list.read();
        let src_threat = self.config.enable_threat_list && threat_list.contains(&src_ip);
        let dst_threat = self.config.enable_threat_list && threat_list.contains(&dst_ip);

        if src_threat || dst_threat {
            self.threat_hits.fetch_add(1, Ordering::Relaxed);
        }

        // ICMP classification
        let icmp_type = icmp_type_byte.map(|t| {
            if is_ipv6 {
                IcmpClassification::from_icmpv6(t)
            } else {
                IcmpClassification::from_icmpv4(t)
            }
        });

        // Update service counts
        {
            let svc_name = service.to_string();
            let mut counts = self.service_counts.write();
            *counts.entry(svc_name).or_insert(0) += 1;
        }

        // Update ICMP counts
        if let Some(ref icmp) = icmp_type {
            let mut counts = self.icmp_counts.write();
            *counts.entry(icmp.display_name().to_string()).or_insert(0) += 1;
        }

        // Update host entries
        let remote_ip = if is_outbound { dst_ip } else { src_ip };
        {
            let favorites = self.favorites.read();
            let mut hosts = self.host_entries.write();
            let entry = hosts.entry(remote_ip).or_insert_with(|| HostTrafficEntry {
                host: ResolvedHost { ip: remote_ip.to_string(), ..Default::default() },
                is_favorite: favorites.contains(&remote_ip),
                is_reserved: classify_ip_reservation(&remote_ip).is_some(),
                reservation_category: classify_ip_reservation(&remote_ip).map(|b| b.category),
                first_seen_epoch_ms: now,
                ..Default::default()
            });
            entry.counters.record_packet(packet_bytes, direction);
            entry.last_seen_epoch_ms = now;
            entry.is_threat_listed = src_threat || dst_threat;
            entry.service = service.clone();
        }

        let endpoint = ConnectionEndpoint::new(
            src_ip,
            Some(src_port),
            dst_ip,
            Some(dst_port),
            transport,
        );

        EnrichedPacket {
            endpoint,
            service,
            src_bogon,
            dst_bogon,
            src_threat,
            dst_threat,
            icmp_type,
            packet_bytes,
            direction,
            timestamp_ms: now,
        }
    }

    /// Trigger async rDNS resolution for an IP.
    pub async fn resolve_host(&self, ip: IpAddr) -> Option<String> {
        let cache = self.rdns_cache.clone();
        let timeout = tokio::time::Duration::from_millis(self.config.rdns_timeout_ms);

        let result = tokio::time::timeout(timeout, tokio::task::spawn_blocking(move || {
            cache.resolve_blocking(&ip)
        }))
        .await;

        match result {
            Ok(Ok(hostname)) => {
                if let Some(ref name) = hostname {
                    let mut hosts = self.host_entries.write();
                    if let Some(entry) = hosts.get_mut(&ip) {
                        entry.host.rdns_hostname = name.clone();
                    }
                }
                hostname
            }
            _ => {
                debug!("rDNS timeout or error for {}", ip);
                None
            }
        }
    }

    /// Generate a traffic snapshot with all enrichments.
    pub fn snapshot(&self, total_packets: u64, total_bytes: u64) -> TrafficSnapshot {
        use std::sync::atomic::Ordering;

        let hosts = self.host_entries.read();
        let service_counts = self.service_counts.read();
        let icmp_counts = self.icmp_counts.read();

        // Build top talkers sorted by total bytes
        let mut talkers: Vec<HostTrafficEntry> = hosts.values().cloned().collect();
        talkers.sort_by(|a, b| b.counters.total_bytes().cmp(&a.counters.total_bytes()));
        talkers.truncate(self.config.top_talkers_limit);

        TrafficSnapshot {
            total_packets,
            total_bytes,
            protocol_counts: HashMap::new(), // filled by capture layer
            service_counts: service_counts.clone(),
            icmp_type_counts: icmp_counts.clone(),
            top_talkers: talkers,
            threat_hits: self.threat_hits.load(Ordering::Relaxed),
            reserved_hits: self.bogon_hits.load(Ordering::Relaxed),
            unique_hosts: hosts.len(),
            unique_connections: 0, // filled by capture layer
            snapshot_epoch_ms: Utc::now().timestamp_millis(),
        }
    }

    /// Add an IP to the favorites list.
    pub fn add_favorite(&self, ip: IpAddr, label: &str) {
        self.favorites.write().add(ip, label);
        // Update host entry if it exists
        let mut hosts = self.host_entries.write();
        if let Some(entry) = hosts.get_mut(&ip) {
            entry.is_favorite = true;
        }
    }

    /// Remove an IP from the favorites list.
    pub fn remove_favorite(&self, ip: &IpAddr) {
        self.favorites.write().remove(ip);
        let mut hosts = self.host_entries.write();
        if let Some(entry) = hosts.get_mut(ip) {
            entry.is_favorite = false;
        }
    }

    /// Get current rDNS cache size.
    pub fn rdns_cache_size(&self) -> usize {
        self.rdns_cache.cache_size()
    }

    /// Get current threat list size.
    pub fn threat_list_size(&self) -> usize {
        self.threat_list.read().count()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_analyze_packet_basic() {
        let engine = DeepAnalysisEngine::new(DeepAnalysisConfig::default());

        let enriched = engine.analyze_packet(
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            45678,
            443,
            &Protocol::Tcp,
            1500,
            true,
            None,
            false,
        );

        assert_eq!(enriched.service, NetworkService::Identified("HTTPS".into()));
        assert!(enriched.src_bogon.is_some()); // 192.168.x.x is private
        assert!(enriched.dst_bogon.is_none()); // 8.8.8.8 is public
        assert!(!enriched.src_threat);
        assert!(!enriched.dst_threat);
        assert_eq!(enriched.packet_bytes, 1500);
    }

    #[test]
    fn test_analyze_icmp_packet() {
        let engine = DeepAnalysisEngine::new(DeepAnalysisConfig::default());

        let enriched = engine.analyze_packet(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            0,
            0,
            &Protocol::Icmp,
            64,
            true,
            Some(8), // Echo Request
            false,
        );

        assert_eq!(enriched.service, NetworkService::NotApplicable);
        assert!(enriched.icmp_type.is_some());
        assert_eq!(enriched.icmp_type.unwrap().display_name(), "Echo Request");
    }

    #[test]
    fn test_service_counts_accumulate() {
        let engine = DeepAnalysisEngine::new(DeepAnalysisConfig::default());

        // 3 HTTPS packets
        for _ in 0..3 {
            engine.analyze_packet(
                "10.0.0.1".parse().unwrap(),
                "1.1.1.1".parse().unwrap(),
                40000, 443, &Protocol::Tcp, 500, true, None, false,
            );
        }
        // 2 SSH packets
        for _ in 0..2 {
            engine.analyze_packet(
                "10.0.0.1".parse().unwrap(),
                "1.1.1.1".parse().unwrap(),
                40001, 22, &Protocol::Tcp, 200, true, None, false,
            );
        }

        let counts = engine.service_counts.read();
        assert_eq!(*counts.get("HTTPS").unwrap(), 3);
        assert_eq!(*counts.get("SSH").unwrap(), 2);
    }

    #[test]
    fn test_host_entries_tracked() {
        let engine = DeepAnalysisEngine::new(DeepAnalysisConfig::default());

        engine.analyze_packet(
            "10.0.0.1".parse().unwrap(),
            "8.8.8.8".parse().unwrap(),
            40000, 53, &Protocol::Udp, 100, true, None, false,
        );
        engine.analyze_packet(
            "10.0.0.1".parse().unwrap(),
            "8.8.4.4".parse().unwrap(),
            40001, 443, &Protocol::Tcp, 200, true, None, false,
        );

        let hosts = engine.host_entries.read();
        assert_eq!(hosts.len(), 2);
        assert!(hosts.contains_key(&"8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(hosts.contains_key(&"8.8.4.4".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_favorites() {
        let engine = DeepAnalysisEngine::new(DeepAnalysisConfig::default());
        let ip: IpAddr = "8.8.8.8".parse().unwrap();

        engine.analyze_packet(
            "10.0.0.1".parse().unwrap(), ip, 40000, 443,
            &Protocol::Tcp, 500, true, None, false,
        );

        engine.add_favorite(ip, "Google DNS");
        {
            let hosts = engine.host_entries.read();
            assert!(hosts.get(&ip).unwrap().is_favorite);
        }

        engine.remove_favorite(&ip);
        {
            let hosts = engine.host_entries.read();
            assert!(!hosts.get(&ip).unwrap().is_favorite);
        }
    }

    #[test]
    fn test_snapshot() {
        let engine = DeepAnalysisEngine::new(DeepAnalysisConfig {
            top_talkers_limit: 5,
            ..Default::default()
        });

        for i in 0..10 {
            engine.analyze_packet(
                "10.0.0.1".parse().unwrap(),
                format!("1.1.1.{}", i).parse().unwrap(),
                40000, 443, &Protocol::Tcp, (i as u64 + 1) * 100, true, None, false,
            );
        }

        let snap = engine.snapshot(10, 5500);
        assert_eq!(snap.unique_hosts, 10);
        assert_eq!(snap.top_talkers.len(), 5); // limited to 5
        // Top talker should be the one with most bytes (1.1.1.9, 1000 bytes)
        assert_eq!(snap.top_talkers[0].host.ip, "1.1.1.9");
    }

    #[test]
    fn test_rdns_cache() {
        let cache = RdnsCache::new();
        assert!(cache.lookup(&"8.8.8.8".parse().unwrap()).is_none());

        cache.insert("8.8.8.8".parse().unwrap(), "dns.google".into());
        assert_eq!(cache.lookup(&"8.8.8.8".parse().unwrap()), Some("dns.google".into()));
        assert_eq!(cache.cache_size(), 1);
    }
}
