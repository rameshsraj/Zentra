//! P2: Collector trait + 6 stub implementations
//!
//! All collectors implement the `Collector` trait and return a vector of
//! `CollectedMetric` values. The runtime scheduler calls `collect()` at the
//! configured interval.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Core types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectedMetric {
    pub name: String,
    pub value: f64,
    pub labels: HashMap<String, String>,
    pub timestamp_ms: i64,
    pub metric_type: MetricType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricType {
    Gauge,
    Counter,
    Histogram,
    Summary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorConfig {
    pub name: String,
    pub collector_type: String,
    pub interval_seconds: u64,
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub credentials: Option<HashMap<String, String>>,
    pub extra: HashMap<String, String>,
}

#[derive(Debug)]
pub struct CollectError(pub String);

impl std::fmt::Display for CollectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CollectError: {}", self.0)
    }
}
impl std::error::Error for CollectError {}

// ============================================================================
// Trait definition
// ============================================================================

#[async_trait]
pub trait Collector: Send + Sync {
    /// Unique type key, e.g. "mysql", "postgres", "nginx"
    fn collector_type(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// Default scrape interval in seconds
    fn default_interval(&self) -> u64 { 30 }

    /// Collect metrics from the configured target
    async fn collect(&self) -> Result<Vec<CollectedMetric>, CollectError>;

    /// Health-check the target without collecting full metrics
    async fn health_check(&self) -> Result<bool, CollectError> {
        Ok(true)
    }
}

// ============================================================================
// 6 Stub Collectors (P2)
// ============================================================================

pub struct MysqlCollector { pub config: CollectorConfig }
pub struct PostgresCollector { pub config: CollectorConfig }
pub struct NginxCollector { pub config: CollectorConfig }
pub struct DockerCollector { pub config: CollectorConfig }
pub struct HttpCheckCollector { pub config: CollectorConfig }
pub struct PingCollector { pub config: CollectorConfig }

macro_rules! impl_stub_collector {
    ($ty:ident, $type_str:expr, $desc:expr, $metrics:expr) => {
        #[async_trait]
        impl Collector for $ty {
            fn collector_type(&self) -> &str { $type_str }
            fn description(&self) -> &str { $desc }
            async fn collect(&self) -> Result<Vec<CollectedMetric>, CollectError> {
                let now = chrono::Utc::now().timestamp_millis();
                Ok($metrics.iter().map(|(name, val): &(&str, f64)| CollectedMetric {
                    name: name.to_string(),
                    value: *val,
                    labels: HashMap::from([
                        ("collector".to_string(), $type_str.to_string()),
                        ("endpoint".to_string(), self.config.endpoint.clone().unwrap_or_default()),
                    ]),
                    timestamp_ms: now,
                    metric_type: MetricType::Gauge,
                }).collect())
            }
        }
    };
}

impl_stub_collector!(MysqlCollector, "mysql", "MySQL/MariaDB metrics collector", vec![
    ("mysql_connections_active", 42.0),
    ("mysql_queries_per_second", 1250.0),
    ("mysql_slow_queries", 3.0),
    ("mysql_buffer_pool_hit_ratio", 98.7),
    ("mysql_replication_lag_seconds", 0.5),
    ("mysql_innodb_rows_read", 45000.0),
]);

impl_stub_collector!(PostgresCollector, "postgres", "PostgreSQL metrics collector", vec![
    ("pg_connections_active", 28.0),
    ("pg_transactions_per_second", 890.0),
    ("pg_cache_hit_ratio", 99.2),
    ("pg_replication_lag_bytes", 1024.0),
    ("pg_dead_tuples", 15000.0),
    ("pg_table_bloat_ratio", 1.12),
]);

impl_stub_collector!(NginxCollector, "nginx", "Nginx status metrics collector", vec![
    ("nginx_active_connections", 156.0),
    ("nginx_requests_per_second", 2340.0),
    ("nginx_reading", 5.0),
    ("nginx_writing", 12.0),
    ("nginx_waiting", 139.0),
    ("nginx_accepts_total", 850000.0),
]);

impl_stub_collector!(DockerCollector, "docker", "Docker container metrics collector", vec![
    ("docker_containers_running", 12.0),
    ("docker_containers_stopped", 3.0),
    ("docker_images_total", 28.0),
    ("docker_cpu_usage_percent", 34.5),
    ("docker_memory_usage_bytes", 2147483648.0),
    ("docker_network_rx_bytes", 1073741824.0),
]);

impl_stub_collector!(HttpCheckCollector, "http_check", "HTTP endpoint health checker", vec![
    ("http_check_status", 1.0),
    ("http_check_response_time_ms", 45.0),
    ("http_check_tls_days_remaining", 87.0),
    ("http_check_content_length_bytes", 12456.0),
    ("http_check_redirect_count", 1.0),
]);

impl_stub_collector!(PingCollector, "ping", "ICMP ping metrics collector", vec![
    ("ping_rtt_ms", 2.3),
    ("ping_packet_loss_percent", 0.0),
    ("ping_min_rtt_ms", 1.8),
    ("ping_max_rtt_ms", 4.2),
    ("ping_jitter_ms", 0.6),
]);

// ============================================================================
// P14: 6 More Collector Stubs
// ============================================================================

pub struct RedisCollector { pub config: CollectorConfig }
pub struct ElasticsearchCollector { pub config: CollectorConfig }
pub struct RabbitmqCollector { pub config: CollectorConfig }
pub struct ApacheCollector { pub config: CollectorConfig }
pub struct HaproxyCollector { pub config: CollectorConfig }
pub struct SnmpCollector { pub config: CollectorConfig }

impl_stub_collector!(RedisCollector, "redis", "Redis metrics collector", vec![
    ("redis_connected_clients", 45.0),
    ("redis_used_memory_bytes", 536870912.0),
    ("redis_ops_per_sec", 12500.0),
    ("redis_keyspace_hits_ratio", 97.8),
    ("redis_evicted_keys", 12.0),
    ("redis_replication_lag", 0.0),
]);

impl_stub_collector!(ElasticsearchCollector, "elasticsearch", "Elasticsearch cluster metrics", vec![
    ("es_cluster_health", 1.0),
    ("es_active_shards", 150.0),
    ("es_docs_count", 45000000.0),
    ("es_store_size_bytes", 21474836480.0),
    ("es_indexing_rate", 2500.0),
    ("es_search_rate", 850.0),
    ("es_jvm_heap_used_percent", 62.0),
]);

impl_stub_collector!(RabbitmqCollector, "rabbitmq", "RabbitMQ metrics collector", vec![
    ("rabbitmq_messages_ready", 1250.0),
    ("rabbitmq_messages_unacked", 34.0),
    ("rabbitmq_consumers", 12.0),
    ("rabbitmq_queues", 28.0),
    ("rabbitmq_connections", 45.0),
    ("rabbitmq_channels", 90.0),
    ("rabbitmq_publish_rate", 450.0),
]);

impl_stub_collector!(ApacheCollector, "apache", "Apache HTTP server metrics", vec![
    ("apache_busy_workers", 8.0),
    ("apache_idle_workers", 42.0),
    ("apache_requests_per_sec", 125.0),
    ("apache_bytes_per_sec", 524288.0),
    ("apache_uptime_seconds", 2592000.0),
]);

impl_stub_collector!(HaproxyCollector, "haproxy", "HAProxy load balancer metrics", vec![
    ("haproxy_current_sessions", 234.0),
    ("haproxy_session_rate", 45.0),
    ("haproxy_bytes_in", 1073741824.0),
    ("haproxy_bytes_out", 2147483648.0),
    ("haproxy_response_2xx", 95000.0),
    ("haproxy_response_5xx", 12.0),
    ("haproxy_backend_up", 4.0),
]);

impl_stub_collector!(SnmpCollector, "snmp", "SNMP device metrics collector", vec![
    ("snmp_if_in_octets", 8589934592.0),
    ("snmp_if_out_octets", 4294967296.0),
    ("snmp_if_in_errors", 5.0),
    ("snmp_if_out_errors", 2.0),
    ("snmp_cpu_load", 35.0),
    ("snmp_memory_used_percent", 72.0),
    ("snmp_uptime_seconds", 5184000.0),
]);
