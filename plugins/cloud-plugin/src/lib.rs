pub mod aws_cloudwatch;
pub mod azure_monitor;
pub mod gcp_monitoring;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use std::time::Duration;
use tracing::info;

pub struct CloudPlugin;

impl CloudPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for CloudPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "cloud-plugin",
            name: "Cloud Monitor",
            version: "0.1.0",
            description: "Monitors AWS, Azure, and GCP via their native APIs — no agents required",
            supported_subtypes: vec![
                // AWS
                "aws".into(), "aws_ec2".into(), "aws_rds".into(), "aws_elb".into(),
                "aws_s3".into(), "aws_lambda".into(), "aws_ecs".into(), "aws_eks".into(),
                // Azure
                "azure".into(), "azure_vm".into(), "azure_sql".into(), "azure_aks".into(),
                "azure_appservice".into(), "azure_storage".into(), "azure_lb".into(),
                // GCP
                "gcp".into(), "gcp_gce".into(), "gcp_cloudsql".into(), "gcp_gke".into(),
                "gcp_cloudrun".into(), "gcp_storage".into(), "gcp_lb".into(),
            ],
            capabilities: vec![
                PluginCapability::CollectMetrics,
                PluginCapability::ConnectionTest,
            ],
        }
    }

    async fn initialize(&mut self, target: &Target) -> Result<()> {
        info!(target = %target.name, "Cloud plugin initialized");
        Ok(())
    }

    async fn test_connection(&self, target: &Target) -> Result<bool> {
        let subtype = target.subtype.as_deref().unwrap_or("").to_lowercase();
        match subtype.as_str() {
            s if s.starts_with("aws") => {
                Ok(aws_cloudwatch::AwsCloudWatchCollector::test(target).await)
            }
            s if s.starts_with("azure") => {
                Ok(azure_monitor::AzureMonitorCollector::test(target).await)
            }
            s if s.starts_with("gcp") => {
                Ok(gcp_monitoring::GcpMonitoringCollector::test(target).await)
            }
            _ => Ok(false),
        }
    }

    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let subtype = target.subtype.as_deref().unwrap_or("").to_lowercase();
        let metrics = match subtype.as_str() {
            s if s.starts_with("aws") => {
                aws_cloudwatch::AwsCloudWatchCollector::collect(target).await
            }
            s if s.starts_with("azure") => {
                azure_monitor::AzureMonitorCollector::collect(target).await
            }
            s if s.starts_with("gcp") => {
                gcp_monitoring::GcpMonitoringCollector::collect(target).await
            }
            _ => vec![],
        };
        Ok(metrics)
    }

    async fn health_check(&self, _target: &Target) -> Result<bool> {
        Ok(true)
    }

    fn polling_interval(&self) -> Duration {
        Duration::from_secs(300) // 5 min — CloudWatch minimum resolution
    }
}
