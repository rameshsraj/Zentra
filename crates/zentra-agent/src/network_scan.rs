//! Network scanner — discovers reachable hosts in the agent's local subnet
//! and reports them to the Zentra server every 10 minutes.
//!
//! Discovery strategy (no root/CAP_NET_RAW required):
//!   1. Detect local subnets from interface addresses.
//!   2. TCP-connect probe a set of common ports on every IP in the subnet.
//!   3. Any IP that responds on at least one port is considered "discovered".
//!   4. POST the list to /api/agents/:id/discovered.

use reqwest::Client;
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Ports we TCP-probe to detect live hosts.
const PROBE_PORTS: &[u16] = &[22, 80, 443, 8080, 3306, 5432, 6379, 9090, 27017];
/// Timeout per port probe.
const PROBE_TIMEOUT_MS: u64 = 300;
/// Maximum hosts to scan per subnet (avoid /8 subnets).
const MAX_HOSTS_PER_SUBNET: u32 = 254;
/// How often to run a full scan (seconds).
const SCAN_INTERVAL_SECS: u64 = 600;

pub struct NetworkScanner {
    client: Client,
    api_endpoint: String,
    api_key: String,
    agent_id: Uuid,
    cancel: CancellationToken,
}

impl NetworkScanner {
    pub fn new(
        api_endpoint: String,
        api_key: String,
        agent_id: Uuid,
        cancel: CancellationToken,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to build network scanner HTTP client");

        Self {
            client,
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_key,
            agent_id,
            cancel,
        }
    }

    pub async fn run(self) {
        info!("Network scanner started ({}s interval)", SCAN_INTERVAL_SECS);

        // Run first scan after a short delay so agent is fully connected
        tokio::time::sleep(Duration::from_secs(30)).await;

        let mut ticker = tokio::time::interval(Duration::from_secs(SCAN_INTERVAL_SECS));

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("Network scanner stopping (shutdown)");
                    break;
                }
                _ = ticker.tick() => {}
            }

            let hosts = self.scan_local_subnets().await;
            if hosts.is_empty() {
                debug!("Network scan found no reachable hosts");
                continue;
            }

            info!(count = hosts.len(), "Network scan complete, reporting to server");
            if let Err(e) = self.report_hosts(&hosts).await {
                warn!(error = %e, "Failed to report discovered hosts");
            }
        }
    }

    /// Detect local /24 subnets and probe all hosts in them.
    async fn scan_local_subnets(&self) -> Vec<DiscoveredHost> {
        let subnets = detect_local_subnets();
        if subnets.is_empty() {
            warn!("No local subnets detected — skipping scan");
            return vec![];
        }

        let mut all_hosts = Vec::new();
        for subnet_base in subnets {
            let mut found = scan_subnet(subnet_base).await;
            all_hosts.append(&mut found);
        }
        all_hosts
    }

    async fn report_hosts(&self, hosts: &[DiscoveredHost]) -> Result<(), String> {
        let entries: Vec<serde_json::Value> = hosts
            .iter()
            .map(|h| {
                json!({
                    "ip_address": h.ip.to_string(),
                    "hostname": h.hostname,
                    "open_ports": h.open_ports,
                    "os_hint": h.os_hint,
                })
            })
            .collect();

        let url = format!(
            "{}/api/agents/{}/discovered",
            self.api_endpoint, self.agent_id
        );

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({ "hosts": entries }))
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Server returned {status}: {body}"));
        }

        Ok(())
    }
}

struct DiscoveredHost {
    ip: Ipv4Addr,
    hostname: Option<String>,
    open_ports: Vec<u16>,
    os_hint: Option<String>,
}

/// Returns the base addresses of all local /24 IPv4 subnets (host part zeroed).
fn detect_local_subnets() -> Vec<Ipv4Addr> {
    let mut subnets = Vec::new();

    // Read /proc/net/fib_trie on Linux to enumerate interfaces
    if let Ok(content) = std::fs::read_to_string("/proc/net/fib_trie") {
        for line in content.lines() {
            let trimmed = line.trim();
            // Lines like "   10.88.11.162  LOCAL" contain the local IPs
            if trimmed.ends_with("LOCAL") {
                let ip_str = trimmed.split_whitespace().next().unwrap_or("");
                if let Ok(IpAddr::V4(ip)) = ip_str.parse::<IpAddr>() {
                    if !ip.is_loopback() && !ip.is_unspecified() {
                        let octets = ip.octets();
                        // Build /24 base (zero the host octet)
                        let base = Ipv4Addr::new(octets[0], octets[1], octets[2], 0);
                        if !subnets.contains(&base) {
                            subnets.push(base);
                        }
                    }
                }
            }
        }
    }

    // Fallback: try to parse hostname -I output
    if subnets.is_empty() {
        if let Ok(output) = std::process::Command::new("hostname").arg("-I").output() {
            if let Ok(s) = std::str::from_utf8(&output.stdout) {
                for token in s.split_whitespace() {
                    if let Ok(IpAddr::V4(ip)) = token.parse::<IpAddr>() {
                        if !ip.is_loopback() {
                            let octets = ip.octets();
                            let base = Ipv4Addr::new(octets[0], octets[1], octets[2], 0);
                            if !subnets.contains(&base) {
                                subnets.push(base);
                            }
                        }
                    }
                }
            }
        }
    }

    subnets
}

/// Probes all 254 host addresses in a /24 subnet concurrently.
async fn scan_subnet(base: Ipv4Addr) -> Vec<DiscoveredHost> {
    let octets = base.octets();
    let mut handles = Vec::new();

    for host_octet in 1u8..=254 {
        // Limit to MAX_HOSTS_PER_SUBNET — already 254 for /24 but guard explicitly
        if (host_octet as u32) > MAX_HOSTS_PER_SUBNET {
            break;
        }
        let ip = Ipv4Addr::new(octets[0], octets[1], octets[2], host_octet);
        handles.push(tokio::spawn(probe_host(ip)));
    }

    let mut found = Vec::new();
    for handle in handles {
        if let Ok(Some(host)) = handle.await {
            found.push(host);
        }
    }
    found
}

/// TCP-probes a single IP on PROBE_PORTS. Returns Some if any port responds.
async fn probe_host(ip: Ipv4Addr) -> Option<DiscoveredHost> {
    let mut open_ports = Vec::new();

    for &port in PROBE_PORTS {
        let addr = SocketAddr::new(IpAddr::V4(ip), port);
        if timeout(
            Duration::from_millis(PROBE_TIMEOUT_MS),
            TcpStream::connect(addr),
        )
        .await
        .ok()
        .and_then(|r| r.ok())
        .is_some()
        {
            open_ports.push(port);
        }
    }

    if open_ports.is_empty() {
        return None;
    }

    // Infer OS hint from open ports
    let os_hint = if open_ports.contains(&22) {
        Some("linux".to_string())
    } else if open_ports.iter().any(|&p| p == 3389 || p == 445 || p == 135) {
        Some("windows".to_string())
    } else {
        Some("unknown".to_string())
    };

    // Attempt reverse DNS (best-effort, short timeout)
    let hostname = reverse_lookup(ip).await;

    Some(DiscoveredHost {
        ip,
        hostname,
        open_ports,
        os_hint,
    })
}

/// Non-blocking reverse DNS lookup with a short timeout.
async fn reverse_lookup(ip: Ipv4Addr) -> Option<String> {
    let addr = IpAddr::V4(ip);
    match timeout(
        Duration::from_millis(500),
        tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&addr)),
    )
    .await
    {
        Ok(Ok(Ok(name))) if name != ip.to_string() => Some(name),
        _ => None,
    }
}
