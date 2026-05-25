use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use etherparse::{LaxPacketHeaders, NetHeaders, TransportHeader};
use parking_lot::RwLock;
use pcap::{Capture, Device};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug};

use crate::program_lookup::ProgramLookup;

/// Protocol classification based on transport layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
    Other,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Tcp => write!(f, "TCP"),
            Protocol::Udp => write!(f, "UDP"),
            Protocol::Icmp => write!(f, "ICMP"),
            Protocol::Other => write!(f, "Other"),
        }
    }
}

/// A connection between two endpoints
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ConnectionKey {
    pub source_ip: IpAddr,
    pub dest_ip: IpAddr,
    pub source_port: u16,
    pub dest_port: u16,
    pub protocol: Protocol,
}

/// Traffic statistics for a single connection
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionStats {
    pub source_ip: String,
    pub dest_ip: String,
    pub source_port: u16,
    pub dest_port: u16,
    pub protocol: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Identified upper-layer service name (e.g. "https", "ssh", "dns")
    pub service_name: Option<String>,
    /// Whether the service port is associated with known malware
    pub is_malicious: bool,
    /// Program/process name that owns this connection (if resolved)
    pub program_name: Option<String>,
    /// Program executable path
    pub program_path: Option<String>,
    /// GeoIP: country code (e.g. "US", "DE")
    pub country_code: Option<String>,
    /// GeoIP: country name (e.g. "United States")
    pub country_name: Option<String>,
    /// GeoIP: ASN name (e.g. "GOOGLE")
    pub asn_name: Option<String>,
    /// Reverse DNS hostname
    pub domain: Option<String>,
}

/// Aggregate traffic statistics per protocol
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolStats {
    pub tcp_bytes: u64,
    pub udp_bytes: u64,
    pub icmp_bytes: u64,
    pub other_bytes: u64,
    pub tcp_packets: u64,
    pub udp_packets: u64,
    pub icmp_packets: u64,
    pub other_packets: u64,
}

/// Per-host traffic summary
#[derive(Debug, Clone, Default, Serialize)]
pub struct HostTraffic {
    pub ip: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub packets_in: u64,
    pub packets_out: u64,
    pub connections: u32,
}

/// Per-program traffic summary (serializable)
#[derive(Debug, Clone, Serialize)]
pub struct ProgramTraffic {
    pub name: String,
    pub path: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connections: u32,
}

/// Snapshot of capture statistics at a point in time
#[derive(Debug, Clone, Serialize)]
pub struct CaptureSnapshot {
    pub total_packets: u64,
    pub total_bytes: u64,
    pub packets_per_second: f64,
    pub bytes_per_second: f64,
    pub active_connections: usize,
    pub protocol_stats: ProtocolStats,
    pub top_talkers: Vec<HostTraffic>,
    pub capture_duration_secs: f64,
    /// Per-program traffic breakdown
    pub programs: Vec<ProgramTraffic>,
}

/// Shared state for the packet capture engine
pub struct CaptureState {
    /// Connection tracking map
    connections: DashMap<ConnectionKey, ConnectionStats>,
    /// Protocol-level aggregate stats
    protocol_stats: RwLock<ProtocolStats>,
    /// Per-host traffic
    host_traffic: DashMap<IpAddr, HostTraffic>,
    /// Total counters
    total_packets: std::sync::atomic::AtomicU64,
    total_bytes: std::sync::atomic::AtomicU64,
    /// Capture start time
    started_at: Instant,
    /// Local addresses (to determine traffic direction)
    local_addrs: RwLock<Vec<IpAddr>>,
    /// Program/process lookup engine
    program_lookup: Arc<ProgramLookup>,
}

impl CaptureState {
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
            protocol_stats: RwLock::new(ProtocolStats::default()),
            host_traffic: DashMap::new(),
            total_packets: std::sync::atomic::AtomicU64::new(0),
            total_bytes: std::sync::atomic::AtomicU64::new(0),
            started_at: Instant::now(),
            local_addrs: RwLock::new(Vec::new()),
            program_lookup: crate::program_lookup::new_shared(),
        }
    }

    /// Get a reference to the program lookup engine
    pub fn program_lookup(&self) -> &Arc<ProgramLookup> {
        &self.program_lookup
    }

    pub fn set_local_addrs(&self, addrs: Vec<IpAddr>) {
        *self.local_addrs.write() = addrs;
    }

    fn is_local(&self, addr: &IpAddr) -> bool {
        let locals = self.local_addrs.read();
        locals.contains(addr)
    }

    /// Process a single parsed packet
    pub fn record_packet(
        &self,
        source_ip: IpAddr,
        dest_ip: IpAddr,
        source_port: u16,
        dest_port: u16,
        protocol: Protocol,
        packet_len: u64,
    ) {
        use std::sync::atomic::Ordering;
        let now = chrono::Utc::now();

        self.total_packets.fetch_add(1, Ordering::Relaxed);
        self.total_bytes.fetch_add(packet_len, Ordering::Relaxed);

        // Protocol stats
        {
            let mut stats = self.protocol_stats.write();
            match protocol {
                Protocol::Tcp => {
                    stats.tcp_bytes += packet_len;
                    stats.tcp_packets += 1;
                }
                Protocol::Udp => {
                    stats.udp_bytes += packet_len;
                    stats.udp_packets += 1;
                }
                Protocol::Icmp => {
                    stats.icmp_bytes += packet_len;
                    stats.icmp_packets += 1;
                }
                Protocol::Other => {
                    stats.other_bytes += packet_len;
                    stats.other_packets += 1;
                }
            }
        }

        // Connection tracking
        let key = ConnectionKey {
            source_ip,
            dest_ip,
            source_port,
            dest_port,
            protocol,
        };
        self.connections
            .entry(key)
            .and_modify(|cs| {
                cs.bytes_sent += packet_len;
                cs.packets_sent += 1;
                cs.last_seen = now;
            })
            .or_insert_with(|| {
                // Identify the service from the well-known port
                let svc = crate::service_db::lookup_service_entry(dest_port, protocol)
                    .or_else(|| crate::service_db::lookup_service_entry(source_port, protocol));
                let service_name = svc.as_ref().map(|s| s.name.to_string());
                let is_malicious = svc.as_ref().map_or(false, |s| s.is_malicious);

                ConnectionStats {
                    source_ip: source_ip.to_string(),
                    dest_ip: dest_ip.to_string(),
                    source_port,
                    dest_port,
                    protocol: protocol.to_string(),
                    bytes_sent: packet_len,
                    bytes_received: 0,
                    packets_sent: 1,
                    packets_received: 0,
                    first_seen: now,
                    last_seen: now,
                    service_name,
                    is_malicious,
                    program_name: None,
                    program_path: None,
                    country_code: None,
                    country_name: None,
                    asn_name: None,
                    domain: None,
                }
            });

        // Host traffic — outbound for source
        let is_src_local = self.is_local(&source_ip);
        let is_dst_local = self.is_local(&dest_ip);

        if is_src_local {
            self.host_traffic
                .entry(source_ip)
                .and_modify(|h| {
                    h.bytes_out += packet_len;
                    h.packets_out += 1;
                })
                .or_insert_with(|| HostTraffic {
                    ip: source_ip.to_string(),
                    bytes_out: packet_len,
                    packets_out: 1,
                    ..Default::default()
                });
        }
        if is_dst_local {
            self.host_traffic
                .entry(dest_ip)
                .and_modify(|h| {
                    h.bytes_in += packet_len;
                    h.packets_in += 1;
                })
                .or_insert_with(|| HostTraffic {
                    ip: dest_ip.to_string(),
                    bytes_in: packet_len,
                    packets_in: 1,
                    ..Default::default()
                });
        }
        // Track external hosts too
        if !is_src_local {
            self.host_traffic
                .entry(source_ip)
                .and_modify(|h| {
                    h.bytes_out += packet_len;
                    h.packets_out += 1;
                })
                .or_insert_with(|| HostTraffic {
                    ip: source_ip.to_string(),
                    bytes_out: packet_len,
                    packets_out: 1,
                    ..Default::default()
                });
        }
        if !is_dst_local {
            self.host_traffic
                .entry(dest_ip)
                .and_modify(|h| {
                    h.bytes_in += packet_len;
                    h.packets_in += 1;
                })
                .or_insert_with(|| HostTraffic {
                    ip: dest_ip.to_string(),
                    bytes_in: packet_len,
                    packets_in: 1,
                    ..Default::default()
                });
        }
    }

    /// Take a snapshot of current statistics
    pub fn snapshot(&self) -> CaptureSnapshot {
        use std::sync::atomic::Ordering;
        let elapsed = self.started_at.elapsed().as_secs_f64().max(0.001);
        let total_packets = self.total_packets.load(Ordering::Relaxed);
        let total_bytes = self.total_bytes.load(Ordering::Relaxed);

        let mut top_talkers: Vec<HostTraffic> = self
            .host_traffic
            .iter()
            .map(|entry| {
                let h = entry.value();
                HostTraffic {
                    ip: h.ip.clone(),
                    bytes_in: h.bytes_in,
                    bytes_out: h.bytes_out,
                    packets_in: h.packets_in,
                    packets_out: h.packets_out,
                    connections: 0,
                }
            })
            .collect();

        // Sort by total bytes (in + out) descending
        top_talkers.sort_by(|a, b| {
            (b.bytes_in + b.bytes_out).cmp(&(a.bytes_in + a.bytes_out))
        });
        top_talkers.truncate(20);

        // Program stats
        let programs: Vec<ProgramTraffic> = self.program_lookup.get_program_stats()
            .into_iter()
            .map(|p| ProgramTraffic {
                name: p.name,
                path: p.path,
                bytes_sent: p.bytes_sent,
                bytes_received: p.bytes_received,
                packets_sent: p.packets_sent,
                packets_received: p.packets_received,
                connections: p.connections,
            })
            .collect();

        CaptureSnapshot {
            total_packets,
            total_bytes,
            packets_per_second: total_packets as f64 / elapsed,
            bytes_per_second: total_bytes as f64 / elapsed,
            active_connections: self.connections.len(),
            protocol_stats: self.protocol_stats.read().clone(),
            top_talkers,
            capture_duration_secs: elapsed,
            programs,
        }
    }

    /// Iterate over active connections
    pub fn connections_iter(&self) -> impl Iterator<Item = dashmap::mapref::multiple::RefMulti<'_, ConnectionKey, ConnectionStats>> {
        self.connections.iter()
    }

    /// Reset all counters
    pub fn reset(&self) {
        use std::sync::atomic::Ordering;
        self.connections.clear();
        *self.protocol_stats.write() = ProtocolStats::default();
        self.host_traffic.clear();
        self.total_packets.store(0, Ordering::Relaxed);
        self.total_bytes.store(0, Ordering::Relaxed);
        self.program_lookup.reset();
    }
}

/// Parse a raw packet using etherparse and extract connection info
pub fn parse_packet(data: &[u8]) -> Option<(IpAddr, IpAddr, u16, u16, Protocol, u64)> {
    let headers = LaxPacketHeaders::from_ethernet(data).ok()?;

    let (src_ip, dst_ip) = match headers.net {
        Some(NetHeaders::Ipv4(ref h, _)) => (
            IpAddr::V4(h.source.into()),
            IpAddr::V4(h.destination.into()),
        ),
        Some(NetHeaders::Ipv6(ref h, _)) => (
            IpAddr::V6(h.source.into()),
            IpAddr::V6(h.destination.into()),
        ),
        _ => return None,
    };

    let (src_port, dst_port, proto) = match headers.transport {
        Some(TransportHeader::Tcp(ref t)) => (t.source_port, t.destination_port, Protocol::Tcp),
        Some(TransportHeader::Udp(ref u)) => (u.source_port, u.destination_port, Protocol::Udp),
        Some(TransportHeader::Icmpv4(_)) | Some(TransportHeader::Icmpv6(_)) => {
            (0, 0, Protocol::Icmp)
        }
        _ => (0, 0, Protocol::Other),
    };

    Some((src_ip, dst_ip, src_port, dst_port, proto, data.len() as u64))
}

/// List available network capture devices
pub fn list_devices() -> Vec<DeviceInfo> {
    match Device::list() {
        Ok(devices) => devices
            .into_iter()
            .map(|d| {
                let addresses: Vec<String> = d
                    .addresses
                    .iter()
                    .map(|a| a.addr.to_string())
                    .collect();
                DeviceInfo {
                    name: d.name,
                    description: d.desc.unwrap_or_default(),
                    addresses,
                }
            })
            .collect(),
        Err(e) => {
            warn!(error = %e, "Failed to list capture devices");
            Vec::new()
        }
    }
}

/// Information about a capture device
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub description: String,
    pub addresses: Vec<String>,
}

/// Start packet capture on a device in a background thread.
/// Returns a handle that can be used to stop the capture.
pub fn start_capture(
    device_name: &str,
    state: Arc<CaptureState>,
    bpf_filter: Option<&str>,
) -> Result<CaptureHandle, String> {
    let device = Device::list()
        .map_err(|e| format!("Failed to list devices: {e}"))?
        .into_iter()
        .find(|d| d.name == device_name)
        .ok_or_else(|| format!("Device '{}' not found", device_name))?;

    // Collect local addresses for direction classification
    let local_addrs: Vec<IpAddr> = device
        .addresses
        .iter()
        .map(|a| a.addr)
        .collect();
    state.set_local_addrs(local_addrs);

    let mut cap = Capture::from_device(device)
        .map_err(|e| format!("Failed to open device: {e}"))?
        .promisc(false)
        .buffer_size(2_000_000) // 2MB buffer
        .snaplen(200) // Only capture headers
        .immediate_mode(false)
        .timeout(150) // 150ms timeout for responsive stopping
        .open()
        .map_err(|e| format!("Failed to activate capture: {e}"))?;

    if let Some(filter) = bpf_filter {
        cap.filter(filter, true)
            .map_err(|e| format!("Failed to set BPF filter: {e}"))?;
    }

    let stop_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop = stop_flag.clone();

    let handle = std::thread::Builder::new()
        .name("Zentra-pcap".into())
        .spawn(move || {
            info!("Packet capture thread started");
            let mut errors = 0u32;

            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                match cap.next_packet() {
                    Ok(packet) => {
                        errors = 0;
                        if let Some((src, dst, sp, dp, proto, len)) = parse_packet(packet.data) {
                            state.record_packet(src, dst, sp, dp, proto, len);

                            // Attempt program identification for local ports
                            let is_src_local = state.is_local(&src);
                            let local_port = if is_src_local { sp } else { dp };
                            if local_port > 0 {
                                if let Some((name, path, pid)) = state.program_lookup().lookup(local_port, proto) {
                                    // Update connection with program info
                                    let key = ConnectionKey {
                                        source_ip: src,
                                        dest_ip: dst,
                                        source_port: sp,
                                        dest_port: dp,
                                        protocol: proto,
                                    };
                                    if let Some(mut conn) = state.connections.get_mut(&key) {
                                        if conn.program_name.is_none() {
                                            conn.program_name = Some(name.clone());
                                            conn.program_path = Some(path.clone());
                                            state.program_lookup().record_connection(&name);
                                        }
                                    }
                                    state.program_lookup().record_traffic(&name, &path, pid, len, 1, is_src_local);
                                }
                            }
                        }
                    }
                    Err(pcap::Error::TimeoutExpired) => {
                        // Normal timeout, just loop
                    }
                    Err(e) => {
                        errors += 1;
                        if errors <= 5 {
                            debug!(error = %e, "Capture error");
                        }
                        if errors > 100 {
                            error!("Too many capture errors, stopping");
                            break;
                        }
                    }
                }
            }
            info!("Packet capture thread stopped");
        })
        .map_err(|e| format!("Failed to spawn capture thread: {e}"))?;

    Ok(CaptureHandle {
        stop_flag,
        _thread: handle,
    })
}

/// Handle to control an active capture session
pub struct CaptureHandle {
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    _thread: std::thread::JoinHandle<()>,
}

impl CaptureHandle {
    /// Signal the capture to stop
    pub fn stop(&self) {
        self.stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if the capture is still running
    pub fn is_running(&self) -> bool {
        !self._thread.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        let devices = list_devices();
        // Should not panic; may be empty in CI
        info!("Found {} capture devices", devices.len());
    }

    #[test]
    fn test_capture_state_snapshot() {
        let state = CaptureState::new();
        state.set_local_addrs(vec!["192.168.1.100".parse().unwrap()]);

        // Simulate packets
        state.record_packet(
            "192.168.1.100".parse().unwrap(),
            "8.8.8.8".parse().unwrap(),
            12345,
            443,
            Protocol::Tcp,
            1200,
        );
        state.record_packet(
            "8.8.8.8".parse().unwrap(),
            "192.168.1.100".parse().unwrap(),
            443,
            12345,
            Protocol::Tcp,
            800,
        );
        state.record_packet(
            "192.168.1.100".parse().unwrap(),
            "1.1.1.1".parse().unwrap(),
            54321,
            53,
            Protocol::Udp,
            64,
        );

        let snap = state.snapshot();
        assert_eq!(snap.total_packets, 3);
        assert_eq!(snap.total_bytes, 2064);
        assert_eq!(snap.protocol_stats.tcp_packets, 2);
        assert_eq!(snap.protocol_stats.udp_packets, 1);
        assert!(snap.active_connections >= 2);
    }

    #[test]
    fn test_parse_packet_tcp() {
        // Craft a minimal Ethernet + IPv4 + TCP packet
        use etherparse::PacketBuilder;
        let builder = PacketBuilder::ethernet2(
            [1, 2, 3, 4, 5, 6],
            [7, 8, 9, 10, 11, 12],
        )
        .ipv4([192, 168, 1, 1], [10, 0, 0, 1], 64)
        .tcp(8080, 443, 1000, 65535);

        let payload = [0u8; 20];
        let mut buf = Vec::with_capacity(100);
        builder.write(&mut buf, &payload).unwrap();

        let result = parse_packet(&buf);
        assert!(result.is_some());
        let (src, dst, sp, dp, proto, _len) = result.unwrap();
        assert_eq!(src, IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(dst, IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(sp, 8080);
        assert_eq!(dp, 443);
        assert_eq!(proto, Protocol::Tcp);
    }
}
