//! Connection Pool Manager trait and shared HTTP client utilities.
//!
//! Provides a standardised interface for all connection pools across the platform,
//! plus a global shared `reqwest::Client` for HTTP-based monitoring endpoints.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// Pool Statistics — uniform across all pool types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    /// Human-readable pool identifier, e.g. "oracle:prod-db1", "ssh:10.0.1.5"
    pub pool_id: String,
    /// Pool type: "pg", "oracle", "ssh", "http", "redis", "custom"
    pub pool_type: String,
    /// Active / in-use connections right now
    pub active_connections: u32,
    /// Idle connections available for immediate reuse
    pub idle_connections: u32,
    /// Maximum connections allowed by this pool
    pub max_connections: u32,
    /// Total connections created since pool startup
    pub total_created: u64,
    /// Total connection acquisitions (cache hits + new)
    pub total_acquisitions: u64,
    /// Connections that timed out or failed
    pub total_errors: u64,
    /// Average time to acquire a connection (microseconds)
    pub avg_acquire_us: u64,
    /// Whether the pool is currently healthy
    pub healthy: bool,
}

// ============================================================================
// Pool Health — quick check result
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolHealth {
    pub pool_id: String,
    pub healthy: bool,
    pub message: String,
    /// Utilisation percentage (active / max * 100)
    pub utilization_pct: f64,
}

// ============================================================================
// ConnectionPoolManager trait
// ============================================================================

#[async_trait]
pub trait ConnectionPoolManager: Send + Sync {
    /// Return statistics for all pools managed by this manager.
    fn pool_stats(&self) -> Vec<PoolStats>;

    /// Quick health check across all pools.
    fn pool_health(&self) -> Vec<PoolHealth>;

    /// Evict idle connections that have exceeded their TTL.
    async fn evict_idle(&self, max_idle: Duration);

    /// Gracefully close all pools (call on shutdown).
    async fn close_all(&self);
}

// ============================================================================
// Shared HTTP Client — singleton for all HTTP-based monitoring
// ============================================================================

/// Configuration for the shared HTTP connection pool.
#[derive(Debug, Clone)]
pub struct HttpPoolConfig {
    /// Max idle connections per host (default 32)
    pub pool_idle_per_host: usize,
    /// Max total idle connections (default 200)
    pub pool_max_idle: usize,
    /// Connection timeout (default 10s)
    pub connect_timeout: Duration,
    /// Request timeout (default 30s)
    pub request_timeout: Duration,
    /// TCP keepalive interval (default 60s)
    pub tcp_keepalive: Duration,
    /// Idle connection timeout (default 90s)
    pub idle_timeout: Duration,
    /// User-Agent header
    pub user_agent: String,
}

impl Default for HttpPoolConfig {
    fn default() -> Self {
        Self {
            pool_idle_per_host: 32,
            pool_max_idle: 200,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            tcp_keepalive: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(90),
            user_agent: format!("Zentra-Monitor/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Configuration for the SSH connection multiplexing pool.
#[derive(Debug, Clone)]
pub struct SshPoolConfig {
    /// ControlMaster socket directory (default /tmp/zentra-ssh)
    pub control_dir: String,
    /// ControlPersist duration in seconds (default 600 = 10 min)
    pub control_persist_secs: u64,
    /// Maximum concurrent SSH connections per host
    pub max_per_host: usize,
    /// Connection timeout in seconds
    pub connect_timeout_secs: u64,
    /// ServerAliveInterval in seconds (default 30)
    pub server_alive_interval: u64,
    /// ServerAliveCountMax (default 3)
    pub server_alive_count_max: u32,
}

impl Default for SshPoolConfig {
    fn default() -> Self {
        Self {
            control_dir: "/tmp/zentra-ssh".into(),
            control_persist_secs: 600,
            max_per_host: 4,
            connect_timeout_secs: 10,
            server_alive_interval: 30,
            server_alive_count_max: 3,
        }
    }
}

/// Thread-safe reference to a shared HTTP client.
pub type SharedHttpClient = Arc<dyn HttpClientProvider>;

/// Trait for providing the shared HTTP client — allows mocking in tests.
pub trait HttpClientProvider: Send + Sync {
    /// Get a reference to the underlying reqwest::Client configured with
    /// connection pooling, keep-alive, timeouts, etc.
    fn client(&self) -> &reqwest::Client;
}

/// Production implementation backed by a real reqwest::Client.
pub struct DefaultHttpClient {
    inner: reqwest::Client,
    config: HttpPoolConfig,
}

impl DefaultHttpClient {
    pub fn new(config: HttpPoolConfig) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .pool_idle_timeout(config.idle_timeout)
            .pool_max_idle_per_host(config.pool_idle_per_host)
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .tcp_keepalive(config.tcp_keepalive)
            .user_agent(&config.user_agent)
            .danger_accept_invalid_certs(
                std::env::var("ZENTRA_SKIP_TLS_VERIFY")
                    .map(|v| v == "true" || v == "1")
                    .unwrap_or(false)
            )
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| format!("Failed to build shared HTTP client: {}", e))?;

        Ok(Self { inner: client, config })
    }

    pub fn with_defaults() -> Result<Self, String> {
        Self::new(HttpPoolConfig::default())
    }

    pub fn config(&self) -> &HttpPoolConfig {
        &self.config
    }
}

impl HttpClientProvider for DefaultHttpClient {
    fn client(&self) -> &reqwest::Client {
        &self.inner
    }
}
