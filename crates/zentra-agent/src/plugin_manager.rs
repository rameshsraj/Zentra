use zentra_core::models::metric::Metric;
use zentra_core::models::target::{Target, TargetSubtype};
use zentra_core::traits::plugin::{Plugin, PluginMetadata};
use zentra_core::Result;
use std::time::Duration;
use tracing::warn;

/// Enum wrapper around concrete plugin types to avoid dyn-incompatible async trait.
pub enum PluginInstance {
    Server(server_plugin::ServerPlugin),
    Database(database_plugin::DatabasePlugin),
    Storage(storage_plugin::StoragePlugin),
    Network(network_plugin::NetworkPlugin),
}

impl PluginInstance {
    pub fn metadata(&self) -> PluginMetadata {
        match self {
            Self::Server(p) => p.metadata(),
            Self::Database(p) => p.metadata(),
            Self::Storage(p) => p.metadata(),
            Self::Network(p) => p.metadata(),
        }
    }

    pub async fn initialize(&mut self, target: &Target) -> Result<()> {
        match self {
            Self::Server(p) => p.initialize(target).await,
            Self::Database(p) => p.initialize(target).await,
            Self::Storage(p) => p.initialize(target).await,
            Self::Network(p) => p.initialize(target).await,
        }
    }

    pub async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>> {
        match self {
            Self::Server(p) => p.collect_metrics(target).await,
            Self::Database(p) => p.collect_metrics(target).await,
            Self::Storage(p) => p.collect_metrics(target).await,
            Self::Network(p) => p.collect_metrics(target).await,
        }
    }

    pub async fn health_check(&self, target: &Target) -> Result<bool> {
        match self {
            Self::Server(p) => p.health_check(target).await,
            Self::Database(p) => p.health_check(target).await,
            Self::Storage(p) => p.health_check(target).await,
            Self::Network(p) => p.health_check(target).await,
        }
    }

    pub fn polling_interval(&self) -> Duration {
        match self {
            Self::Server(p) => p.polling_interval(),
            Self::Database(p) => p.polling_interval(),
            Self::Storage(p) => p.polling_interval(),
            Self::Network(p) => p.polling_interval(),
        }
    }
}

/// Instantiates the appropriate plugin for a given target subtype
pub fn create_plugin_for_target(target: &Target) -> Option<PluginInstance> {
    match &target.target_subtype {
        TargetSubtype::Linux | TargetSubtype::Windows => {
            Some(PluginInstance::Server(server_plugin::ServerPlugin::new()))
        }
        TargetSubtype::PostgreSQL | TargetSubtype::MySQL | TargetSubtype::MariaDB => {
            Some(PluginInstance::Database(database_plugin::DatabasePlugin::new()))
        }
        TargetSubtype::LocalDisk | TargetSubtype::NFS | TargetSubtype::SAN | TargetSubtype::HP3PAR => {
            Some(PluginInstance::Storage(storage_plugin::StoragePlugin::new()))
        }
        TargetSubtype::Interface | TargetSubtype::Switch | TargetSubtype::Router | TargetSubtype::Firewall => {
            Some(PluginInstance::Network(network_plugin::NetworkPlugin::new()))
        }
        other => {
            warn!(subtype = ?other, "No plugin available for target subtype");
            None
        }
    }
}
