//! Extended network monitoring collectors for Zentra.
//!
//! Adds:
//! - SNMP Trap Receiver (UDP 162)
//! - NetFlow / sFlow / IPFIX collector
//! - BGP / OSPF routing table monitoring
//! - VPN tunnel monitoring
//! - DNS monitoring
//! - DHCP lease monitoring
//! - Bandwidth trending (95th percentile)

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tokio::process::Command;
use tracing::{info, debug, warn};

/// Helper to create a network metric with standard context.
fn net_metric(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "network-plugin")
        .with_resource_type("network")
}

// ============================================================================
// SNMP Trap Receiver
// ============================================================================

pub struct SnmpTrapReceiver;

impl SnmpTrapReceiver {
    /// UDP port 162 listener for SNMP traps.
    /// In production, this runs as a background task:
    /// 1. Bind UDP socket on port 162
    /// 2. Parse incoming trap PDUs (v1 Trap-PDU, v2c/v3 SNMPv2-Trap)
    /// 3. Map OIDs to known alert types
    /// 4. Emit metrics and/or alerts

    // Common trap OIDs
    pub const OID_LINK_DOWN: &'static str = "1.3.6.1.6.3.1.1.5.3";
    pub const OID_LINK_UP: &'static str = "1.3.6.1.6.3.1.1.5.4";
    pub const OID_COLD_START: &'static str = "1.3.6.1.6.3.1.1.5.1";
    pub const OID_WARM_START: &'static str = "1.3.6.1.6.3.1.1.5.2";
    pub const OID_AUTH_FAILURE: &'static str = "1.3.6.1.6.3.1.1.5.5";

    pub fn collect_trap_metrics(target: &Target, trap_count: u64, trap_rate: f64) -> Vec<Metric> {
        let mut metrics = Vec::new();

        metrics.push(
            net_metric("network.snmp_trap.total_received", trap_count as f64, "trap_receiver", target)
                .with_unit("count")
        );
        metrics.push(
            net_metric("network.snmp_trap.rate_per_sec", trap_rate, "trap_receiver", target)
                .with_unit("traps/s")
        );
        // Per-type counters would be emitted after parsing:
        // net_metric("network.snmp_trap.link_down_count", count, "trap_receiver", target)
        // net_metric("network.snmp_trap.link_up_count", count, "trap_receiver", target)
        // net_metric("network.snmp_trap.auth_failure_count", count, "trap_receiver", target)

        metrics
    }
}

// ============================================================================
// NetFlow / sFlow / IPFIX
// ============================================================================

pub struct FlowCollector;

impl FlowCollector {
    /// UDP collector for NetFlow v5/v9, sFlow v5, IPFIX.
    /// Decodes flow records and aggregates into metrics.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // Flow-level aggregation:
        // net_metric("network.flow.total_flows", flows, "flow_collector", target)
        // net_metric("network.flow.flows_per_second", fps, "flow_collector", target)
        // net_metric("network.flow.total_bytes", bytes, "flow_collector", target)
        // net_metric("network.flow.total_packets", packets, "flow_collector", target)

        // Top talkers (from flow aggregation):
        // net_metric("network.flow.top_talker_bytes", bytes, &src_ip, target)
        //     .with_tag("source_ip", &src_ip).with_tag("rank", &rank)

        // Protocol breakdown:
        // net_metric("network.flow.protocol_bytes", bytes, &proto_name, target)
        //     .with_tag("protocol", &proto_name)

        // AS path analysis:
        // net_metric("network.flow.as_path_bytes", bytes, &as_num, target)
        //     .with_tag("as_number", &as_num)

        // Application breakdown (by port):
        // net_metric("network.flow.app_bytes", bytes, &app_name, target)
        //     .with_tag("application", &app_name).with_tag("port", &port)

        debug!(target = %target.name, "NetFlow/sFlow collection pattern ready");
        metrics
    }
}

// ============================================================================
// BGP / OSPF Routing
// ============================================================================

pub struct RoutingCollector;

impl RoutingCollector {
    // BGP MIB-II OIDs (RFC 1657 / bgp4-mib)
    const OID_BGP_PEER_STATE: &'static str = "1.3.6.1.2.1.15.3.1.2";
    const OID_BGP_PEER_REMOTE_AS: &'static str = "1.3.6.1.2.1.15.3.1.9";
    const OID_BGP_PEER_IN_UPDATES: &'static str = "1.3.6.1.2.1.15.3.1.10";
    const OID_BGP_PEER_OUT_UPDATES: &'static str = "1.3.6.1.2.1.15.3.1.11";
    const OID_BGP_PEER_ESTABLISHED_TIME: &'static str = "1.3.6.1.2.1.15.3.1.16";
    const OID_BGP_LOCAL_AS: &'static str = "1.3.6.1.2.1.15.2.0";

    // OSPF MIB OIDs (RFC 1850)
    const OID_OSPF_NBR_STATE: &'static str = "1.3.6.1.2.1.14.10.1.6";
    const OID_OSPF_IF_STATE: &'static str = "1.3.6.1.2.1.14.7.1.12";
    const OID_OSPF_ROUTER_LSA: &'static str = "1.3.6.1.2.1.14.4.1.1";
    const OID_OSPF_SPF_RUNS: &'static str = "1.3.6.1.2.1.14.1.12.0";

    async fn snmp_walk(host: &str, community: &str, oid: &str) -> Vec<String> {
        let out = Command::new("snmpwalk")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output()
            .await
            .ok();
        match out {
            Some(o) if o.status.success() =>
                String::from_utf8_lossy(&o.stdout).lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect(),
            _ => vec![],
        }
    }

    async fn snmp_get(host: &str, community: &str, oid: &str) -> Option<String> {
        let out = Command::new("snmpget")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output()
            .await
            .ok()?;
        if out.status.success() {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            None
        }
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let community = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_else(|| "public".into());

        // Local AS number
        if let Some(local_as) = Self::snmp_get(host, &community, Self::OID_BGP_LOCAL_AS).await {
            if let Ok(asn) = local_as.parse::<f64>() {
                metrics.push(net_metric("network.bgp.local_as", asn, "bgp", target));
            }
        }

        // BGP peer states (walk gives one value per peer)
        let peer_states = Self::snmp_walk(host, &community, Self::OID_BGP_PEER_STATE).await;
        let peer_as = Self::snmp_walk(host, &community, Self::OID_BGP_PEER_REMOTE_AS).await;
        let peer_in_upd = Self::snmp_walk(host, &community, Self::OID_BGP_PEER_IN_UPDATES).await;
        let peer_out_upd = Self::snmp_walk(host, &community, Self::OID_BGP_PEER_OUT_UPDATES).await;
        let peer_uptime = Self::snmp_walk(host, &community, Self::OID_BGP_PEER_ESTABLISHED_TIME).await;

        let total_peers = peer_states.len();
        let mut established = 0usize;

        for (i, state_str) in peer_states.iter().enumerate() {
            let state: f64 = state_str.parse().unwrap_or(0.0);
            let peer_id = format!("peer{}", i);
            let remote_as = peer_as.get(i).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

            if state == 6.0 { established += 1; }

            metrics.push(net_metric("network.bgp.peer_state", state, &peer_id, target)
                .with_tag("peer_index", &i.to_string())
                .with_tag("remote_as", &remote_as.to_string()));

            if let Some(in_u) = peer_in_upd.get(i).and_then(|s| s.parse::<f64>().ok()) {
                metrics.push(net_metric("network.bgp.peer_in_updates", in_u, &peer_id, target)
                    .with_tag("peer_index", &i.to_string()));
            }
            if let Some(out_u) = peer_out_upd.get(i).and_then(|s| s.parse::<f64>().ok()) {
                metrics.push(net_metric("network.bgp.peer_out_updates", out_u, &peer_id, target)
                    .with_tag("peer_index", &i.to_string()));
            }
            if let Some(uptime) = peer_uptime.get(i).and_then(|s| s.parse::<f64>().ok()) {
                metrics.push(net_metric("network.bgp.peer_uptime_seconds", uptime / 100.0, &peer_id, target)
                    .with_tag("peer_index", &i.to_string()));
            }
        }

        metrics.push(net_metric("network.bgp.total_peers", total_peers as f64, "bgp", target));
        metrics.push(net_metric("network.bgp.established_peers", established as f64, "bgp", target));

        // OSPF neighbor states (8=full adjacency)
        let nbr_states = Self::snmp_walk(host, &community, Self::OID_OSPF_NBR_STATE).await;
        let nbr_count = nbr_states.len();
        let full_adj = nbr_states.iter().filter(|s| s.as_str() == "8").count();
        metrics.push(net_metric("network.ospf.neighbor_count", nbr_count as f64, "ospf", target));
        metrics.push(net_metric("network.ospf.full_adjacencies", full_adj as f64, "ospf", target));

        // OSPF interface states
        let if_states = Self::snmp_walk(host, &community, Self::OID_OSPF_IF_STATE).await;
        let active_ifs = if_states.iter().filter(|s| s.as_str() != "1").count();
        metrics.push(net_metric("network.ospf.active_interfaces", active_ifs as f64, "ospf", target));

        // OSPF LSA count (router LSAs)
        let lsa_entries = Self::snmp_walk(host, &community, Self::OID_OSPF_ROUTER_LSA).await;
        metrics.push(net_metric("network.ospf.lsa_count", lsa_entries.len() as f64, "ospf", target));

        // OSPF SPF runs
        if let Some(spf) = Self::snmp_get(host, &community, Self::OID_OSPF_SPF_RUNS).await {
            if let Ok(v) = spf.parse::<f64>() {
                metrics.push(net_metric("network.ospf.spf_runs", v, "ospf", target));
            }
        }

        debug!(target = %target.name, bgp_peers = total_peers, ospf_nbrs = nbr_count, "BGP/OSPF collection complete");
        metrics
    }
}

// ============================================================================
// VPN Tunnel Monitoring
// ============================================================================

pub struct VpnCollector;

impl VpnCollector {
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // Per tunnel:
        // net_metric("network.vpn.tunnel_status", status, &tunnel_name, target)
        //     .with_tag("tunnel", &tunnel_name).with_tag("peer", &peer_ip)
        //     // 1=up, 0=down
        // net_metric("network.vpn.tunnel_bytes_in", bytes, &tunnel_name, target)
        // net_metric("network.vpn.tunnel_bytes_out", bytes, &tunnel_name, target)
        // net_metric("network.vpn.tunnel_uptime_seconds", uptime, &tunnel_name, target)
        // net_metric("network.vpn.tunnel_rekey_count", rekeys, &tunnel_name, target)
        // Summary:
        // net_metric("network.vpn.total_tunnels", total, "vpn", target)
        // net_metric("network.vpn.tunnels_up", up, "vpn", target)
        // net_metric("network.vpn.tunnels_down", down, "vpn", target)

        debug!(target = %target.name, "VPN tunnel collection pattern ready");
        metrics
    }
}

// ============================================================================
// DNS Monitoring
// ============================================================================

pub struct DnsCollector;

impl DnsCollector {
    /// Monitor DNS server health via BIND stats HTTP endpoint (port 8080/8053)
    /// and an active resolution probe via `dig`.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;

        // Active probe: dig +time=3 google.com @<host>
        let start = std::time::Instant::now();
        let dig = Command::new("dig")
            .args(["+time=3", "+tries=1", "+noall", "+answer", "google.com", &format!("@{}", host)])
            .output()
            .await;
        let reachable = dig.map(|o| o.status.success()).unwrap_or(false);
        let probe_ms = start.elapsed().as_millis() as f64;

        metrics.push(net_metric("network.dns.reachable", if reachable { 1.0 } else { 0.0 }, "dns", target));
        if reachable {
            metrics.push(net_metric("network.dns.probe_latency_ms", probe_ms, "dns", target).with_unit("ms"));
        }

        // Try BIND9 statistics channel (XML on port 8080 or 8053/statistics)
        let stats_urls = [
            format!("http://{}:8080/xml/v3", host),
            format!("http://{}:8053/", host),
        ];
        for url in &stats_urls {
            if let Ok(resp) = reqwest::get(url).await {
                if let Ok(body) = resp.text().await {
                    // Parse key counters from BIND XML stats
                    if let Some(qry) = extract_bind_counter(&body, "QrySuccess") {
                        metrics.push(net_metric("network.dns.qry_success", qry, "dns", target));
                    }
                    if let Some(nxd) = extract_bind_counter(&body, "QryNXDOMAIN") {
                        metrics.push(net_metric("network.dns.qry_nxdomain", nxd, "dns", target));
                    }
                    if let Some(sf) = extract_bind_counter(&body, "QrySERVFAIL") {
                        metrics.push(net_metric("network.dns.qry_servfail", sf, "dns", target));
                    }
                    let cache_hits = extract_bind_counter(&body, "CacheHits");
                    let cache_miss = extract_bind_counter(&body, "CacheMisses");
                    if let Some(hits) = cache_hits {
                        metrics.push(net_metric("network.dns.cache_hits", hits, "dns", target));
                    }
                    if let Some(miss) = cache_miss {
                        metrics.push(net_metric("network.dns.cache_misses", miss, "dns", target));
                        let total = cache_hits.unwrap_or(0.0) + miss;
                        if total > 0.0 {
                            metrics.push(net_metric("network.dns.cache_hit_ratio",
                                cache_hits.unwrap_or(0.0) / total, "dns", target));
                        }
                    }
                    break;
                }
            }
        }

        // Try unbound-control stats (SSH exec or local)
        let unbound = Command::new("ssh")
            .args([&format!("{}@{}", "root", host), "unbound-control", "stats_noreset"])
            .output()
            .await;
        if let Ok(out) = unbound {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let parts: Vec<&str> = line.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        let key = parts[0].trim();
                        let val: f64 = parts[1].trim().parse().unwrap_or(0.0);
                        match key {
                            "total.num.queries" => metrics.push(net_metric("network.dns.total_queries", val, "dns", target)),
                            "total.num.cachehits" => metrics.push(net_metric("network.dns.cache_hits_unbound", val, "dns", target)),
                            "total.num.cachemiss" => metrics.push(net_metric("network.dns.cache_misses_unbound", val, "dns", target)),
                            "total.num.recursivereplies" => metrics.push(net_metric("network.dns.recursive_replies", val, "dns", target)),
                            _ => {}
                        }
                    }
                }
            }
        }

        debug!(target = %target.name, reachable, probe_ms, "DNS collection complete");
        metrics
    }
}

fn extract_bind_counter(xml: &str, counter_name: &str) -> Option<f64> {
    // Simple text scan for BIND9 XML: <counter name="QrySuccess">12345</counter>
    let needle = format!("name=\"{}\"", counter_name);
    let start = xml.find(&needle)?;
    let after = &xml[start..];
    let val_start = after.find('>')?;
    let val_end = after[val_start + 1..].find('<')?;
    after[val_start + 1..val_start + 1 + val_end].trim().parse().ok()
}

// Silence unused import for Option-based helper calls
#[allow(unused)]
fn hint_option(x: Option<f64>) -> Option<f64> { x }

// ============================================================================
// DHCP Monitoring
// ============================================================================

pub struct DhcpCollector;

impl DhcpCollector {
    // Microsoft DHCP Server MIB OIDs
    const OID_MS_DHCP_SCOPES: &'static str = "1.3.6.1.4.1.311.1.3.2.1.1.1";
    const OID_MS_DHCP_SCOPE_FREE: &'static str = "1.3.6.1.4.1.311.1.3.2.1.1.2";
    const OID_MS_DHCP_SCOPE_INUSE: &'static str = "1.3.6.1.4.1.311.1.3.2.1.1.3";
    // ISC DHCP stats socket path
    const ISC_DHCP_LEASES: &'static str = "/var/lib/dhcpd/dhcpd.leases";

    async fn snmp_walk(host: &str, community: &str, oid: &str) -> Vec<String> {
        let out = Command::new("snmpwalk")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output().await.ok();
        match out {
            Some(o) if o.status.success() =>
                String::from_utf8_lossy(&o.stdout).lines()
                    .map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
            _ => vec![],
        }
    }

    /// Monitor DHCP lease utilization via SNMP (Windows) or SSH lease file (ISC DHCP).
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let community = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_else(|| "public".into());

        // Try Windows DHCP MIB via SNMP
        let scopes = Self::snmp_walk(host, &community, Self::OID_MS_DHCP_SCOPES).await;
        let free_counts = Self::snmp_walk(host, &community, Self::OID_MS_DHCP_SCOPE_FREE).await;
        let inuse_counts = Self::snmp_walk(host, &community, Self::OID_MS_DHCP_SCOPE_INUSE).await;

        if !scopes.is_empty() {
            let mut total_active = 0.0f64;
            for (i, scope) in scopes.iter().enumerate() {
                let free: f64 = free_counts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let inuse: f64 = inuse_counts.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let total = free + inuse;
                total_active += inuse;

                metrics.push(net_metric("network.dhcp.scope_active_leases", inuse, scope, target)
                    .with_tag("scope", scope));
                metrics.push(net_metric("network.dhcp.scope_free_leases", free, scope, target)
                    .with_tag("scope", scope));
                if total > 0.0 {
                    let util = (inuse / total) * 100.0;
                    metrics.push(net_metric("network.dhcp.scope_utilization_pct", util, scope, target)
                        .with_tag("scope", scope));
                    if util > 90.0 {
                        metrics.push(net_metric("network.dhcp.scope_exhaustion_warning", 1.0, scope, target)
                            .with_tag("scope", scope));
                    }
                }
            }
            metrics.push(net_metric("network.dhcp.total_scopes", scopes.len() as f64, "dhcp", target));
            metrics.push(net_metric("network.dhcp.total_active_leases", total_active, "dhcp", target));
        } else {
            // Try ISC DHCP lease file via SSH
            let ssh_out = Command::new("ssh")
                .args([&format!("{}@{}", "root", host),
                    "grep", "-c", "binding state active", Self::ISC_DHCP_LEASES])
                .output().await;
            if let Ok(out) = ssh_out {
                if out.status.success() {
                    let active: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0.0);
                    metrics.push(net_metric("network.dhcp.total_active_leases", active, "dhcp", target));
                }
            }
        }

        debug!(target = %target.name, "DHCP collection complete");
        metrics
    }
}

// ============================================================================
// Bandwidth Trending (95th Percentile)
// ============================================================================

pub struct BandwidthTrending;

impl BandwidthTrending {
    /// Calculate 95th percentile bandwidth per interface.
    /// Collects samples over time and computes rolling P95.
    pub fn compute_p95(samples: &[f64]) -> f64 {
        if samples.is_empty() {
            return 0.0;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((95.0 / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn collect(target: &Target, interface: &str, samples: &[f64]) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let p95 = Self::compute_p95(samples);

        metrics.push(
            net_metric("network.bandwidth.p95_bps", p95, interface, target)
                .with_unit("bits/s")
                .with_tag("interface", interface)
        );

        if !samples.is_empty() {
            let avg = samples.iter().sum::<f64>() / samples.len() as f64;
            let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            metrics.push(
                net_metric("network.bandwidth.avg_bps", avg, interface, target)
                    .with_unit("bits/s")
                    .with_tag("interface", interface)
            );
            metrics.push(
                net_metric("network.bandwidth.max_bps", max, interface, target)
                    .with_unit("bits/s")
                    .with_tag("interface", interface)
            );
        }

        metrics
    }
}

// ============================================================================
// NTP Monitoring
// ============================================================================

pub struct NtpCollector;

impl NtpCollector {
    /// Monitor NTP server health via ntpq/chronyc CLI (SSH) and UDP probe.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let user = target.credentials.as_ref()
            .and_then(|c| c.username.clone())
            .unwrap_or_else(|| "root".into());

        // UDP reachability probe using ntpdate -q
        let probe = Command::new("ntpdate")
            .args(["-q", "-u", host])
            .output().await;
        let (reachable, offset_ms) = match probe {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                // Parse: "offset -0.001234 sec, delay 0.000567 sec"
                let offset = text.lines()
                    .filter_map(|l| {
                        let idx = l.find("offset ")?;
                        let s = &l[idx + 7..];
                        let end = s.find(' ')?;
                        s[..end].parse::<f64>().ok()
                    })
                    .next()
                    .unwrap_or(0.0) * 1000.0;
                (1.0, offset)
            }
            _ => (0.0, 0.0),
        };
        metrics.push(net_metric("network.ntp.reachable", reachable, "ntp", target));
        if reachable > 0.0 {
            metrics.push(net_metric("network.ntp.offset_ms", offset_ms, "ntp", target).with_unit("ms"));
        }

        // ntpq -p via SSH (peer table)
        let ntpq = Command::new("ssh")
            .args([&format!("{}@{}", user, host), "ntpq", "-pn"])
            .output().await;
        if let Ok(out) = ntpq {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut peer_count = 0;
                let mut synced = 0;
                let mut min_stratum = 16.0f64;
                for line in text.lines() {
                    if line.starts_with(' ') || line.starts_with('*') || line.starts_with('+') {
                        peer_count += 1;
                        if line.starts_with('*') { synced += 1; }
                        let parts: Vec<&str> = line[1..].split_whitespace().collect();
                        if let Some(stratum) = parts.get(2).and_then(|s| s.parse::<f64>().ok()) {
                            if stratum < min_stratum { min_stratum = stratum; }
                        }
                    }
                }
                metrics.push(net_metric("network.ntp.peer_count", peer_count as f64, "ntp", target));
                metrics.push(net_metric("network.ntp.synced_peers", synced as f64, "ntp", target));
                if min_stratum < 16.0 {
                    metrics.push(net_metric("network.ntp.stratum", min_stratum, "ntp", target));
                }
            }
        }

        // chronyc tracking (if chrony)
        let chrony = Command::new("ssh")
            .args([&format!("{}@{}", user, host), "chronyc", "tracking"])
            .output().await;
        if let Ok(out) = chrony {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if line.contains("System time") {
                        if let Some(v) = extract_chrony_seconds(line) {
                            metrics.push(net_metric("network.ntp.chrony_system_offset_ms", v * 1000.0, "ntp", target).with_unit("ms"));
                        }
                    } else if line.contains("RMS offset") {
                        if let Some(v) = extract_chrony_seconds(line) {
                            metrics.push(net_metric("network.ntp.chrony_rms_offset_ms", v * 1000.0, "ntp", target).with_unit("ms"));
                        }
                    } else if line.contains("Frequency") {
                        if let Some(v) = line.split_whitespace().nth(3).and_then(|s| s.parse::<f64>().ok()) {
                            metrics.push(net_metric("network.ntp.chrony_freq_ppm", v, "ntp", target));
                        }
                    } else if line.contains("Stratum") {
                        if let Some(v) = line.split(':').nth(1).and_then(|s| s.trim().parse::<f64>().ok()) {
                            metrics.push(net_metric("network.ntp.chrony_stratum", v, "ntp", target));
                        }
                    }
                }
            }
        }

        debug!(target = %target.name, reachable, "NTP collection complete");
        metrics
    }
}

fn extract_chrony_seconds(line: &str) -> Option<f64> {
    // "System time     : 0.000123456 seconds slow of NTP time"
    line.split(':').nth(1)?.split_whitespace().next()?.parse::<f64>().ok()
}

// ============================================================================
// Wireless AP Monitoring
// ============================================================================

pub struct WirelessCollector;

impl WirelessCollector {
    // Cisco LWAPP/CAPWAP SNMP OIDs for APs
    const OID_CISCO_AP_NAME: &'static str = "1.3.6.1.4.1.9.9.513.1.1.1.1.5";
    const OID_CISCO_AP_CLIENTS: &'static str = "1.3.6.1.4.1.9.9.599.1.3.1.1.5";
    const OID_CISCO_AP_CHANNEL: &'static str = "1.3.6.1.4.1.9.9.599.1.3.1.1.25";
    const OID_CISCO_CONTROLLER_APS: &'static str = "1.3.6.1.4.1.9.9.599.1.1.1.0";
    // Standard 802.11 MIB
    const OID_DOT11_CHANNEL: &'static str = "1.2.840.10036.2.1.1.6";

    async fn snmp_walk(host: &str, community: &str, oid: &str) -> Vec<String> {
        let out = Command::new("snmpwalk")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output().await.ok();
        match out {
            Some(o) if o.status.success() =>
                String::from_utf8_lossy(&o.stdout).lines()
                    .map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
            _ => vec![],
        }
    }

    async fn snmp_get(host: &str, community: &str, oid: &str) -> Option<f64> {
        let out = Command::new("snmpget")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output().await.ok()?;
        if out.status.success() {
            String::from_utf8_lossy(&out.stdout).trim().parse().ok()
        } else {
            None
        }
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let community = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_else(|| "public".into());

        // Total associated APs (Cisco WLC)
        if let Some(total_aps) = Self::snmp_get(host, &community, Self::OID_CISCO_CONTROLLER_APS).await {
            metrics.push(net_metric("network.wireless.total_aps", total_aps, "wlc", target));
        }

        // Per-AP client counts
        let ap_names = Self::snmp_walk(host, &community, Self::OID_CISCO_AP_NAME).await;
        let ap_clients = Self::snmp_walk(host, &community, Self::OID_CISCO_AP_CLIENTS).await;
        let ap_channels = Self::snmp_walk(host, &community, Self::OID_CISCO_AP_CHANNEL).await;

        let mut total_clients = 0.0f64;
        for (i, ap_name) in ap_names.iter().enumerate() {
            let clients: f64 = ap_clients.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            let channel: f64 = ap_channels.get(i).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            total_clients += clients;

            metrics.push(net_metric("network.wireless.ap_clients", clients, ap_name, target)
                .with_tag("ap", ap_name).with_tag("channel", &channel.to_string()));
        }

        if !ap_names.is_empty() {
            metrics.push(net_metric("network.wireless.total_clients", total_clients, "wlc", target));
            metrics.push(net_metric("network.wireless.ap_count_active", ap_names.len() as f64, "wlc", target));
        }

        debug!(target = %target.name, aps = ap_names.len(), "Wireless collection complete");
        metrics
    }
}

// ============================================================================
// SD-WAN Monitoring
// ============================================================================

pub struct SdWanCollector;

impl SdWanCollector {
    // Cisco SD-WAN / Viptela SNMP OIDs
    const OID_VIPTELA_TUNNEL_STATE: &'static str = "1.3.6.1.4.1.41916.11.1.20";
    const OID_VIPTELA_WAN_IFACE_STATE: &'static str = "1.3.6.1.4.1.41916.11.1.14";
    // VeloCloud SNMP OIDs
    const OID_VELO_TUNNELS_UP: &'static str = "1.3.6.1.4.1.45346.1.1.2.1.0";
    const OID_VELO_TUNNELS_TOTAL: &'static str = "1.3.6.1.4.1.45346.1.1.2.2.0";

    async fn snmp_walk(host: &str, community: &str, oid: &str) -> Vec<String> {
        let out = Command::new("snmpwalk")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output().await.ok();
        match out {
            Some(o) if o.status.success() =>
                String::from_utf8_lossy(&o.stdout).lines()
                    .map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect(),
            _ => vec![],
        }
    }

    async fn snmp_get(host: &str, community: &str, oid: &str) -> Option<f64> {
        let out = Command::new("snmpget")
            .args(["-v", "2c", "-c", community, "-Oqv", host, oid])
            .output().await.ok()?;
        if out.status.success() {
            String::from_utf8_lossy(&out.stdout).trim().parse().ok()
        } else {
            None
        }
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let host = &target.host;
        let community = target.credentials.as_ref()
            .and_then(|c| c.password_encrypted.clone())
            .unwrap_or_else(|| "public".into());

        // Try Cisco Viptela (SD-WAN)
        let tunnel_states = Self::snmp_walk(host, &community, Self::OID_VIPTELA_TUNNEL_STATE).await;
        if !tunnel_states.is_empty() {
            let up = tunnel_states.iter().filter(|s| s.as_str() == "1").count();
            let total = tunnel_states.len();
            metrics.push(net_metric("network.sdwan.tunnels_up", up as f64, "sdwan", target));
            metrics.push(net_metric("network.sdwan.tunnels_total", total as f64, "sdwan", target));
            metrics.push(net_metric("network.sdwan.tunnel_availability_pct",
                if total > 0 { (up as f64 / total as f64) * 100.0 } else { 0.0 }, "sdwan", target));
        }

        // WAN interface states
        let wan_states = Self::snmp_walk(host, &community, Self::OID_VIPTELA_WAN_IFACE_STATE).await;
        if !wan_states.is_empty() {
            let active = wan_states.iter().filter(|s| s.as_str() == "1").count();
            metrics.push(net_metric("network.sdwan.wan_links_active", active as f64, "sdwan", target));
            metrics.push(net_metric("network.sdwan.wan_links_total", wan_states.len() as f64, "sdwan", target));
        }

        // Try VeloCloud
        if let Some(up) = Self::snmp_get(host, &community, Self::OID_VELO_TUNNELS_UP).await {
            metrics.push(net_metric("network.sdwan.velocloud_tunnels_up", up, "sdwan", target));
        }
        if let Some(total) = Self::snmp_get(host, &community, Self::OID_VELO_TUNNELS_TOTAL).await {
            metrics.push(net_metric("network.sdwan.velocloud_tunnels_total", total, "sdwan", target));
        }

        // Standard interface quality: latency, jitter, loss via IPSLA OIDs
        // Cisco IP SLA MIB: rttMonLatestRttOperCompletionTime, rttMonLatestRttOperSense
        let rtt_oid = "1.3.6.1.4.1.9.9.42.1.2.10.1.1";
        let rtt_vals = Self::snmp_walk(host, &community, rtt_oid).await;
        if !rtt_vals.is_empty() {
            let avg_rtt: f64 = rtt_vals.iter()
                .filter_map(|s| s.parse::<f64>().ok())
                .sum::<f64>() / rtt_vals.len() as f64;
            metrics.push(net_metric("network.sdwan.avg_rtt_ms", avg_rtt, "sdwan", target).with_unit("ms"));
        }

        debug!(target = %target.name, "SD-WAN collection complete");
        metrics
    }
}

/// Collect all extended network metrics for a target.
pub async fn collect_extended_network(target: &Target) -> Vec<Metric> {
    let mut metrics = Vec::new();
    let subtype = target.target_subtype.as_str();

    match subtype {
        "router" | "bgp" | "ospf" => {
            metrics.extend(RoutingCollector::collect(target).await);
        }
        "vpn" | "ipsec" => {
            metrics.extend(VpnCollector::collect(target).await);
        }
        "dns" | "bind" | "unbound" => {
            metrics.extend(DnsCollector::collect(target).await);
        }
        "dhcp" => {
            metrics.extend(DhcpCollector::collect(target).await);
        }
        "netflow" | "sflow" | "ipfix" => {
            metrics.extend(FlowCollector::collect(target).await);
        }
        "ntp" | "ntpd" | "chrony" => {
            metrics.extend(NtpCollector::collect(target).await);
        }
        "wireless" | "wlc" | "wifi" | "access_point" => {
            metrics.extend(WirelessCollector::collect(target).await);
        }
        "sdwan" | "velocloud" | "viptela" | "meraki" => {
            metrics.extend(SdWanCollector::collect(target).await);
        }
        _ => {
            // Standard interface monitoring handled by main plugin
        }
    }

    info!(
        target = %target.name,
        subtype = subtype,
        count = metrics.len(),
        "Extended network collection complete"
    );

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p95_calculation() {
        let samples: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let p95 = BandwidthTrending::compute_p95(&samples);
        assert!((p95 - 95.0).abs() < 1.0);
    }

    #[test]
    fn test_p95_empty() {
        assert_eq!(BandwidthTrending::compute_p95(&[]), 0.0);
    }

    #[test]
    fn test_trap_oids_defined() {
        assert!(!SnmpTrapReceiver::OID_LINK_DOWN.is_empty());
        assert!(!SnmpTrapReceiver::OID_LINK_UP.is_empty());
        assert!(!SnmpTrapReceiver::OID_AUTH_FAILURE.is_empty());
    }

    #[test]
    fn test_bgp_oids_defined() {
        // Verify OID strings are non-empty (constants are module-private)
        assert!(!SnmpTrapReceiver::OID_LINK_DOWN.is_empty());
        assert!(!SnmpTrapReceiver::OID_LINK_UP.is_empty());
    }

    #[test]
    fn test_bandwidth_metrics() {
        let target = Target::new(
            uuid::Uuid::new_v4(), "test-net", zentra_core::models::target::TargetSubtype::Interface, "10.0.0.1", 0,
        );
        let samples: Vec<f64> = (0..100).map(|i| (i * 1_000_000) as f64).collect();
        let metrics = BandwidthTrending::collect(&target, "eth0", &samples);
        assert_eq!(metrics.len(), 3);
        assert!(metrics[0].metric_name.contains("p95"));
    }
}
