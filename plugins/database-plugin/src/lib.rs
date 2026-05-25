pub mod oracle;
pub mod oracle_connection;
pub mod oracle_metrics_loader;
pub mod oracle_metric_executor;
pub mod vault_provider;
pub mod oracle_alertlog;
pub mod oracle_multi_db;
pub mod oracle_scheduler;
pub mod oracle_cache;
pub mod oracle_txeventq;
pub mod oracle_vault;
pub mod target_pool_cache;

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use zentra_core::traits::plugin::{Plugin, PluginMetadata, PluginCapability};
use zentra_core::Result;
use std::time::Duration;
use tracing::info;

pub struct DatabasePlugin;

impl DatabasePlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for DatabasePlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "database-plugin",
            name: "Database Monitor",
            version: "0.1.0",
            description: "Monitors PostgreSQL, MySQL, MariaDB databases",
            supported_subtypes: vec![
                "postgresql".into(), "mysql".into(), "mariadb".into(),
                "oracle".into(), "mongodb".into(), "redis".into(),
            ],
            capabilities: vec![
                PluginCapability::CollectMetrics,
                PluginCapability::CollectLogs,
                PluginCapability::ConnectionTest,
            ],
        }
    }

    async fn initialize(&mut self, target: &Target) -> Result<()> {
        info!(target = %target.name, "Database plugin initialized");
        Ok(())
    }

    async fn test_connection(&self, target: &Target) -> Result<bool> {
        // TCP connectivity check
        let addr = format!("{}:{}", target.host, target.port);
        match tokio::net::TcpStream::connect(&addr).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        let mut metrics = Vec::new();

        // Test connectivity as a basic metric
        let reachable = self.test_connection(target).await.unwrap_or(false);
        metrics.push(
            Metric::new("database.reachable", if reachable { 1.0 } else { 0.0 }, &target.name)
                .with_context(target.org_id, target.target_id, "database-plugin")
                .with_resource_type("database")
                .with_unit("boolean")
        );

        // If we can connect, collect DB-specific metrics based on subtype
        if reachable {
            match target.target_subtype {
                zentra_core::models::target::TargetSubtype::Oracle => {
                    let ora_metrics = oracle::collect_oracle_metrics(target).await;
                    metrics.extend(ora_metrics);
                }
                _ => {
                    // PostgreSQL / MySQL / others
                    if let Some(ref creds) = target.credentials {
                        if let Some(ref db_name) = creds.database_name {
                            if let Ok(pg_metrics) = self.collect_pg_metrics(target, db_name).await {
                                metrics.extend(pg_metrics);
                            }
                        }
                    }
                }
            }
        }

        Ok(metrics)
    }

    async fn health_check(&self, target: &Target) -> Result<bool> {
        self.test_connection(target).await
    }

    fn polling_interval(&self) -> Duration {
        Duration::from_secs(60)
    }
}

impl DatabasePlugin {
    async fn collect_pg_metrics(&self, target: &Target, db_name: &str) -> std::result::Result<Vec<Metric>, String> {
        let creds = target.credentials.as_ref().ok_or("No credentials")?;
        let user = creds.username.as_deref().unwrap_or("postgres");
        let pass = creds.password_encrypted.as_deref().unwrap_or("");

        let key = target_pool_cache::PoolKey {
            host: target.host.clone(),
            port: target.port,
            database: db_name.to_string(),
            user: user.to_string(),
        };

        let pool = target_pool_cache::get_or_create(key, pass).await?;

        let mut metrics = Vec::new();

        // Active connections
        if let Ok(row) = sqlx::query_as::<_, (i64,)>(
            "SELECT count(*) FROM pg_stat_activity WHERE state = 'active'"
        ).fetch_one(&pool).await {
            metrics.push(
                Metric::new("database.active_connections", row.0 as f64, &target.name)
                    .with_context(target.org_id, target.target_id, "database-plugin")
                    .with_resource_type("database")
                    .with_unit("count")
            );
        }

        // Database size
        if let Ok(row) = sqlx::query_as::<_, (i64,)>(
            "SELECT pg_database_size(current_database())"
        ).fetch_one(&pool).await {
            metrics.push(
                Metric::new("database.size", row.0 as f64, &target.name)
                    .with_context(target.org_id, target.target_id, "database-plugin")
                    .with_resource_type("database")
                    .with_unit("bytes")
            );
        }

        // Transaction rates
        if let Ok(row) = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<i64>)>(
            "SELECT xact_commit, xact_rollback, tup_fetched FROM pg_stat_database WHERE datname = current_database()"
        ).fetch_one(&pool).await {
            if let Some(commits) = row.0 {
                metrics.push(
                    Metric::new("database.xact_commit", commits as f64, &target.name)
                        .with_context(target.org_id, target.target_id, "database-plugin")
                        .with_resource_type("database")
                        .with_unit("count")
                );
            }
            if let Some(rollbacks) = row.1 {
                metrics.push(
                    Metric::new("database.xact_rollback", rollbacks as f64, &target.name)
                        .with_context(target.org_id, target.target_id, "database-plugin")
                        .with_resource_type("database")
                        .with_unit("count")
                );
            }
        }

        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::target::{Target, TargetSubtype};

    #[tokio::test]
    async fn test_database_plugin_metadata() {
        let plugin = DatabasePlugin::new();
        let meta = plugin.metadata();
        assert_eq!(meta.id, "database-plugin");
        assert!(meta.supported_subtypes.contains(&"postgresql".to_string()));
    }
}
