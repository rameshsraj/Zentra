use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Broad category of monitoring target
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Network,
    Database,
    Storage,
    Server,
    Middleware,
}

impl TargetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Database => "database",
            Self::Storage => "storage",
            Self::Server => "server",
            Self::Middleware => "middleware",
        }
    }
}

/// Specific subtype within a target type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TargetSubtype {
    // Network
    Interface,
    Switch,
    Router,
    Firewall,
    Bgp,
    Ospf,
    Vpn,
    Dns,
    Dhcp,
    NetFlow,
    SFlow,
    Ipfix,
    // Database
    PostgreSQL,
    MySQL,
    MariaDB,
    Oracle,
    MongoDB,
    Redis,
    // Storage
    LocalDisk,
    NFS,
    SAN,
    HP3PAR,
    S3,
    MinIO,
    Ceph,
    NetApp,
    PureStorage,
    Veeam,
    SanFabric,
    // Server
    Linux,
    Windows,
    Container,
    Kubernetes,
    Snmp,
    Ipmi,
    BMC,
    VSphere,
    ESXi,
    VCenter,
    // Middleware
    Nginx,
    Apache,
    Elasticsearch,
    HAProxy,
    Tomcat,
    PhpFpm,
    RabbitMQ,
    Kafka,
    Memcached,
}

impl TargetSubtype {
    pub fn parent_type(&self) -> TargetType {
        match self {
            Self::Interface | Self::Switch | Self::Router | Self::Firewall
            | Self::Bgp | Self::Ospf | Self::Vpn | Self::Dns | Self::Dhcp
            | Self::NetFlow | Self::SFlow | Self::Ipfix => TargetType::Network,

            Self::PostgreSQL | Self::MySQL | Self::MariaDB | Self::Oracle
            | Self::MongoDB | Self::Redis => TargetType::Database,

            Self::LocalDisk | Self::NFS | Self::SAN | Self::HP3PAR
            | Self::S3 | Self::MinIO | Self::Ceph | Self::NetApp
            | Self::PureStorage | Self::Veeam | Self::SanFabric => TargetType::Storage,

            Self::Linux | Self::Windows | Self::Container | Self::Kubernetes
            | Self::Snmp | Self::Ipmi | Self::BMC | Self::VSphere
            | Self::ESXi | Self::VCenter => TargetType::Server,

            Self::Nginx | Self::Apache | Self::Elasticsearch | Self::HAProxy
            | Self::Tomcat | Self::PhpFpm | Self::RabbitMQ | Self::Kafka
            | Self::Memcached => TargetType::Middleware,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interface => "interface",
            Self::Switch => "switch",
            Self::Router => "router",
            Self::Firewall => "firewall",
            Self::Bgp => "bgp",
            Self::Ospf => "ospf",
            Self::Vpn => "vpn",
            Self::Dns => "dns",
            Self::Dhcp => "dhcp",
            Self::NetFlow => "netflow",
            Self::SFlow => "sflow",
            Self::Ipfix => "ipfix",
            Self::PostgreSQL => "postgresql",
            Self::MySQL => "mysql",
            Self::MariaDB => "mariadb",
            Self::Oracle => "oracle",
            Self::MongoDB => "mongodb",
            Self::Redis => "redis",
            Self::LocalDisk => "localdisk",
            Self::NFS => "nfs",
            Self::SAN => "san",
            Self::HP3PAR => "hp3par",
            Self::S3 => "s3",
            Self::MinIO => "minio",
            Self::Ceph => "ceph",
            Self::NetApp => "netapp",
            Self::PureStorage => "pure_storage",
            Self::Veeam => "veeam",
            Self::SanFabric => "san_fabric",
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Container => "container",
            Self::Kubernetes => "kubernetes",
            Self::Snmp => "snmp",
            Self::Ipmi => "ipmi",
            Self::BMC => "bmc",
            Self::VSphere => "vsphere",
            Self::ESXi => "esxi",
            Self::VCenter => "vcenter",
            Self::Nginx => "nginx",
            Self::Apache => "apache",
            Self::Elasticsearch => "elasticsearch",
            Self::HAProxy => "haproxy",
            Self::Tomcat => "tomcat",
            Self::PhpFpm => "phpfpm",
            Self::RabbitMQ => "rabbitmq",
            Self::Kafka => "kafka",
            Self::Memcached => "memcached",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TargetStatus {
    Pending,
    Connected,
    Healthy,
    Warning,
    Critical,
    Unreachable,
    Disabled,
}

/// Monitoring target — a database, server, storage, or network device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub target_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub target_type: TargetType,
    pub target_subtype: TargetSubtype,
    pub host: String,
    pub port: u16,
    pub credentials: Option<TargetCredentials>,
    pub connection_options: BTreeMap<String, serde_json::Value>,
    pub status: TargetStatus,
    pub environment: String,
    pub tags: BTreeMap<String, String>,
    pub check_interval_seconds: u32,
    pub last_check_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetCredentials {
    pub username: Option<String>,
    pub password_encrypted: Option<String>,
    pub database_name: Option<String>,
    pub ssl_enabled: bool,
}

impl TargetCredentials {
    /// Decrypt `password_encrypted`, returning the plaintext.
    /// Returns empty string if the field is `None` or empty.
    /// Falls back to the raw value if decryption fails (supports unencrypted legacy data).
    pub fn decrypt_password(&self) -> String {
        match &self.password_encrypted {
            Some(enc) if !enc.is_empty() => {
                crate::crypto::decrypt(enc).unwrap_or_else(|_| enc.clone())
            }
            _ => String::new(),
        }
    }
}

impl Target {
    pub fn new(
        org_id: Uuid,
        name: impl Into<String>,
        target_subtype: TargetSubtype,
        host: impl Into<String>,
        port: u16,
    ) -> Self {
        let now = Utc::now();
        Self {
            target_id: Uuid::now_v7(),
            org_id,
            name: name.into(),
            description: None,
            target_type: target_subtype.parent_type(),
            target_subtype,
            host: host.into(),
            port,
            credentials: None,
            connection_options: BTreeMap::new(),
            status: TargetStatus::Pending,
            environment: "production".into(),
            tags: BTreeMap::new(),
            check_interval_seconds: 60,
            last_check_at: None,
            last_error: None,
            consecutive_failures: 0,
            created_at: now,
            updated_at: now,
            is_active: true,
        }
    }

    pub fn with_credentials(mut self, creds: TargetCredentials) -> Self {
        self.credentials = Some(creds);
        self
    }

    pub fn with_environment(mut self, env: impl Into<String>) -> Self {
        self.environment = env.into();
        self
    }

    pub fn record_success(&mut self) {
        self.status = TargetStatus::Healthy;
        self.last_check_at = Some(Utc::now());
        self.last_error = None;
        self.consecutive_failures = 0;
        self.updated_at = Utc::now();
    }

    pub fn record_failure(&mut self, error: impl Into<String>) {
        self.consecutive_failures += 1;
        self.last_check_at = Some(Utc::now());
        self.last_error = Some(error.into());
        self.updated_at = Utc::now();

        self.status = if self.consecutive_failures >= 5 {
            TargetStatus::Critical
        } else if self.consecutive_failures >= 2 {
            TargetStatus::Warning
        } else {
            TargetStatus::Unreachable
        };
    }
}

/// Form data for creating a new target from the UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTargetRequest {
    pub name: String,
    pub description: Option<String>,
    pub target_subtype: TargetSubtype,
    pub host: String,
    #[serde(default)]
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub database_name: Option<String>,
    #[serde(default)]
    pub ssl_enabled: bool,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub check_interval_seconds: Option<u32>,
}

fn default_environment() -> String { "production".to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_creation() {
        let org = Uuid::new_v4();
        let t = Target::new(org, "Main DB", TargetSubtype::PostgreSQL, "10.0.0.1", 5432);
        assert_eq!(t.target_type, TargetType::Database);
        assert_eq!(t.status, TargetStatus::Pending);
        assert!(t.is_active);
    }

    #[test]
    fn test_target_failure_escalation() {
        let org = Uuid::new_v4();
        let mut t = Target::new(org, "DB", TargetSubtype::PostgreSQL, "10.0.0.1", 5432);

        t.record_failure("connection refused");
        assert_eq!(t.status, TargetStatus::Unreachable);
        assert_eq!(t.consecutive_failures, 1);

        t.record_failure("timeout");
        assert_eq!(t.status, TargetStatus::Warning);

        for _ in 0..3 {
            t.record_failure("timeout");
        }
        assert_eq!(t.status, TargetStatus::Critical);

        t.record_success();
        assert_eq!(t.status, TargetStatus::Healthy);
        assert_eq!(t.consecutive_failures, 0);
    }

    #[test]
    fn test_subtype_parent() {
        assert_eq!(TargetSubtype::PostgreSQL.parent_type(), TargetType::Database);
        assert_eq!(TargetSubtype::Linux.parent_type(), TargetType::Server);
        assert_eq!(TargetSubtype::Interface.parent_type(), TargetType::Network);
        assert_eq!(TargetSubtype::HP3PAR.parent_type(), TargetType::Storage);
        // New subtypes
        assert_eq!(TargetSubtype::Bgp.parent_type(), TargetType::Network);
        assert_eq!(TargetSubtype::Ceph.parent_type(), TargetType::Storage);
        assert_eq!(TargetSubtype::VSphere.parent_type(), TargetType::Server);
        assert_eq!(TargetSubtype::Oracle.parent_type(), TargetType::Database);
    }

    #[test]
    fn test_subtype_as_str() {
        assert_eq!(TargetSubtype::PostgreSQL.as_str(), "postgresql");
        assert_eq!(TargetSubtype::Bgp.as_str(), "bgp");
        assert_eq!(TargetSubtype::Ceph.as_str(), "ceph");
        assert_eq!(TargetSubtype::VSphere.as_str(), "vsphere");
        assert_eq!(TargetSubtype::PureStorage.as_str(), "pure_storage");
    }
}
