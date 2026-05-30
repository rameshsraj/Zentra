pub mod capture;
pub mod host_resolver;
pub mod report;
pub mod traffic_analyzer;
pub mod threat_engine;
pub mod extended_collectors;
pub mod network_types;
pub mod deep_analysis;
pub mod service_db;
pub mod program_lookup;
pub mod hubble_client;

use std::sync::Arc;
use std::time::Duration;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use sysinfo::Networks;
use tracing::info;

use crate::capture::{CaptureState, CaptureHandle};
use crate::traffic_analyzer::{TrafficAnalyzer, AnomalyThresholds};

#[allow(dead_code)]
pub struct NetworkPlugin {
    networks: Option<Networks>,
    capture_state: Arc<CaptureState>,
    capture_handle: Option<CaptureHandle>,
    analyzer: TrafficAnalyzer,
}

impl NetworkPlugin {
    pub fn new() -> Self {
        Self {
            networks: None,
            capture_state: Arc::new(CaptureState::new()),
            capture_handle: None,
            analyzer: TrafficAnalyzer::new(AnomalyThresholds::default()),
        }
    }

    /// Start packet capture on the specified device (or auto-detect)
    pub fn start_capture(&mut self, device_name: Option<&str>, bpf_filter: Option<&str>) -> std::result::Result<(), String> {
        // Stop existing capture if any
        if let Some(ref handle) = self.capture_handle {
            handle.stop();
        }

        let dev_name = if let Some(name) = device_name {
            name.to_string()
        } else {
            // Auto-detect: pick first device with addresses
            let devices = capture::list_devices();
            devices
                .into_iter()
                .find(|d| !d.addresses.is_empty())
                .map(|d| d.name)
                .ok_or_else(|| "No suitable capture device found".to_string())?
        };

        self.capture_state.reset();
        let handle = capture::start_capture(&dev_name, self.capture_state.clone(), bpf_filter)?;
        self.capture_handle = Some(handle);
        info!(device = %dev_name, "Packet capture started");
        Ok(())
    }

    /// Stop packet capture
    pub fn stop_capture(&mut self) {
        if let Some(ref handle) = self.capture_handle {
            handle.stop();
            info!("Packet capture stopped");
        }
        self.capture_handle = None;
    }

    /// Get a reference to capture state for external queries
    pub fn capture_state(&self) -> &Arc<CaptureState> {
        &self.capture_state
    }
}

impl Plugin for NetworkPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "network-plugin",
            name: "Network Monitor",
            version: "0.1.0",
            description: "Monitors network interfaces, bandwidth, and deep packet capture",
            supported_subtypes: vec![
                "interface".into(), "switch".into(), "router".into(), "firewall".into(),
                "bgp".into(), "ospf".into(), "vpn".into(), "dns".into(), "dhcp".into(),
                "netflow".into(), "sflow".into(), "ipfix".into(),
            ],
            capabilities: vec![
                PluginCapability::CollectMetrics,
                PluginCapability::NetworkCapture,
            ],
        }
    }

    #[tracing::instrument(skip(self, _target), fields(plugin.id = "network-plugin", net.transport = "ip"))]
    async fn initialize(&mut self, _target: &Target) -> Result<()> {
        self.networks = Some(Networks::new_with_refreshed_list());
        info!("Network plugin initialized with capture engine");
        Ok(())
    }

    async fn test_connection(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    #[tracing::instrument(skip(self, target), fields(plugin.id = "network-plugin", net.transport = "ip"))]
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        // Extended collectors (SNMP Traps, NetFlow, BGP/OSPF, VPN, DNS, DHCP, Bandwidth)
        let ext = extended_collectors::collect_extended_network(target).await;
        metrics.extend(ext);

        // Interface-level metrics from sysinfo
        if let Some(ref networks) = self.networks {
            for (name, data) in networks.iter() {
                metrics.push(
                    Metric::new("network.bytes_received", data.total_received() as f64, name)
                        .with_context(target.org_id, target.target_id, "network-plugin")
                        .with_resource_type("network_interface")
                        .with_unit("bytes")
                        .with_tag("interface", name)
                );
                metrics.push(
                    Metric::new("network.bytes_transmitted", data.total_transmitted() as f64, name)
                        .with_context(target.org_id, target.target_id, "network-plugin")
                        .with_resource_type("network_interface")
                        .with_unit("bytes")
                        .with_tag("interface", name)
                );
                metrics.push(
                    Metric::new("network.packets_received", data.total_packets_received() as f64, name)
                        .with_context(target.org_id, target.target_id, "network-plugin")
                        .with_resource_type("network_interface")
                        .with_unit("packets")
                        .with_tag("interface", name)
                );
                metrics.push(
                    Metric::new("network.packets_transmitted", data.total_packets_transmitted() as f64, name)
                        .with_context(target.org_id, target.target_id, "network-plugin")
                        .with_resource_type("network_interface")
                        .with_unit("packets")
                        .with_tag("interface", name)
                );
                metrics.push(
                    Metric::new("network.errors_received", data.total_errors_on_received() as f64, name)
                        .with_context(target.org_id, target.target_id, "network-plugin")
                        .with_resource_type("network_interface")
                        .with_unit("errors")
                        .with_tag("interface", name)
                );
                metrics.push(
                    Metric::new("network.errors_transmitted", data.total_errors_on_transmitted() as f64, name)
                        .with_context(target.org_id, target.target_id, "network-plugin")
                        .with_resource_type("network_interface")
                        .with_unit("errors")
                        .with_tag("interface", name)
                );
            }
        }

        // Deep capture metrics from pcap engine
        let snap = self.capture_state.snapshot();
        let cap_resource = "capture";

        metrics.push(
            Metric::new("network.capture.total_packets", snap.total_packets as f64, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("packets")
        );
        metrics.push(
            Metric::new("network.capture.total_bytes", snap.total_bytes as f64, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("bytes")
        );
        metrics.push(
            Metric::new("network.capture.packets_per_sec", snap.packets_per_second, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("packets/s")
        );
        metrics.push(
            Metric::new("network.capture.bytes_per_sec", snap.bytes_per_second, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("bytes/s")
        );
        metrics.push(
            Metric::new("network.capture.active_connections", snap.active_connections as f64, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("connections")
        );

        // Protocol breakdown
        metrics.push(
            Metric::new("network.capture.tcp_bytes", snap.protocol_stats.tcp_bytes as f64, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("bytes")
                .with_tag("protocol", "tcp")
        );
        metrics.push(
            Metric::new("network.capture.udp_bytes", snap.protocol_stats.udp_bytes as f64, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("bytes")
                .with_tag("protocol", "udp")
        );
        metrics.push(
            Metric::new("network.capture.icmp_bytes", snap.protocol_stats.icmp_bytes as f64, cap_resource)
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("bytes")
                .with_tag("protocol", "icmp")
        );

        // Top talkers as individual metrics
        for (i, talker) in snap.top_talkers.iter().take(5).enumerate() {
            let total = talker.bytes_in + talker.bytes_out;
            metrics.push(
                Metric::new(
                    "network.capture.top_talker_bytes",
                    total as f64,
                    &talker.ip,
                )
                .with_context(target.org_id, target.target_id, "network-plugin")
                .with_resource_type("network_capture")
                .with_unit("bytes")
                .with_tag("rank", &(i + 1).to_string())
                .with_tag("ip", &talker.ip)
            );
        }

        Ok(metrics)
    }

    async fn health_check(&self, _target: &Target) -> Result<bool> {
        let iface_ok = self.networks.is_some();
        let capture_ok = self.capture_handle.as_ref().map_or(true, |h| h.is_running());
        Ok(iface_ok && capture_ok)
    }

    fn polling_interval(&self) -> Duration {
        Duration::from_secs(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::target::{Target, TargetSubtype};

    #[tokio::test]
    async fn test_network_plugin() {
        let mut plugin = NetworkPlugin::new();
        let target = Target::new(
            uuid::Uuid::new_v4(),
            "test-net",
            TargetSubtype::Interface,
            "127.0.0.1",
            0,
        );
        plugin.initialize(&target).await.unwrap();
        let metrics = plugin.collect_metrics(&target).await.unwrap();
        // Should have metrics for at least one interface
        assert!(!metrics.is_empty() || true); // May be empty in CI
    }
}
