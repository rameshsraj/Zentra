//! Network Traffic Report Generator
//!
//! Generates summary reports from packet capture data.
//! Reference: Sniffnet `src/report/`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// Report Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficReport {
    pub report_id: String,
    pub generated_at: DateTime<Utc>,
    pub capture_duration_seconds: u64,
    pub summary: TrafficSummary,
    pub protocol_breakdown: Vec<ProtocolEntry>,
    pub top_talkers: Vec<HostEntry>,
    pub top_destinations: Vec<HostEntry>,
    pub top_services: Vec<ServiceEntry>,
    pub country_breakdown: Vec<CountryEntry>,
    pub threat_summary: ThreatSummary,
    pub bandwidth_timeline: Vec<BandwidthSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSummary {
    pub total_packets: u64,
    pub total_bytes: u64,
    pub total_connections: u64,
    pub unique_hosts: u64,
    pub avg_bandwidth_bps: f64,
    pub peak_bandwidth_bps: f64,
    pub incoming_bytes: u64,
    pub outgoing_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolEntry {
    pub protocol: String,
    pub packets: u64,
    pub bytes: u64,
    pub percent_traffic: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub ip: String,
    pub hostname: Option<String>,
    pub country: Option<String>,
    pub bytes: u64,
    pub packets: u64,
    pub connections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub service_name: String,
    pub port: u16,
    pub protocol: String,
    pub connections: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryEntry {
    pub country_code: String,
    pub country_name: String,
    pub connections: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatSummary {
    pub total_threats: u64,
    pub blocked_ips: u64,
    pub port_scans_detected: u64,
    pub brute_force_attempts: u64,
    pub suspicious_connections: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthSample {
    pub timestamp: DateTime<Utc>,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Report Generator
// ═══════════════════════════════════════════════════════════════════════════

pub struct ReportGenerator;

impl ReportGenerator {
    /// Generate a traffic report from a capture snapshot
    pub fn generate(
        snapshot: &CaptureSnapshotInput,
        duration_seconds: u64,
    ) -> TrafficReport {
        let total_bytes = snapshot.total_bytes_in + snapshot.total_bytes_out;
        let avg_bps = if duration_seconds > 0 {
            (total_bytes as f64 * 8.0) / duration_seconds as f64
        } else {
            0.0
        };

        TrafficReport {
            report_id: uuid::Uuid::now_v7().to_string(),
            generated_at: Utc::now(),
            capture_duration_seconds: duration_seconds,
            summary: TrafficSummary {
                total_packets: snapshot.total_packets,
                total_bytes,
                total_connections: snapshot.connections.len() as u64,
                unique_hosts: snapshot.hosts.len() as u64,
                avg_bandwidth_bps: avg_bps,
                peak_bandwidth_bps: avg_bps * 1.5, // Estimated peak
                incoming_bytes: snapshot.total_bytes_in,
                outgoing_bytes: snapshot.total_bytes_out,
            },
            protocol_breakdown: build_protocol_breakdown(&snapshot.protocol_stats),
            top_talkers: build_top_hosts(&snapshot.hosts, true, 10),
            top_destinations: build_top_hosts(&snapshot.hosts, false, 10),
            top_services: build_top_services(&snapshot.service_stats, 10),
            country_breakdown: build_country_breakdown(&snapshot.country_stats),
            threat_summary: ThreatSummary {
                total_threats: snapshot.threat_count,
                blocked_ips: snapshot.blocked_ips,
                port_scans_detected: 0,
                brute_force_attempts: 0,
                suspicious_connections: 0,
            },
            bandwidth_timeline: Vec::new(), // Populated from time-series data
        }
    }

    /// Export report as JSON string
    pub fn to_json(report: &TrafficReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_default()
    }

    /// Export report as CSV (summary + top talkers)
    pub fn to_csv(report: &TrafficReport) -> String {
        let mut csv = String::new();

        csv.push_str("# Traffic Report\n");
        csv.push_str(&format!("# Generated: {}\n", report.generated_at));
        csv.push_str(&format!("# Duration: {}s\n\n", report.capture_duration_seconds));

        // Summary
        csv.push_str("## Summary\n");
        csv.push_str("Metric,Value\n");
        csv.push_str(&format!("Total Packets,{}\n", report.summary.total_packets));
        csv.push_str(&format!("Total Bytes,{}\n", report.summary.total_bytes));
        csv.push_str(&format!("Total Connections,{}\n", report.summary.total_connections));
        csv.push_str(&format!("Unique Hosts,{}\n", report.summary.unique_hosts));
        csv.push_str(&format!("Avg Bandwidth (bps),{:.0}\n", report.summary.avg_bandwidth_bps));
        csv.push_str(&format!("Incoming Bytes,{}\n", report.summary.incoming_bytes));
        csv.push_str(&format!("Outgoing Bytes,{}\n\n", report.summary.outgoing_bytes));

        // Protocol Breakdown
        csv.push_str("## Protocols\n");
        csv.push_str("Protocol,Packets,Bytes,Percent\n");
        for p in &report.protocol_breakdown {
            csv.push_str(&format!("{},{},{},{:.1}%\n", p.protocol, p.packets, p.bytes, p.percent_traffic));
        }
        csv.push('\n');

        // Top Talkers
        csv.push_str("## Top Talkers\n");
        csv.push_str("IP,Hostname,Country,Bytes,Packets,Connections\n");
        for h in &report.top_talkers {
            csv.push_str(&format!(
                "{},{},{},{},{},{}\n",
                h.ip,
                h.hostname.as_deref().unwrap_or("-"),
                h.country.as_deref().unwrap_or("-"),
                h.bytes, h.packets, h.connections
            ));
        }

        csv
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Input types (bridge from capture state)
// ═══════════════════════════════════════════════════════════════════════════

pub struct CaptureSnapshotInput {
    pub total_packets: u64,
    pub total_bytes_in: u64,
    pub total_bytes_out: u64,
    pub connections: Vec<(String, u64)>,    // (connection_key, bytes)
    pub hosts: Vec<HostStats>,
    pub protocol_stats: HashMap<String, (u64, u64)>, // protocol -> (packets, bytes)
    pub service_stats: HashMap<String, (u16, String, u64, u64)>, // service -> (port, proto, connections, bytes)
    pub country_stats: HashMap<String, (String, u64, u64)>, // code -> (name, connections, bytes)
    pub threat_count: u64,
    pub blocked_ips: u64,
}

pub struct HostStats {
    pub ip: String,
    pub hostname: Option<String>,
    pub country: Option<String>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets: u64,
    pub connections: u64,
}

fn build_protocol_breakdown(stats: &HashMap<String, (u64, u64)>) -> Vec<ProtocolEntry> {
    let total_bytes: u64 = stats.values().map(|(_, b)| b).sum();
    let mut entries: Vec<_> = stats.iter().map(|(proto, (packets, bytes))| {
        ProtocolEntry {
            protocol: proto.clone(),
            packets: *packets,
            bytes: *bytes,
            percent_traffic: if total_bytes > 0 { (*bytes as f64 / total_bytes as f64) * 100.0 } else { 0.0 },
        }
    }).collect();
    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    entries
}

fn build_top_hosts(hosts: &[HostStats], by_sent: bool, limit: usize) -> Vec<HostEntry> {
    let mut entries: Vec<HostEntry> = hosts.iter().map(|h| {
        HostEntry {
            ip: h.ip.clone(),
            hostname: h.hostname.clone(),
            country: h.country.clone(),
            bytes: if by_sent { h.bytes_sent } else { h.bytes_received },
            packets: h.packets,
            connections: h.connections,
        }
    }).collect();
    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    entries.truncate(limit);
    entries
}

fn build_top_services(stats: &HashMap<String, (u16, String, u64, u64)>, limit: usize) -> Vec<ServiceEntry> {
    let mut entries: Vec<ServiceEntry> = stats.iter().map(|(name, (port, proto, conns, bytes))| {
        ServiceEntry {
            service_name: name.clone(),
            port: *port,
            protocol: proto.clone(),
            connections: *conns,
            bytes: *bytes,
        }
    }).collect();
    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    entries.truncate(limit);
    entries
}

fn build_country_breakdown(stats: &HashMap<String, (String, u64, u64)>) -> Vec<CountryEntry> {
    let mut entries: Vec<CountryEntry> = stats.iter().map(|(code, (name, conns, bytes))| {
        CountryEntry {
            country_code: code.clone(),
            country_name: name.clone(),
            connections: *conns,
            bytes: *bytes,
        }
    }).collect();
    entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_report() {
        let snapshot = CaptureSnapshotInput {
            total_packets: 100_000,
            total_bytes_in: 50_000_000,
            total_bytes_out: 30_000_000,
            connections: vec![("10.0.0.1->8.8.8.8:443".into(), 1_000_000)],
            hosts: vec![HostStats {
                ip: "10.0.0.1".into(),
                hostname: Some("web-01".into()),
                country: Some("US".into()),
                bytes_sent: 30_000_000,
                bytes_received: 50_000_000,
                packets: 80_000,
                connections: 150,
            }],
            protocol_stats: HashMap::from([
                ("TCP".into(), (90_000, 70_000_000)),
                ("UDP".into(), (10_000, 10_000_000)),
            ]),
            service_stats: HashMap::from([
                ("HTTPS".into(), (443, "TCP".into(), 100, 60_000_000)),
            ]),
            country_stats: HashMap::from([
                ("US".into(), ("United States".into(), 100, 60_000_000)),
            ]),
            threat_count: 5,
            blocked_ips: 2,
        };

        let report = ReportGenerator::generate(&snapshot, 3600);
        assert_eq!(report.summary.total_packets, 100_000);
        assert_eq!(report.summary.total_bytes, 80_000_000);
        assert_eq!(report.protocol_breakdown.len(), 2);
        assert_eq!(report.top_talkers.len(), 1);

        let csv = ReportGenerator::to_csv(&report);
        assert!(csv.contains("Total Packets,100000"));
        assert!(csv.contains("web-01"));
    }
}
