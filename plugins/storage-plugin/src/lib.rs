pub mod collectors;
pub mod netapp_ontap;
pub mod pure_storage;
pub mod ceph_deep;
pub mod veeam;
pub mod zfs_ssh;
pub mod dell_emc;
pub mod minio_deep;
pub mod ibm_gpfs;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use std::time::Duration;
use tracing::info;

pub struct StoragePlugin;

impl StoragePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for StoragePlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "storage-plugin",
            name: "Storage Monitor",
            version: "0.1.0",
            description: "Monitors local disks, NFS, SAN, and HP 3PAR storage",
            supported_subtypes: vec![
                "localdisk".into(), "nfs".into(), "san".into(), "hp3par".into(),
                "s3".into(), "minio".into(), "ceph".into(), "netapp".into(),
                "pure_storage".into(), "veeam".into(), "san_fabric".into(),
                "zfs".into(), "freenas".into(), "truenas".into(),
                "dellemc".into(), "dell_unity".into(), "powerstore".into(),
                "gpfs".into(), "spectrum_scale".into(), "ibm_gpfs".into(),
            ],
            capabilities: vec![
                PluginCapability::CollectMetrics,
                PluginCapability::ConnectionTest,
            ],
        }
    }

    #[tracing::instrument(skip(self, target), fields(plugin.id = "storage-plugin"))]
    async fn initialize(&mut self, target: &Target) -> Result<()> {
        info!(target = %target.name, "Storage plugin initialized");
        Ok(())
    }

    async fn test_connection(&self, target: &Target) -> Result<bool> {
        let addr = format!("{}:{}", target.host, target.port);
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    #[tracing::instrument(skip(self, target), fields(plugin.id = "storage-plugin"))]
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        // Legacy pattern-based collectors (S3, SAN fabric, thin provisioning)
        let ext = collectors::collect_extended_storage(target).await;
        metrics.extend(ext);

        // Route to real API collectors based on subtype
        let subtype = target.target_subtype.as_str().to_lowercase();
        match subtype.as_str() {
            "netapp" | "ontap" | "netapp_ontap" => {
                metrics.extend(netapp_ontap::NetAppOntapCollector::collect(target).await);
            }
            "pure_storage" | "pure" | "flasharray" => {
                metrics.extend(pure_storage::PureStorageCollector::collect(target).await);
            }
            "ceph" | "ceph_deep" => {
                metrics.extend(ceph_deep::CephDeepCollector::collect(target).await);
            }
            "veeam" | "veeam_vbr" => {
                metrics.extend(veeam::VeeamCollector::collect(target).await);
            }
            "zfs" | "zfs_ssh" | "freenas" | "truenas" => {
                metrics.extend(zfs_ssh::ZfsSshCollector::collect(target).await);
            }
            "dellemc" | "dell_emc" | "dell_unity" | "powerstore" | "vnx" => {
                metrics.extend(dell_emc::DellEmcCollector::collect(target).await);
            }
            "minio" | "s3_minio" => {
                metrics.extend(minio_deep::MinioDeepCollector::collect(target).await);
            }
            "gpfs" | "spectrum_scale" | "ibm_gpfs" => {
                metrics.extend(ibm_gpfs::IbmGpfsCollector::collect(target).await);
            }
            _ => {}
        }

        // Local disk metrics via sysinfo
        for disk in sysinfo::Disks::new_with_refreshed_list().iter() {
            let mount = disk.mount_point().to_string_lossy().to_string();
            let total = disk.total_space() as f64;
            let available = disk.available_space() as f64;
            let used = total - available;
            let used_pct = if total > 0.0 { (used / total) * 100.0 } else { 0.0 };

            metrics.push(
                Metric::new("storage.disk_total", total, &mount)
                    .with_context(target.org_id, target.target_id, "storage-plugin")
                    .with_resource_type("storage")
                    .with_unit("bytes")
                    .with_tag("mount", &mount)
            );
            metrics.push(
                Metric::new("storage.disk_used", used, &mount)
                    .with_context(target.org_id, target.target_id, "storage-plugin")
                    .with_resource_type("storage")
                    .with_unit("bytes")
                    .with_tag("mount", &mount)
            );
            metrics.push(
                Metric::new("storage.disk_usage_percent", used_pct, &mount)
                    .with_context(target.org_id, target.target_id, "storage-plugin")
                    .with_resource_type("storage")
                    .with_unit("percent")
                    .with_tag("mount", &mount)
            );
        }

        Ok(metrics)
    }

    async fn health_check(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    fn polling_interval(&self) -> Duration {
        Duration::from_secs(120)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::target::{Target, TargetSubtype};

    #[tokio::test]
    async fn test_storage_plugin() {
        let mut plugin = StoragePlugin::new();
        let target = Target::new(
            uuid::Uuid::new_v4(),
            "test-storage",
            TargetSubtype::LocalDisk,
            "127.0.0.1",
            0,
        );
        plugin.initialize(&target).await.unwrap();
        let metrics = plugin.collect_metrics(&target).await.unwrap();
        assert!(!metrics.is_empty());
    }
}
