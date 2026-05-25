pub mod collectors;
pub mod linux_deep;
pub mod smart_disk_health;
pub mod redfish_inventory;
pub mod wmi_collector;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use std::time::Duration;
use sysinfo::System;
use tracing::info;

pub struct ServerPlugin {
    system: Option<System>,
}

impl ServerPlugin {
    pub fn new() -> Self {
        Self { system: None }
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

    async fn initialize(&mut self, _target: &Target) -> Result<()> {
        let mut sys = System::new_all();
        sys.refresh_all();
        self.system = Some(sys);
        info!("Server plugin initialized");
        Ok(())
    }

    async fn test_connection(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        // Extended collectors (SNMP, IPMI, vSphere, K8s, Patch Status)
        let ext = collectors::collect_extended_server(target).await;
        metrics.extend(ext);

        // Deep Linux metrics (SSH-based, agentless)
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

        if let Some(ref sys) = self.system {
            // CPU
            metrics.push(
                Metric::new("server.cpu_usage", sys.global_cpu_usage() as f64, "cpu")
                    .with_context(target.org_id, target.target_id, "server-plugin")
                    .with_resource_type("server")
                    .with_unit("percent")
            );

            // Memory
            let total_mem = sys.total_memory() as f64;
            let used_mem = sys.used_memory() as f64;
            let mem_percent = if total_mem > 0.0 { (used_mem / total_mem) * 100.0 } else { 0.0 };

            metrics.push(
                Metric::new("server.memory_usage", mem_percent, "memory")
                    .with_context(target.org_id, target.target_id, "server-plugin")
                    .with_resource_type("server")
                    .with_unit("percent")
            );
            metrics.push(
                Metric::new("server.memory_total", total_mem, "memory")
                    .with_context(target.org_id, target.target_id, "server-plugin")
                    .with_resource_type("server")
                    .with_unit("bytes")
            );
            metrics.push(
                Metric::new("server.memory_used", used_mem, "memory")
                    .with_context(target.org_id, target.target_id, "server-plugin")
                    .with_resource_type("server")
                    .with_unit("bytes")
            );

            // Swap
            let used_swap = sys.used_swap() as f64;
            metrics.push(
                Metric::new("server.swap_used", used_swap, "swap")
                    .with_context(target.org_id, target.target_id, "server-plugin")
                    .with_resource_type("server")
                    .with_unit("bytes")
            );

            // Process count
            metrics.push(
                Metric::new("server.process_count", sys.processes().len() as f64, "processes")
                    .with_context(target.org_id, target.target_id, "server-plugin")
                    .with_resource_type("server")
                    .with_unit("count")
            );

            // Disks
            for disk in sysinfo::Disks::new_with_refreshed_list().iter() {
                let mount = disk.mount_point().to_string_lossy().to_string();
                let total = disk.total_space() as f64;
                let available = disk.available_space() as f64;
                let used_pct = if total > 0.0 { ((total - available) / total) * 100.0 } else { 0.0 };

                metrics.push(
                    Metric::new("server.disk_usage", used_pct, &mount)
                        .with_context(target.org_id, target.target_id, "server-plugin")
                        .with_resource_type("disk")
                        .with_unit("percent")
                        .with_tag("mount", &mount)
                );
            }
        }

        Ok(metrics)
    }

    async fn health_check(&self, _target: &Target) -> Result<bool> {
        Ok(self.system.is_some())
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
