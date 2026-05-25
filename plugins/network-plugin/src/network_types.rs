//! Zentra Network Analysis Types
//! Provides host resolution, service detection, bogon classification,
//! IP threat lists, protocol enums, traffic direction, address-port pairs,
//! ICMP type mapping, and data counters for the Zentra monitoring platform.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Instant;

// ============================================================================
// Protocol
// ============================================================================

/// Transport protocol observed during packet inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportProtocol {
    Tcp,
    Udp,
    Icmp,
    Arp,
}

impl fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Tcp => write!(f, "TCP"),
            Self::Udp => write!(f, "UDP"),
            Self::Icmp => write!(f, "ICMP"),
            Self::Arp => write!(f, "ARP"),
        }
    }
}

// ============================================================================
// Traffic Direction
// ============================================================================

/// Direction of traffic relative to monitored interface.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TrafficDirection {
    #[default]
    Inbound,
    Outbound,
}

impl fmt::Display for TrafficDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Inbound => write!(f, "inbound"),
            Self::Outbound => write!(f, "outbound"),
        }
    }
}

// ============================================================================
// Data Representation
// ============================================================================

/// Unit for displaying data volumes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum DataUnit {
    Packets,
    #[default]
    Bytes,
    Bits,
}

// ============================================================================
// Connection Endpoint
// ============================================================================

/// A network connection identified by source/destination IP, port, and protocol.
#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionEndpoint {
    pub src_ip: IpAddr,
    pub src_port: Option<u16>,
    pub dst_ip: IpAddr,
    pub dst_port: Option<u16>,
    pub protocol: TransportProtocol,
}

impl ConnectionEndpoint {
    pub fn new(
        src_ip: IpAddr,
        src_port: Option<u16>,
        dst_ip: IpAddr,
        dst_port: Option<u16>,
        protocol: TransportProtocol,
    ) -> Self {
        Self { src_ip, src_port, dst_ip, dst_port, protocol }
    }

    /// Returns the canonical key (ordered by IP) so both directions map to same connection.
    pub fn canonical_key(&self) -> String {
        let a = format!("{}:{}", self.src_ip, self.src_port.unwrap_or(0));
        let b = format!("{}:{}", self.dst_ip, self.dst_port.unwrap_or(0));
        if a <= b {
            format!("{a}<>{b}/{}", self.protocol)
        } else {
            format!("{b}<>{a}/{}", self.protocol)
        }
    }
}

impl fmt::Display for ConnectionEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}:{} -> {}:{} [{}]",
            self.src_ip,
            self.src_port.map_or("*".into(), |p| p.to_string()),
            self.dst_ip,
            self.dst_port.map_or("*".into(), |p| p.to_string()),
            self.protocol,
        )
    }
}

// ============================================================================
// Traffic Counters
// ============================================================================

/// Bidirectional traffic counters for a connection or host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct TrafficCounters {
    pub inbound_packets: u64,
    pub outbound_packets: u64,
    pub inbound_bytes: u64,
    pub outbound_bytes: u64,
}

impl TrafficCounters {
    pub fn record_packet(&mut self, bytes: u64, direction: TrafficDirection) {
        match direction {
            TrafficDirection::Inbound => {
                self.inbound_packets += 1;
                self.inbound_bytes += bytes;
            }
            TrafficDirection::Outbound => {
                self.outbound_packets += 1;
                self.outbound_bytes += bytes;
            }
        }
    }

    pub fn total_packets(&self) -> u64 {
        self.inbound_packets + self.outbound_packets
    }

    pub fn total_bytes(&self) -> u64 {
        self.inbound_bytes + self.outbound_bytes
    }

    pub fn total_bits(&self) -> u64 {
        self.total_bytes() * 8
    }

    pub fn value_for_unit(&self, unit: DataUnit, direction: Option<TrafficDirection>) -> u64 {
        match (unit, direction) {
            (DataUnit::Packets, Some(TrafficDirection::Inbound)) => self.inbound_packets,
            (DataUnit::Packets, Some(TrafficDirection::Outbound)) => self.outbound_packets,
            (DataUnit::Packets, None) => self.total_packets(),
            (DataUnit::Bytes, Some(TrafficDirection::Inbound)) => self.inbound_bytes,
            (DataUnit::Bytes, Some(TrafficDirection::Outbound)) => self.outbound_bytes,
            (DataUnit::Bytes, None) => self.total_bytes(),
            (DataUnit::Bits, Some(TrafficDirection::Inbound)) => self.inbound_bytes * 8,
            (DataUnit::Bits, Some(TrafficDirection::Outbound)) => self.outbound_bytes * 8,
            (DataUnit::Bits, None) => self.total_bits(),
        }
    }

    pub fn merge(&mut self, other: &TrafficCounters) {
        self.inbound_packets += other.inbound_packets;
        self.outbound_packets += other.outbound_packets;
        self.inbound_bytes += other.inbound_bytes;
        self.outbound_bytes += other.outbound_bytes;
    }
}

// ============================================================================
// ASN (Autonomous System Number)
// ============================================================================

/// Autonomous System information for an IP address.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutonomousSystem {
    pub asn_number: u32,
    pub name: String,
}

impl fmt::Display for AutonomousSystem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.name.is_empty() {
            write!(f, "AS{}", self.asn_number)
        } else {
            write!(f, "AS{} ({})", self.asn_number, self.name)
        }
    }
}

// ============================================================================
// Resolved Host
// ============================================================================

/// A resolved network host with rDNS, ASN, and geo info.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResolvedHost {
    pub ip: String,
    pub rdns_hostname: String,
    pub asn: AutonomousSystem,
    pub country_code: String,
    pub country_name: String,
    pub city: String,
}

impl ResolvedHost {
    pub fn label(&self) -> String {
        if !self.rdns_hostname.is_empty()
            && self.rdns_hostname.parse::<IpAddr>().is_err()
        {
            if self.asn.name.is_empty() {
                self.rdns_hostname.clone()
            } else {
                format!("{} — {}", self.rdns_hostname, self.asn.name)
            }
        } else if !self.asn.name.is_empty() {
            format!("{} — {}", self.ip, self.asn.name)
        } else {
            self.ip.clone()
        }
    }
}

impl fmt::Display for ResolvedHost {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ============================================================================
// Service Detection
// ============================================================================

/// Identified upper-layer service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkService {
    Identified(String),
    Unrecognized,
    NotApplicable,
}

impl Default for NetworkService {
    fn default() -> Self {
        Self::Unrecognized
    }
}

impl fmt::Display for NetworkService {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Identified(name) => write!(f, "{name}"),
            Self::Unrecognized => write!(f, "unknown"),
            Self::NotApplicable => write!(f, "n/a"),
        }
    }
}

/// Zentra port-to-service mapping for well-known ports.
pub fn identify_service(port: u16, proto: TransportProtocol) -> NetworkService {
    // ICMP/ARP don't use ports
    if matches!(proto, TransportProtocol::Icmp | TransportProtocol::Arp) {
        return NetworkService::NotApplicable;
    }

    let name = match (port, proto) {
        (20 | 21, TransportProtocol::Tcp) => "FTP",
        (22, TransportProtocol::Tcp) => "SSH",
        (23, TransportProtocol::Tcp) => "Telnet",
        (25, TransportProtocol::Tcp) => "SMTP",
        (53, _) => "DNS",
        (67 | 68, TransportProtocol::Udp) => "DHCP",
        (69, TransportProtocol::Udp) => "TFTP",
        (80, TransportProtocol::Tcp) => "HTTP",
        (110, TransportProtocol::Tcp) => "POP3",
        (123, TransportProtocol::Udp) => "NTP",
        (143, TransportProtocol::Tcp) => "IMAP",
        (161 | 162, TransportProtocol::Udp) => "SNMP",
        (389, TransportProtocol::Tcp) => "LDAP",
        (443, TransportProtocol::Tcp) => "HTTPS",
        (445, TransportProtocol::Tcp) => "SMB",
        (465, TransportProtocol::Tcp) => "SMTPS",
        (514, TransportProtocol::Udp) => "Syslog",
        (587, TransportProtocol::Tcp) => "SMTP-Submission",
        (636, TransportProtocol::Tcp) => "LDAPS",
        (853, _) => "DNS-over-TLS",
        (993, TransportProtocol::Tcp) => "IMAPS",
        (995, TransportProtocol::Tcp) => "POP3S",
        (1433, TransportProtocol::Tcp) => "MSSQL",
        (1521, TransportProtocol::Tcp) => "Oracle-DB",
        (2049, _) => "NFS",
        (3306, TransportProtocol::Tcp) => "MySQL",
        (3389, TransportProtocol::Tcp) => "RDP",
        (5432, TransportProtocol::Tcp) => "PostgreSQL",
        (5672, TransportProtocol::Tcp) => "AMQP",
        (5900, TransportProtocol::Tcp) => "VNC",
        (6379, TransportProtocol::Tcp) => "Redis",
        (6443, TransportProtocol::Tcp) => "K8s-API",
        (8080, TransportProtocol::Tcp) => "HTTP-Proxy",
        (8443, TransportProtocol::Tcp) => "HTTPS-Alt",
        (9090, TransportProtocol::Tcp) => "Prometheus",
        (9100, TransportProtocol::Tcp) => "Node-Exporter",
        (9200, TransportProtocol::Tcp) => "Elasticsearch",
        (27017, TransportProtocol::Tcp) => "MongoDB",
        _ => return NetworkService::Unrecognized,
    };

    NetworkService::Identified(name.into())
}

// ============================================================================
// IP Threat List
// ============================================================================

/// Thread-safe IP threat list for flagging known-bad addresses.
#[derive(Clone, Default, Debug)]
pub struct IpThreatList {
    entries: Arc<HashSet<IpAddr>>,
    source_name: String,
}

impl IpThreatList {
    pub fn new(entries: HashSet<IpAddr>, source: impl Into<String>) -> Self {
        Self { entries: Arc::new(entries), source_name: source.into() }
    }

    /// Load from file: one IP per line, # comments allowed.
    pub async fn load_from_file(path: &str, source: impl Into<String>) -> Self {
        let source_name = source.into();
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                let mut set = HashSet::new();
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    // Handle CIDR notation by taking just the IP for now
                    let ip_str = trimmed.split('/').next().unwrap_or(trimmed);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        set.insert(ip);
                    }
                }
                Self { entries: Arc::new(set), source_name }
            }
            Err(_) => Self { entries: Arc::new(HashSet::new()), source_name },
        }
    }

    pub fn contains(&self, ip: &IpAddr) -> bool {
        self.entries.contains(ip)
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn source(&self) -> &str {
        &self.source_name
    }
}

// ============================================================================
// Bogon / Reserved IP Classification
// ============================================================================

/// Result of bogon classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BogonResult {
    pub is_bogon: bool,
    pub category: String,
}

/// Classify whether an IP belongs to a reserved / non-routable range.
pub fn classify_ip_reservation(ip: &IpAddr) -> Option<BogonResult> {
    let cat = match ip {
        IpAddr::V4(v4) => classify_v4_reservation(v4),
        IpAddr::V6(_v6) => classify_v6_reservation(ip),
    };
    cat.map(|c| BogonResult { is_bogon: true, category: c.to_string() })
}

fn classify_v4_reservation(ip: &Ipv4Addr) -> Option<&'static str> {
    let o = ip.octets();
    if o[0] == 0 { return Some("current-network"); }
    if o[0] == 10 { return Some("private-rfc1918-class-a"); }
    if o[0] == 127 { return Some("loopback"); }
    if o[0] == 169 && o[1] == 254 { return Some("link-local"); }
    if o[0] == 172 && (16..=31).contains(&o[1]) { return Some("private-rfc1918-class-b"); }
    if o[0] == 192 && o[1] == 168 { return Some("private-rfc1918-class-c"); }
    if o[0] == 192 && o[1] == 0 && o[2] == 0 { return Some("ietf-protocol"); }
    if o[0] == 192 && o[1] == 0 && o[2] == 2 { return Some("documentation-test-net-1"); }
    if o[0] == 198 && (18..=19).contains(&o[1]) { return Some("benchmark-testing"); }
    if o[0] == 198 && o[1] == 51 && o[2] == 100 { return Some("documentation-test-net-2"); }
    if o[0] == 203 && o[1] == 0 && o[2] == 113 { return Some("documentation-test-net-3"); }
    if o[0] == 100 && (64..=127).contains(&o[1]) { return Some("carrier-grade-nat"); }
    if o[0] >= 224 && o[0] <= 239 { return Some("multicast"); }
    if o[0] >= 240 { return Some("reserved-future"); }
    None
}

fn classify_v6_reservation(ip: &IpAddr) -> Option<&'static str> {
    if let IpAddr::V6(v6) = ip {
        let s = v6.segments();
        if s == [0u16; 8] { return Some("unspecified-v6"); }
        if s[..7] == [0u16; 7] && s[7] == 1 { return Some("loopback-v6"); }
        if s[0..5] == [0, 0, 0, 0, 0] && s[5] == 0xffff { return Some("ipv4-mapped"); }
        if s[0] & 0xffc0 == 0xfe80 { return Some("link-local-v6"); }
        if s[0] & 0xfe00 == 0xfc00 { return Some("unique-local-v6"); }
        if s[0] & 0xff00 == 0xff00 { return Some("multicast-v6"); }
        if s[0] == 0x2001 && s[1] == 0x0db8 { return Some("documentation-v6"); }
        if s[0] == 0x0100 && s[1..4] == [0, 0, 0] { return Some("discard-only-v6"); }
    }
    None
}

// ============================================================================
// ICMP Classification
// ============================================================================

/// Classified ICMP message type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum IcmpClassification {
    EchoReply,
    DestUnreachable,
    SourceQuench,
    Redirect,
    EchoRequest,
    RouterAdvert,
    RouterSolicit,
    TimeExceeded,
    ParamProblem,
    Timestamp,
    TimestampReply,
    AddrMaskRequest,
    AddrMaskReply,
    TraceRoute,
    PacketTooBig,
    NeighborSolicit,
    NeighborAdvert,
    MldQuery,
    MldReport,
    Other(u8),
}

impl IcmpClassification {
    /// Classify ICMPv4 type byte.
    pub fn from_icmpv4(type_code: u8) -> Self {
        match type_code {
            0 => Self::EchoReply,
            3 => Self::DestUnreachable,
            4 => Self::SourceQuench,
            5 => Self::Redirect,
            8 => Self::EchoRequest,
            9 => Self::RouterAdvert,
            10 => Self::RouterSolicit,
            11 => Self::TimeExceeded,
            12 => Self::ParamProblem,
            13 => Self::Timestamp,
            14 => Self::TimestampReply,
            17 => Self::AddrMaskRequest,
            18 => Self::AddrMaskReply,
            30 => Self::TraceRoute,
            other => Self::Other(other),
        }
    }

    /// Classify ICMPv6 type byte.
    pub fn from_icmpv6(type_code: u8) -> Self {
        match type_code {
            1 => Self::DestUnreachable,
            2 => Self::PacketTooBig,
            3 => Self::TimeExceeded,
            4 => Self::ParamProblem,
            128 => Self::EchoRequest,
            129 => Self::EchoReply,
            130 => Self::MldQuery,
            131 => Self::MldReport,
            133 => Self::RouterSolicit,
            134 => Self::RouterAdvert,
            135 => Self::NeighborSolicit,
            136 => Self::NeighborAdvert,
            137 => Self::Redirect,
            other => Self::Other(other),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::EchoReply => "Echo Reply",
            Self::DestUnreachable => "Destination Unreachable",
            Self::SourceQuench => "Source Quench",
            Self::Redirect => "Redirect",
            Self::EchoRequest => "Echo Request",
            Self::RouterAdvert => "Router Advertisement",
            Self::RouterSolicit => "Router Solicitation",
            Self::TimeExceeded => "Time Exceeded",
            Self::ParamProblem => "Parameter Problem",
            Self::Timestamp => "Timestamp",
            Self::TimestampReply => "Timestamp Reply",
            Self::AddrMaskRequest => "Address Mask Request",
            Self::AddrMaskReply => "Address Mask Reply",
            Self::TraceRoute => "Traceroute",
            Self::PacketTooBig => "Packet Too Big",
            Self::NeighborSolicit => "Neighbor Solicitation",
            Self::NeighborAdvert => "Neighbor Advertisement",
            Self::MldQuery => "MLD Query",
            Self::MldReport => "MLD Report",
            Self::Other(_) => "Other",
        }
    }
}

impl fmt::Display for IcmpClassification {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ============================================================================
// Host Traffic Summary
// ============================================================================

/// Per-host traffic summary combining counters with host metadata.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostTrafficEntry {
    pub host: ResolvedHost,
    pub service: NetworkService,
    pub counters: TrafficCounters,
    pub is_threat_listed: bool,
    pub is_reserved: bool,
    pub reservation_category: Option<String>,
    pub is_favorite: bool,
    pub active_connections: usize,
    pub first_seen_epoch_ms: i64,
    pub last_seen_epoch_ms: i64,
}

/// Aggregate traffic analysis result.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrafficSnapshot {
    pub total_packets: u64,
    pub total_bytes: u64,
    pub protocol_counts: HashMap<String, u64>,
    pub service_counts: HashMap<String, u64>,
    pub icmp_type_counts: HashMap<String, u64>,
    pub top_talkers: Vec<HostTrafficEntry>,
    pub threat_hits: u64,
    pub reserved_hits: u64,
    pub unique_hosts: usize,
    pub unique_connections: usize,
    pub snapshot_epoch_ms: i64,
}

// ============================================================================
// Favorites List
// ============================================================================

/// User-maintained favorites list for quick-access hosts.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FavoritesList {
    pub entries: Vec<FavoriteEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FavoriteEntry {
    pub ip: IpAddr,
    pub label: String,
    pub notes: String,
    pub added_epoch_ms: i64,
}

impl FavoritesList {
    pub fn add(&mut self, ip: IpAddr, label: impl Into<String>) {
        let now = chrono::Utc::now().timestamp_millis();
        self.entries.push(FavoriteEntry {
            ip,
            label: label.into(),
            notes: String::new(),
            added_epoch_ms: now,
        });
    }

    pub fn remove(&mut self, ip: &IpAddr) {
        self.entries.retain(|e| &e.ip != ip);
    }

    pub fn contains(&self, ip: &IpAddr) -> bool {
        self.entries.iter().any(|e| &e.ip == ip)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    #[test]
    fn test_protocol_display() {
        assert_eq!(TransportProtocol::Tcp.to_string(), "TCP");
        assert_eq!(TransportProtocol::Udp.to_string(), "UDP");
        assert_eq!(TransportProtocol::Icmp.to_string(), "ICMP");
    }

    #[test]
    fn test_traffic_direction_display() {
        assert_eq!(TrafficDirection::Inbound.to_string(), "inbound");
        assert_eq!(TrafficDirection::Outbound.to_string(), "outbound");
    }

    #[test]
    fn test_connection_endpoint_display() {
        let ep = ConnectionEndpoint::new(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            Some(12345),
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            Some(443),
            TransportProtocol::Tcp,
        );
        let s = ep.to_string();
        assert!(s.contains("10.0.0.1:12345"));
        assert!(s.contains("8.8.8.8:443"));
    }

    #[test]
    fn test_connection_endpoint_canonical_key() {
        let ep1 = ConnectionEndpoint::new(
            "10.0.0.1".parse().unwrap(), Some(1000),
            "10.0.0.2".parse().unwrap(), Some(2000),
            TransportProtocol::Tcp,
        );
        let ep2 = ConnectionEndpoint::new(
            "10.0.0.2".parse().unwrap(), Some(2000),
            "10.0.0.1".parse().unwrap(), Some(1000),
            TransportProtocol::Tcp,
        );
        assert_eq!(ep1.canonical_key(), ep2.canonical_key());
    }

    #[test]
    fn test_traffic_counters() {
        let mut tc = TrafficCounters::default();
        tc.record_packet(100, TrafficDirection::Inbound);
        tc.record_packet(200, TrafficDirection::Outbound);
        tc.record_packet(150, TrafficDirection::Inbound);

        assert_eq!(tc.inbound_packets, 2);
        assert_eq!(tc.outbound_packets, 1);
        assert_eq!(tc.inbound_bytes, 250);
        assert_eq!(tc.outbound_bytes, 200);
        assert_eq!(tc.total_packets(), 3);
        assert_eq!(tc.total_bytes(), 450);
        assert_eq!(tc.total_bits(), 3600);
    }

    #[test]
    fn test_traffic_counters_merge() {
        let mut a = TrafficCounters::default();
        a.record_packet(100, TrafficDirection::Inbound);
        let mut b = TrafficCounters::default();
        b.record_packet(200, TrafficDirection::Outbound);
        a.merge(&b);
        assert_eq!(a.inbound_packets, 1);
        assert_eq!(a.outbound_packets, 1);
        assert_eq!(a.total_bytes(), 300);
    }

    #[test]
    fn test_traffic_counters_unit_query() {
        let mut tc = TrafficCounters::default();
        tc.record_packet(100, TrafficDirection::Inbound);
        assert_eq!(tc.value_for_unit(DataUnit::Packets, Some(TrafficDirection::Inbound)), 1);
        assert_eq!(tc.value_for_unit(DataUnit::Bytes, None), 100);
        assert_eq!(tc.value_for_unit(DataUnit::Bits, None), 800);
    }

    #[test]
    fn test_service_identification() {
        assert_eq!(
            identify_service(443, TransportProtocol::Tcp),
            NetworkService::Identified("HTTPS".into())
        );
        assert_eq!(
            identify_service(80, TransportProtocol::Tcp),
            NetworkService::Identified("HTTP".into())
        );
        assert_eq!(
            identify_service(53, TransportProtocol::Udp),
            NetworkService::Identified("DNS".into())
        );
        assert_eq!(
            identify_service(22, TransportProtocol::Tcp),
            NetworkService::Identified("SSH".into())
        );
        assert_eq!(
            identify_service(5432, TransportProtocol::Tcp),
            NetworkService::Identified("PostgreSQL".into())
        );
        assert_eq!(
            identify_service(6443, TransportProtocol::Tcp),
            NetworkService::Identified("K8s-API".into())
        );
        assert_eq!(identify_service(55555, TransportProtocol::Tcp), NetworkService::Unrecognized);
        assert_eq!(identify_service(0, TransportProtocol::Icmp), NetworkService::NotApplicable);
    }

    #[test]
    fn test_bogon_classification_v4() {
        let private_a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let res = classify_ip_reservation(&private_a).unwrap();
        assert!(res.is_bogon);
        assert_eq!(res.category, "private-rfc1918-class-a");

        let loopback = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        assert!(classify_ip_reservation(&loopback).is_some());

        let private_c = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        assert!(classify_ip_reservation(&private_c).is_some());

        let link_local = IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1));
        assert!(classify_ip_reservation(&link_local).is_some());

        let public = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
        assert!(classify_ip_reservation(&public).is_none());

        let public2 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        assert!(classify_ip_reservation(&public2).is_none());
    }

    #[test]
    fn test_bogon_classification_v6() {
        let loopback = IpAddr::V6(Ipv6Addr::LOCALHOST);
        let res = classify_ip_reservation(&loopback).unwrap();
        assert_eq!(res.category, "loopback-v6");

        let unspec = IpAddr::V6(Ipv6Addr::UNSPECIFIED);
        assert!(classify_ip_reservation(&unspec).is_some());

        let link_local: IpAddr = "fe80::1".parse().unwrap();
        assert!(classify_ip_reservation(&link_local).is_some());

        let public: IpAddr = "2001:4860:4860::8888".parse().unwrap();
        assert!(classify_ip_reservation(&public).is_none());
    }

    #[test]
    fn test_icmp_classification_v4() {
        assert_eq!(IcmpClassification::from_icmpv4(8).display_name(), "Echo Request");
        assert_eq!(IcmpClassification::from_icmpv4(0).display_name(), "Echo Reply");
        assert_eq!(IcmpClassification::from_icmpv4(3).display_name(), "Destination Unreachable");
        assert_eq!(IcmpClassification::from_icmpv4(11).display_name(), "Time Exceeded");
    }

    #[test]
    fn test_icmp_classification_v6() {
        assert_eq!(IcmpClassification::from_icmpv6(128).display_name(), "Echo Request");
        assert_eq!(IcmpClassification::from_icmpv6(129).display_name(), "Echo Reply");
        assert_eq!(IcmpClassification::from_icmpv6(135).display_name(), "Neighbor Solicitation");
        assert_eq!(IcmpClassification::from_icmpv6(2).display_name(), "Packet Too Big");
    }

    #[test]
    fn test_ip_threat_list() {
        let mut set = HashSet::new();
        set.insert(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)));
        set.insert(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)));
        let tl = IpThreatList::new(set, "test-feed");

        assert!(tl.contains(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(tl.contains(&IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4))));
        assert!(!tl.contains(&IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4))));
        assert_eq!(tl.count(), 2);
        assert_eq!(tl.source(), "test-feed");
    }

    #[test]
    fn test_resolved_host_label() {
        let h1 = ResolvedHost {
            ip: "8.8.8.8".into(),
            rdns_hostname: "dns.google".into(),
            asn: AutonomousSystem { asn_number: 15169, name: "GOOGLE".into() },
            country_code: "US".into(),
            country_name: "United States".into(),
            city: "Mountain View".into(),
        };
        assert_eq!(h1.label(), "dns.google — GOOGLE");

        let h2 = ResolvedHost {
            ip: "1.2.3.4".into(),
            rdns_hostname: String::new(),
            asn: AutonomousSystem { asn_number: 0, name: String::new() },
            ..Default::default()
        };
        assert_eq!(h2.label(), "1.2.3.4");
    }

    #[test]
    fn test_favorites_list() {
        let mut favs = FavoritesList::default();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(!favs.contains(&ip));
        favs.add(ip, "My Server");
        assert!(favs.contains(&ip));
        assert_eq!(favs.entries.len(), 1);
        favs.remove(&ip);
        assert!(!favs.contains(&ip));
    }

    #[test]
    fn test_autonomous_system_display() {
        let asn = AutonomousSystem { asn_number: 15169, name: "GOOGLE".into() };
        assert_eq!(asn.to_string(), "AS15169 (GOOGLE)");

        let asn_empty = AutonomousSystem { asn_number: 12345, name: String::new() };
        assert_eq!(asn_empty.to_string(), "AS12345");
    }
}
