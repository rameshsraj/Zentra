use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use chrono::Utc;
use tracing::{info, warn};
use uuid::Uuid;

use zentra_core::models::threat::{
    ThreatCategory, ThreatEvent, ThreatSeverity, ThreatStatus,
};

use crate::capture::CaptureState;

/// Network-specific threat detection engine.
/// Analyzes capture state for port scans, brute force, DDoS, data exfiltration, etc.
pub struct NetworkThreatEngine {
    org_id: Uuid,
    /// Tracks unique destination ports per source IP for port scan detection
    port_scan_tracker: HashMap<IpAddr, PortScanState>,
    /// Tracks connection attempts per source+dest+port for brute force detection
    brute_force_tracker: HashMap<(IpAddr, u16), BruteForceState>,
    /// Tracks bytes per source for exfiltration detection
    exfil_tracker: HashMap<IpAddr, ExfilState>,
    /// Active threats (deduplicated by source+category)
    active_threats: Vec<ThreatEvent>,
    /// Configuration
    config: ThreatConfig,
    /// Last analysis time
    last_analysis: Instant,
}

#[derive(Debug, Clone)]
struct PortScanState {
    unique_ports: Vec<u16>,
    first_seen: Instant,
}

#[derive(Debug, Clone)]
struct BruteForceState {
    attempts: u32,
    first_seen: Instant,
}

#[derive(Debug, Clone)]
struct ExfilState {
    bytes_out: u64,
    first_seen: Instant,
}

/// Configuration for threat detection thresholds
#[derive(Debug, Clone)]
pub struct ThreatConfig {
    /// Unique destination ports from one IP to trigger port scan alert
    pub port_scan_threshold: usize,
    /// Time window for port scan detection
    pub port_scan_window: Duration,
    /// Failed connection attempts to same port to trigger brute force alert
    pub brute_force_threshold: u32,
    /// Time window for brute force detection
    pub brute_force_window: Duration,
    /// Bytes outbound from single host to trigger exfiltration alert
    pub exfil_bytes_threshold: u64,
    /// Time window for exfiltration detection
    pub exfil_window: Duration,
    /// Connections per second to trigger DDoS alert
    pub ddos_conn_threshold: usize,
    /// Known suspicious ports (Telnet, known backdoors, etc.)
    pub suspicious_ports: Vec<u16>,
    /// Known malware callback ports
    pub malware_ports: Vec<u16>,
}

impl Default for ThreatConfig {
    fn default() -> Self {
        Self {
            port_scan_threshold: 25,
            port_scan_window: Duration::from_secs(60),
            brute_force_threshold: 10,
            brute_force_window: Duration::from_secs(120),
            exfil_bytes_threshold: 500_000_000, // 500MB
            exfil_window: Duration::from_secs(300),
            ddos_conn_threshold: 1000,
            suspicious_ports: vec![23, 445, 1080, 3128, 4444, 5555, 6666, 8888, 9999],
            malware_ports: vec![4444, 5555, 6666, 1337, 31337],
        }
    }
}

impl NetworkThreatEngine {
    pub fn new(org_id: Uuid, config: ThreatConfig) -> Self {
        Self {
            org_id,
            port_scan_tracker: HashMap::new(),
            brute_force_tracker: HashMap::new(),
            exfil_tracker: HashMap::new(),
            active_threats: Vec::new(),
            config,
            last_analysis: Instant::now(),
        }
    }

    /// Analyze current capture state and return newly detected threats
    pub fn analyze(&mut self, state: &CaptureState) -> Vec<ThreatEvent> {
        let now = Instant::now();

        // Expire old tracking data
        self.expire_old_data(now);

        // Snapshot connection data to avoid borrow conflicts
        let connections: Vec<_> = state
            .connections_iter()
            .map(|entry| {
                let c = entry.value();
                (
                    c.source_ip.clone(),
                    c.dest_ip.clone(),
                    c.source_port,
                    c.dest_port,
                    c.bytes_sent,
                    c.packets_sent,
                )
            })
            .collect();

        // Phase 1: Update trackers (mutable borrow of tracker maps only)
        for (src_str, _dst_str, _sp, dp, bytes_sent, pkts_sent) in &connections {
            let src_ip: IpAddr = match src_str.parse() {
                Ok(ip) => ip,
                Err(_) => continue,
            };

            // Port scan tracking
            let ps = self.port_scan_tracker
                .entry(src_ip)
                .or_insert_with(|| PortScanState {
                    unique_ports: Vec::new(),
                    first_seen: now,
                });
            if !ps.unique_ports.contains(dp) {
                ps.unique_ports.push(*dp);
            }

            // Brute force tracking
            let bf_ports = [22, 3389, 5900, 3306, 5432];
            if bf_ports.contains(dp) {
                let key = (src_ip, *dp);
                let bf = self.brute_force_tracker
                    .entry(key)
                    .or_insert_with(|| BruteForceState {
                        attempts: 0,
                        first_seen: now,
                    });
                bf.attempts = *pkts_sent as u32;
            }

            // Exfiltration tracking
            let exfil = self.exfil_tracker
                .entry(src_ip)
                .or_insert_with(|| ExfilState {
                    bytes_out: 0,
                    first_seen: now,
                });
            exfil.bytes_out += bytes_sent;
        }

        // Phase 2: Detect threats (reads trackers + active_threats for dedup)
        let mut new_threats = Vec::new();

        // Port scan detection
        for (src_ip, ps) in &self.port_scan_tracker {
            if ps.unique_ports.len() >= self.config.port_scan_threshold {
                let threat = ThreatEvent::new(
                    self.org_id,
                    ThreatCategory::PortScan,
                    ThreatSeverity::High,
                    format!("Port scan detected: {} scanned {} unique ports", src_ip, ps.unique_ports.len()),
                )
                .with_source(&src_ip.to_string())
                .with_evidence(format!("Ports: {:?}", &ps.unique_ports[..ps.unique_ports.len().min(20)]))
                .with_mitre("Reconnaissance", "T1046");

                if !self.is_duplicate_threat(&threat) {
                    info!(source = %src_ip, ports = ps.unique_ports.len(), "Port scan detected");
                    new_threats.push(threat);
                }
            }
        }

        // Brute force detection
        for ((src_ip, port), bf) in &self.brute_force_tracker {
            if bf.attempts >= self.config.brute_force_threshold {
                let threat = ThreatEvent::new(
                    self.org_id,
                    ThreatCategory::BruteForce,
                    ThreatSeverity::High,
                    format!("Possible brute force: {} → port {} ({} attempts)", src_ip, port, bf.attempts),
                )
                .with_source(&src_ip.to_string())
                .with_port(*port)
                .with_mitre("Credential Access", "T1110");

                if !self.is_duplicate_threat(&threat) {
                    warn!(source = %src_ip, port, attempts = bf.attempts, "Brute force detected");
                    new_threats.push(threat);
                }
            }
        }

        // Suspicious port & malware callback detection
        for (src_str, dst_str, _sp, dp, _bytes, _pkts) in &connections {
            if self.config.suspicious_ports.contains(dp) {
                let threat = ThreatEvent::new(
                    self.org_id,
                    ThreatCategory::SuspiciousConnection,
                    ThreatSeverity::Medium,
                    format!("Connection to suspicious port: {} → {}:{}", src_str, dst_str, dp),
                )
                .with_source(src_str)
                .with_dest(dst_str)
                .with_port(*dp);

                if !self.is_duplicate_threat(&threat) {
                    new_threats.push(threat);
                }
            }

            if self.config.malware_ports.contains(dp) {
                let threat = ThreatEvent::new(
                    self.org_id,
                    ThreatCategory::MalwareCallback,
                    ThreatSeverity::Critical,
                    format!("Possible malware callback: {} → {}:{}", src_str, dst_str, dp),
                )
                .with_source(src_str)
                .with_dest(dst_str)
                .with_port(*dp)
                .with_mitre("Command and Control", "T1571");

                if !self.is_duplicate_threat(&threat) {
                    warn!(source = %src_str, dest = %dst_str, port = dp, "Malware callback detected");
                    new_threats.push(threat);
                }
            }
        }

        // Data exfiltration detection
        for (src_ip, exfil) in &self.exfil_tracker {
            if exfil.bytes_out >= self.config.exfil_bytes_threshold {
                let threat = ThreatEvent::new(
                    self.org_id,
                    ThreatCategory::DataExfiltration,
                    ThreatSeverity::Critical,
                    format!("Possible data exfiltration: {} sent {:.1} MB", src_ip, exfil.bytes_out as f64 / 1_000_000.0),
                )
                .with_source(&src_ip.to_string())
                .with_mitre("Exfiltration", "T1048");

                if !self.is_duplicate_threat(&threat) {
                    warn!(source = %src_ip, bytes = exfil.bytes_out, "Data exfiltration detected");
                    new_threats.push(threat);
                }
            }
        }

        // DDoS detection (via snapshot)
        let snap = state.snapshot();
        if snap.active_connections > self.config.ddos_conn_threshold {
            let threat = ThreatEvent::new(
                self.org_id,
                ThreatCategory::DDoS,
                ThreatSeverity::Critical,
                format!("Possible DDoS: {} active connections, {:.0} pkts/s", snap.active_connections, snap.packets_per_second),
            )
            .with_mitre("Impact", "T1498");

            if !self.is_duplicate_threat(&threat) {
                warn!(connections = snap.active_connections, pps = snap.packets_per_second, "DDoS detected");
                new_threats.push(threat);
            }
        }

        // Store new threats
        for t in &new_threats {
            self.active_threats.push(t.clone());
        }

        self.last_analysis = now;
        new_threats
    }

    /// Check if a similar threat already exists (dedup by category + source IP)
    fn is_duplicate_threat(&self, new_threat: &ThreatEvent) -> bool {
        self.active_threats.iter().any(|existing| {
            existing.category == new_threat.category
                && existing.source_ip == new_threat.source_ip
                && existing.status == ThreatStatus::Active
        })
    }

    /// Expire old tracking data outside the detection windows
    fn expire_old_data(&mut self, now: Instant) {
        self.port_scan_tracker.retain(|_, v| {
            now.duration_since(v.first_seen) < self.config.port_scan_window
        });
        self.brute_force_tracker.retain(|_, v| {
            now.duration_since(v.first_seen) < self.config.brute_force_window
        });
        self.exfil_tracker.retain(|_, v| {
            now.duration_since(v.first_seen) < self.config.exfil_window
        });
        // Expire resolved/old active threats
        self.active_threats.retain(|t| {
            t.status == ThreatStatus::Active
                && (Utc::now() - t.first_seen).num_minutes() < 30
        });
    }

    /// Get current active threats
    pub fn active_threats(&self) -> &[ThreatEvent] {
        &self.active_threats
    }

    /// Get total threat count
    pub fn threat_count(&self) -> usize {
        self.active_threats.len()
    }

    /// Reset all tracking state
    pub fn reset(&mut self) {
        self.port_scan_tracker.clear();
        self.brute_force_tracker.clear();
        self.exfil_tracker.clear();
        self.active_threats.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureState, Protocol};

    #[test]
    fn test_port_scan_detection() {
        let org_id = Uuid::new_v4();
        let state = CaptureState::new();
        state.set_local_addrs(vec!["192.168.1.1".parse().unwrap()]);

        // Simulate a port scan: one source hitting many destination ports
        let src: IpAddr = "10.0.0.99".parse().unwrap();
        let dst: IpAddr = "192.168.1.1".parse().unwrap();
        for port in 1..=30 {
            state.record_packet(src, dst, 54321, port, Protocol::Tcp, 64);
        }

        let mut engine = NetworkThreatEngine::new(org_id, ThreatConfig {
            port_scan_threshold: 25,
            ..Default::default()
        });

        let threats = engine.analyze(&state);
        assert!(!threats.is_empty(), "Should detect port scan");
        assert_eq!(threats[0].category, ThreatCategory::PortScan);
        assert_eq!(threats[0].severity, ThreatSeverity::High);
    }

    #[test]
    fn test_suspicious_port_detection() {
        let org_id = Uuid::new_v4();
        let state = CaptureState::new();

        // Connection to telnet port
        state.record_packet(
            "10.0.0.1".parse().unwrap(),
            "192.168.1.1".parse().unwrap(),
            12345,
            23, // Telnet
            Protocol::Tcp,
            100,
        );

        let mut engine = NetworkThreatEngine::new(org_id, ThreatConfig::default());
        let threats = engine.analyze(&state);

        let suspicious = threats.iter().find(|t| t.category == ThreatCategory::SuspiciousConnection);
        assert!(suspicious.is_some(), "Should detect suspicious port connection");
    }

    #[test]
    fn test_deduplication() {
        let org_id = Uuid::new_v4();
        let state = CaptureState::new();

        state.record_packet(
            "10.0.0.1".parse().unwrap(),
            "192.168.1.1".parse().unwrap(),
            12345,
            23,
            Protocol::Tcp,
            100,
        );

        let mut engine = NetworkThreatEngine::new(org_id, ThreatConfig::default());

        let threats1 = engine.analyze(&state);
        let threats2 = engine.analyze(&state);

        // Second analysis should not produce duplicate threats
        let suspicious1 = threats1.iter().filter(|t| t.category == ThreatCategory::SuspiciousConnection).count();
        let suspicious2 = threats2.iter().filter(|t| t.category == ThreatCategory::SuspiciousConnection).count();
        assert!(suspicious1 > 0);
        assert_eq!(suspicious2, 0, "Duplicate threats should be suppressed");
    }
}
