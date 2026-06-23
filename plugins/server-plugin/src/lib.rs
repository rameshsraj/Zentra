pub mod collectors;
pub mod linux_deep;
pub mod smart_disk_health;
pub mod redfish_inventory;
pub mod wmi_collector;

use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use std::time::Duration;
use tracing::info;

pub struct ServerPlugin;

impl ServerPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for ServerPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "server-plugin",
            name: "Server Monitor",
            version: "0.1.0",
            description: "Monitors server CPU, memory, disk, and processes",
            supported_subtypes: vec![
                "linux".into(), "unix".into(), "freebsd".into(),
                "ubuntu".into(), "centos".into(), "rhel".into(), "debian".into(),
                "windows".into(), "windows_server".into(), "winrm".into(), "wmi".into(),
                "snmp".into(), "ipmi".into(), "bmc".into(),
                "idrac".into(), "ilo".into(), "xcc".into(), "redfish".into(),
                "vsphere".into(), "esxi".into(), "vcenter".into(),
                "kubernetes".into(), "k8s".into(),
            ],
            capabilities: vec![
                PluginCapability::CollectMetrics,
                PluginCapability::ResourceDiscovery,
            ],
        }
    }

    #[tracing::instrument(skip(self, _target), fields(plugin.id = "server-plugin"))]
    async fn initialize(&mut self, _target: &Target) -> Result<()> {
        info!("Server plugin initialized");
        Ok(())
    }

    async fn test_connection(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    #[tracing::instrument(skip(self, target), fields(plugin.id = "server-plugin"))]
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<zentra_core::models::metric::Metric>> {
        let mut metrics = Vec::new();

        // Extended collectors (SNMP, IPMI, vSphere, K8s, Patch Status)
        let ext = collectors::collect_extended_server(target).await;
        metrics.extend(ext);

        // Deep Linux metrics via SSH into the remote target
        let subtype = target.target_subtype.as_str().to_lowercase();
        if matches!(subtype.as_str(), "linux" | "unix" | "freebsd" | "ubuntu" | "centos" | "rhel" | "debian") {
            let deep = linux_deep::LinuxDeepCollector::collect(target).await;
            metrics.extend(deep);
            let smart = smart_disk_health::SmartDiskHealthCollector::collect(target).await;
            metrics.extend(smart);
        }

        // Windows via WMI/WinRM (agentless)
        if matches!(subtype.as_str(), "windows" | "windows_server" | "winrm" | "wmi") {
            let win = wmi_collector::WmiCollector::collect(target).await;
            metrics.extend(win);
        }

        // Redfish hardware inventory (BMC — any server with BMC/iDRAC/iLO)
        if matches!(subtype.as_str(), "bmc" | "idrac" | "ilo" | "xcc" | "redfish" | "ipmi") {
            let redfish = redfish_inventory::RedfishCollector::collect(target).await;
            metrics.extend(redfish);
        }

        Ok(metrics)
    }

    async fn health_check(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    fn polling_interval(&self) -> Duration {
        Duration::from_secs(30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::target::{Target, TargetSubtype};

    #[tokio::test]
    async fn test_server_plugin() {
        let mut plugin = ServerPlugin::new();
        let target = Target::new(
            uuid::Uuid::new_v4(),
            "test-server",
            TargetSubtype::Windows,
            "127.0.0.1",
            0,
        );
        plugin.initialize(&target).await.unwrap();
        let metrics = plugin.collect_metrics(&target).await.unwrap();
        assert!(!metrics.is_empty());
        // Should have at least CPU and memory
        let names: Vec<&str> = metrics.iter().map(|m| m.metric_name.as_str()).collect();
        assert!(names.contains(&"server.cpu_usage"));
        assert!(names.contains(&"server.memory_usage"));
    }
}
