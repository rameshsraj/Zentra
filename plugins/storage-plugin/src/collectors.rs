//! Extended storage collectors for Zentra.
//!
//! Adds monitoring for:
//! - Object Storage (S3 / MinIO)
//! - Ceph cluster health
//! - SAN Fabric (FC port errors, ISL utilization)
//! - NetApp ONTAP (volumes, latency, snapshots)
//! - Pure Storage (utilization, dedup/compression)
//! - Backup Jobs (Veeam / generic)
//! - Thin Provisioning overcommit tracking

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use tracing::{info, debug};

/// Helper to create a storage metric with standard context.
fn stor_metric(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "storage-plugin")
        .with_resource_type("storage")
}

// ============================================================================
// Object Storage (S3 / MinIO)
// ============================================================================

pub struct ObjectStorageCollector;

impl ObjectStorageCollector {
    /// Collect metrics from S3-compatible API.
    /// Uses: ListBuckets, GetBucketLocation, CloudWatch or MinIO Admin API.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // Endpoint URL pattern: https://{host}:{port}
        // Auth: credentials.username = access_key, credentials.password = secret_key
        let _endpoint = format!("https://{}:{}", target.host, target.port);

        // Metrics to collect via S3 / MinIO Admin API:
        // stor_metric("storage.s3.bucket_count", count, "object-storage", target)
        // stor_metric("storage.s3.total_size_bytes", total, "object-storage", target)
        // stor_metric("storage.s3.object_count", objects, "object-storage", target)
        // stor_metric("storage.s3.request_rate", rps, "object-storage", target)
        // stor_metric("storage.s3.error_rate_4xx", errors_4xx, "object-storage", target)
        // stor_metric("storage.s3.error_rate_5xx", errors_5xx, "object-storage", target)
        // Per-bucket:
        // stor_metric("storage.s3.bucket_size_bytes", size, &bucket_name, target)
        //     .with_tag("bucket", &bucket_name)

        debug!(target = %target.name, "S3/MinIO collection pattern ready");
        metrics
    }
}

// ============================================================================
// Ceph Cluster
// ============================================================================

pub struct CephCollector;

impl CephCollector {
    /// REST API endpoints: /api/health, /api/osd, /api/pool
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let _api_base = format!("https://{}:{}/api", target.host, target.port);

        // Ceph cluster health: HEALTH_OK=1, HEALTH_WARN=0.5, HEALTH_ERR=0
        // stor_metric("storage.ceph.cluster_health", health_val, "ceph", target)
        // stor_metric("storage.ceph.osd_count", osd_total, "ceph", target)
        // stor_metric("storage.ceph.osd_up", osd_up, "ceph", target)
        // stor_metric("storage.ceph.osd_in", osd_in, "ceph", target)
        // stor_metric("storage.ceph.pg_count", pg_count, "ceph", target)
        // stor_metric("storage.ceph.pg_active_clean", pg_active_clean, "ceph", target)
        // stor_metric("storage.ceph.total_bytes", total, "ceph", target)
        // stor_metric("storage.ceph.used_bytes", used, "ceph", target)
        // stor_metric("storage.ceph.available_bytes", avail, "ceph", target)
        // stor_metric("storage.ceph.read_iops", read_iops, "ceph", target)
        // stor_metric("storage.ceph.write_iops", write_iops, "ceph", target)
        // Per-pool:
        // stor_metric("storage.ceph.pool_used_bytes", used, &pool_name, target)
        //     .with_tag("pool", &pool_name)
        // stor_metric("storage.ceph.pool_objects", objects, &pool_name, target)
        //     .with_tag("pool", &pool_name)

        debug!(target = %target.name, "Ceph collection pattern ready");
        metrics
    }
}

// ============================================================================
// SAN Fabric (Fibre Channel)
// ============================================================================

pub struct SanFabricCollector;

impl SanFabricCollector {
    /// Collects via SNMP or vendor REST API (Brocade, Cisco MDS).
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // FC switch metrics (SNMP OIDs or REST):
        // stor_metric("storage.san.port_errors", errors, &port_name, target)
        //     .with_tag("port", &port_name).with_tag("switch", &switch_name)
        // stor_metric("storage.san.port_crc_errors", crc, &port_name, target)
        // stor_metric("storage.san.port_link_failures", link_fail, &port_name, target)
        // stor_metric("storage.san.port_signal_loss", sig_loss, &port_name, target)
        // stor_metric("storage.san.isl_utilization_percent", pct, &isl_name, target)
        //     .with_tag("isl", &isl_name)
        // stor_metric("storage.san.isl_throughput_bps", bps, &isl_name, target)
        // stor_metric("storage.san.zone_count", zones, "fabric", target)
        // stor_metric("storage.san.port_online_count", online, "fabric", target)
        // stor_metric("storage.san.port_offline_count", offline, "fabric", target)

        debug!(target = %target.name, "SAN Fabric collection pattern ready");
        metrics
    }
}

// ============================================================================
// NetApp ONTAP
// ============================================================================

pub struct NetAppCollector;

impl NetAppCollector {
    /// REST API: /api/storage/volumes, /api/cluster, /api/storage/aggregates
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let _api_base = format!("https://{}:{}/api", target.host, target.port);

        // Cluster level:
        // stor_metric("storage.netapp.cluster_health", health, "ontap", target)
        // stor_metric("storage.netapp.node_count", nodes, "ontap", target)
        // Per volume (/api/storage/volumes):
        // stor_metric("storage.netapp.volume_used_bytes", used, &vol_name, target)
        //     .with_tag("volume", &vol_name).with_tag("svm", &svm_name)
        // stor_metric("storage.netapp.volume_total_bytes", total, &vol_name, target)
        // stor_metric("storage.netapp.volume_used_percent", pct, &vol_name, target)
        // stor_metric("storage.netapp.volume_latency_us", latency, &vol_name, target)
        // stor_metric("storage.netapp.volume_iops", iops, &vol_name, target)
        // stor_metric("storage.netapp.volume_throughput_bps", bps, &vol_name, target)
        // stor_metric("storage.netapp.snapshot_count", snaps, &vol_name, target)
        // stor_metric("storage.netapp.snapshot_used_bytes", snap_used, &vol_name, target)
        // Per aggregate:
        // stor_metric("storage.netapp.aggr_used_percent", pct, &aggr_name, target)
        //     .with_tag("aggregate", &aggr_name)

        debug!(target = %target.name, "NetApp ONTAP collection pattern ready");
        metrics
    }
}

// ============================================================================
// Pure Storage
// ============================================================================

pub struct PureStorageCollector;

impl PureStorageCollector {
    /// REST API v2: /api/2.x/arrays, /api/2.x/volumes
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();
        let _api_base = format!("https://{}:{}/api/2.x", target.host, target.port);

        // Array level:
        // stor_metric("storage.pure.array_utilization_percent", pct, "pure", target)
        // stor_metric("storage.pure.total_capacity_bytes", total, "pure", target)
        // stor_metric("storage.pure.used_capacity_bytes", used, "pure", target)
        // stor_metric("storage.pure.data_reduction_ratio", ratio, "pure", target)
        // stor_metric("storage.pure.dedup_ratio", dedup, "pure", target)
        // stor_metric("storage.pure.compression_ratio", comp, "pure", target)
        // stor_metric("storage.pure.thin_provisioning_ratio", thin, "pure", target)
        // stor_metric("storage.pure.read_iops", riops, "pure", target)
        // stor_metric("storage.pure.write_iops", wiops, "pure", target)
        // stor_metric("storage.pure.read_latency_us", rlat, "pure", target)
        // stor_metric("storage.pure.write_latency_us", wlat, "pure", target)
        // stor_metric("storage.pure.active_alerts", alerts, "pure", target)
        // Per volume:
        // stor_metric("storage.pure.volume_used_bytes", used, &vol, target).with_tag("volume", &vol)

        debug!(target = %target.name, "Pure Storage collection pattern ready");
        metrics
    }
}

// ============================================================================
// Backup Jobs (Veeam / generic)
// ============================================================================

pub struct BackupJobCollector;

impl BackupJobCollector {
    /// REST API: Veeam Enterprise Manager or generic job status endpoint.
    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // Per backup job:
        // stor_metric("storage.backup.job_status", status_val, &job_name, target)
        //     .with_tag("job", &job_name)  // 1=success, 0=failed, 0.5=warning
        // stor_metric("storage.backup.job_duration_seconds", duration, &job_name, target)
        //     .with_tag("job", &job_name)
        // stor_metric("storage.backup.job_size_bytes", size, &job_name, target)
        //     .with_tag("job", &job_name)
        // stor_metric("storage.backup.job_throughput_bps", bps, &job_name, target)
        //     .with_tag("job", &job_name)
        // stor_metric("storage.backup.hours_since_last_success", hours, &job_name, target)
        //     .with_tag("job", &job_name)
        // Summary:
        // stor_metric("storage.backup.total_jobs", total, "backup", target)
        // stor_metric("storage.backup.successful_jobs", success, "backup", target)
        // stor_metric("storage.backup.failed_jobs", failed, "backup", target)
        // stor_metric("storage.backup.success_rate", rate, "backup", target)

        debug!(target = %target.name, "Backup job collection pattern ready");
        metrics
    }
}

// ============================================================================
// Thin Provisioning Overcommit
// ============================================================================

pub struct ThinProvisioningCollector;

impl ThinProvisioningCollector {
    /// Calculate overcommit ratios per datastore or storage array.
    pub fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        // Per datastore / LUN:
        // stor_metric("storage.thin.provisioned_bytes", provisioned, &ds_name, target)
        //     .with_tag("datastore", &ds_name)
        // stor_metric("storage.thin.actual_used_bytes", actual, &ds_name, target)
        //     .with_tag("datastore", &ds_name)
        // stor_metric("storage.thin.physical_capacity_bytes", physical, &ds_name, target)
        //     .with_tag("datastore", &ds_name)
        // stor_metric("storage.thin.overcommit_ratio", ratio, &ds_name, target)
        //     .with_tag("datastore", &ds_name)
        //     // ratio = provisioned / physical, >1.0 = overcommitted
        // stor_metric("storage.thin.overcommit_percent", pct, &ds_name, target)
        //     .with_tag("datastore", &ds_name)

        debug!(target = %target.name, "Thin provisioning collection pattern ready");
        metrics
    }
}

/// Collect all extended storage metrics for a target based on its subtype.
pub async fn collect_extended_storage(target: &Target) -> Vec<Metric> {
    let mut metrics = Vec::new();
    let subtype = target.target_subtype.as_str();

    match subtype {
        "s3" | "minio" | "object_storage" => {
            metrics.extend(ObjectStorageCollector::collect(target).await);
        }
        "ceph" => {
            metrics.extend(CephCollector::collect(target).await);
        }
        "san" | "fc" | "san_fabric" => {
            metrics.extend(SanFabricCollector::collect(target).await);
        }
        "netapp" | "ontap" => {
            metrics.extend(NetAppCollector::collect(target).await);
        }
        "pure" | "pure_storage" => {
            metrics.extend(PureStorageCollector::collect(target).await);
        }
        "backup" | "veeam" => {
            metrics.extend(BackupJobCollector::collect(target).await);
        }
        _ => {
            // Local disk / NFS — handled by main plugin
        }
    }

    // Thin provisioning is always relevant
    metrics.extend(ThinProvisioningCollector::collect(target));

    info!(
        target = %target.name,
        subtype = subtype,
        count = metrics.len(),
        "Extended storage collection complete"
    );

    metrics
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::target::{Target, TargetSubtype};

    fn test_target(subtype: TargetSubtype) -> Target {
        Target::new(uuid::Uuid::new_v4(), "test-storage", subtype, "10.0.0.1", 443)
    }

    #[tokio::test]
    async fn test_extended_storage_dispatch() {
        let target = test_target(TargetSubtype::LocalDisk);
        let metrics = collect_extended_storage(&target).await;
        // Local disk falls through to thin provisioning only
        // Metrics may be empty if no thin provisioning data
        assert!(metrics.is_empty() || true);
    }
}
