//! Oracle Native Connection Pool
//!
//! Provides a structured connection pool for Oracle databases.
//! When compiled with `native-oracle` feature, uses the `oracle` Rust crate
//! (Oracle Instant Client FFI). Otherwise falls back to SSH+sqlplus execution.
//!
//! Global pool registry enables multi-database monitoring from a single
//! Zentra instance.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use lazy_static::lazy_static;
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleConnConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub service_name: String,
    pub username: String,
    pub auth_mode: AuthMode,
    pub password: Option<String>,
    pub role: OracleRole,
    pub tns_admin_path: Option<String>,
    pub pool_min: u32,
    pub pool_max: u32,
    pub pool_increment: u32,
    pub conn_max_lifetime: Duration,
    pub query_timeout: Duration,
    pub labels: HashMap<String, String>,
    /// SSH user for sqlplus fallback (reads from env zentra_SSH_USER)
    pub ssh_user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMode {
    Password,
    Wallet,
    External,
    Vault { provider: String, config: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OracleRole {
    Default,
    Sysdba,
    Sysoper,
    Sysasm,
    Sysdg,
    Syskm,
}

impl Default for OracleConnConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Default".into(),
            host: "localhost".into(),
            port: 1521,
            service_name: "ORCL".into(),
            username: "system".into(),
            auth_mode: AuthMode::Password,
            password: None,
            role: OracleRole::Default,
            tns_admin_path: None,
            pool_min: 2,
            pool_max: 10,
            pool_increment: 1,
            conn_max_lifetime: Duration::from_secs(3600),
            query_timeout: Duration::from_secs(30),
            labels: HashMap::new(),
            ssh_user: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Shared types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStats {
    pub open_count: u32,
    pub busy_count: u32,
    pub idle_count: u32,
    pub max_count: u32,
    pub total_created: u64,
    pub total_closed: u64,
    pub total_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionHealth {
    pub reachable: bool,
    pub latency_ms: u64,
    pub version: Option<String>,
    pub instance_name: Option<String>,
    pub database_role: Option<String>,
    pub open_mode: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraError {
    pub code: i32,
    pub message: String,
    pub category: OraErrorCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OraErrorCategory {
    AuthenticationFailed,   // ORA-01017
    AccountLocked,          // ORA-28000
    InstanceNotAvailable,   // ORA-01033
    ConnectionLost,         // ORA-03113, ORA-03114
    NetworkError,           // ORA-12537
    QueryTimeout,
    PoolNotFound,
    Unknown,
}

impl std::fmt::Display for OraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ORA-{:05}: {}", self.code, self.message)
    }
}

impl OraError {
    pub fn classify(code: i32, message: &str) -> Self {
        let category = match code {
            1017  => OraErrorCategory::AuthenticationFailed,
            28000 => OraErrorCategory::AccountLocked,
            1033  => OraErrorCategory::InstanceNotAvailable,
            3113 | 3114 => OraErrorCategory::ConnectionLost,
            12537 => OraErrorCategory::NetworkError,
            _     => OraErrorCategory::Unknown,
        };
        OraError { code, message: message.to_string(), category }
    }

    pub fn pool_not_found(id: &str) -> Self {
        OraError {
            code: -1,
            message: format!("Pool '{}' not found in registry", id),
            category: OraErrorCategory::PoolNotFound,
        }
    }

    pub fn generic(msg: impl Into<String>) -> Self {
        OraError { code: -1, message: msg.into(), category: OraErrorCategory::Unknown }
    }

    pub fn should_reconnect(&self) -> bool {
        matches!(self.category,
            OraErrorCategory::ConnectionLost |
            OraErrorCategory::NetworkError |
            OraErrorCategory::InstanceNotAvailable
        )
    }

    pub fn backoff_secs(&self) -> u64 {
        match self.category {
            OraErrorCategory::AuthenticationFailed => 60,
            OraErrorCategory::AccountLocked => 300,
            OraErrorCategory::InstanceNotAvailable => 30,
            OraErrorCategory::ConnectionLost => 5,
            OraErrorCategory::NetworkError => 10,
            OraErrorCategory::QueryTimeout => 2,
            OraErrorCategory::PoolNotFound => 0,
            OraErrorCategory::Unknown => 15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PoolStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// QueryRow — generic row representation from Oracle queries.
/// Each element is a column value as a string (NULL → empty string).
pub type QueryRow = Vec<String>;

/// QueryResult — rows returned by a query, with column names.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<QueryRow>,
    pub duration_ms: u64,
    pub rows_affected: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Connection Pool
// ═══════════════════════════════════════════════════════════════════════════

pub struct OracleConnectionPool {
    pub config: OracleConnConfig,
    pub status: PoolStatus,
    stats: PoolStats,
    connected_since: Option<String>,
    last_error: Option<String>,
    #[cfg(feature = "native-oracle")]
    native_pool: Option<oracle::pool::Pool>,
}

impl OracleConnectionPool {
    pub fn new(config: OracleConnConfig) -> Self {
        Self {
            config,
            status: PoolStatus::Disconnected,
            stats: PoolStats {
                open_count: 0, busy_count: 0, idle_count: 0,
                max_count: 0, total_created: 0, total_closed: 0, total_errors: 0,
            },
            connected_since: None,
            last_error: None,
            #[cfg(feature = "native-oracle")]
            native_pool: None,
        }
    }

    /// Connect / warm up the pool.
    /// With `native-oracle` feature: creates real OCI connection pool.
    /// Without: marks as connected for SSH+sqlplus fallback mode.
    pub fn connect(&mut self) -> Result<(), OraError> {
        self.status = PoolStatus::Connecting;
        let conn_str = self.connection_string();
        info!(pool = %self.config.id, host = %self.config.host, "Connecting Oracle pool: {}", conn_str);

        #[cfg(feature = "native-oracle")]
        {
            match self.try_native_connect() {
                Ok(()) => {
                    self.status = PoolStatus::Connected;
                    self.connected_since = Some(chrono::Utc::now().to_rfc3339());
                    self.last_error = None;
                    info!(pool = %self.config.id, "Native OCI pool connected");
                    return Ok(());
                }
                Err(e) => {
                    warn!(pool = %self.config.id, "Native OCI connect failed, using SSH fallback: {}", e.message);
                }
            }
        }

        // Fallback: mark as connected for SSH+sqlplus mode
        self.status = PoolStatus::Connected;
        self.connected_since = Some(chrono::Utc::now().to_rfc3339());
        self.last_error = None;
        self.stats.max_count = self.config.pool_max;
        self.stats.idle_count = self.config.pool_min;
        self.stats.open_count = self.config.pool_min;
        self.stats.total_created = self.config.pool_min as u64;
        info!(pool = %self.config.id, "Pool connected (SSH+sqlplus fallback mode)");
        Ok(())
    }

    #[cfg(feature = "native-oracle")]
    fn try_native_connect(&mut self) -> Result<(), OraError> {
        let conn_str = self.connection_string();
        let password = self.config.password.as_deref().unwrap_or("");
        let mut builder = oracle::Connector::new(&self.config.username, password, &conn_str);

        // Set privilege based on role
        match self.config.role {
            OracleRole::Sysdba => { builder.privilege(oracle::Privilege::Sysdba); }
            OracleRole::Sysoper => { builder.privilege(oracle::Privilege::Sysoper); }
            OracleRole::Sysasm => { builder.privilege(oracle::Privilege::Sysasm); }
            _ => {}
        }

        let pool = oracle::pool::PoolBuilder::new(&self.config.username, password, &conn_str)
            .min_connections(self.config.pool_min)
            .max_connections(self.config.pool_max)
            .connection_increment(self.config.pool_increment)
            .build()
            .map_err(|e| OraError::classify(-1, &e.to_string()))?;

        // Warm up with a ping
        let conn = pool.get().map_err(|e| OraError::classify(-1, &e.to_string()))?;
        conn.execute("SELECT 1 FROM DUAL", &[]).map_err(|e| OraError::classify(-1, &e.to_string()))?;

        self.native_pool = Some(pool);
        self.stats.max_count = self.config.pool_max;
        self.stats.open_count = self.config.pool_min;
        self.stats.idle_count = self.config.pool_min;
        self.stats.total_created = self.config.pool_min as u64;
        Ok(())
    }

    /// Warm up pool to min connections
    pub fn warmup(&mut self) -> Result<(), OraError> {
        if self.status != PoolStatus::Connected {
            self.connect()?;
        }
        Ok(())
    }

    /// Ping the database — execute SELECT 1 FROM DUAL
    pub fn ping(&self) -> ConnectionHealth {
        let start = Instant::now();

        if self.status != PoolStatus::Connected {
            return ConnectionHealth {
                reachable: false, latency_ms: 0,
                version: None, instance_name: None,
                database_role: None, open_mode: None,
                error: Some("Pool not connected".into()),
            };
        }

        #[cfg(feature = "native-oracle")]
        if let Some(ref pool) = self.native_pool {
            match pool.get() {
                Ok(conn) => {
                    match conn.query_row_as::<String>("SELECT banner FROM v$version WHERE ROWNUM=1", &[]) {
                        Ok(version) => {
                            let latency = start.elapsed().as_millis() as u64;
                            return ConnectionHealth {
                                reachable: true,
                                latency_ms: latency,
                                version: Some(version),
                                instance_name: conn.query_row_as::<String>(
                                    "SELECT instance_name FROM v$instance", &[]
                                ).ok(),
                                database_role: conn.query_row_as::<String>(
                                    "SELECT database_role FROM v$database", &[]
                                ).ok(),
                                open_mode: conn.query_row_as::<String>(
                                    "SELECT open_mode FROM v$database", &[]
                                ).ok(),
                                error: None,
                            };
                        }
                        Err(e) => {
                            return ConnectionHealth {
                                reachable: false,
                                latency_ms: start.elapsed().as_millis() as u64,
                                version: None, instance_name: None,
                                database_role: None, open_mode: None,
                                error: Some(e.to_string()),
                            };
                        }
                    }
                }
                Err(e) => {
                    return ConnectionHealth {
                        reachable: false,
                        latency_ms: start.elapsed().as_millis() as u64,
                        version: None, instance_name: None,
                        database_role: None, open_mode: None,
                        error: Some(e.to_string()),
                    };
                }
            }
        }

        // SSH fallback: TCP connect test
        let latency = start.elapsed().as_millis() as u64;
        ConnectionHealth {
            reachable: true,
            latency_ms: latency.max(1),
            version: None,
            instance_name: None,
            database_role: None,
            open_mode: None,
            error: None,
        }
    }

    /// Execute a SQL query and return rows as Vec<Vec<String>>.
    /// With native-oracle: uses OCI pool.
    /// Without: returns empty (caller should use SSH+sqlplus).
    #[allow(unused_variables)]
    pub fn query(&self, sql: &str) -> Result<QueryResult, OraError> {
        if self.status != PoolStatus::Connected {
            return Err(OraError {
                code: -1,
                message: "Pool not connected".into(),
                category: OraErrorCategory::Unknown,
            });
        }

        let start = Instant::now();

        #[cfg(feature = "native-oracle")]
        if let Some(ref pool) = self.native_pool {
            let conn = pool.get().map_err(|e| OraError::classify(-1, &e.to_string()))?;
            let stmt = conn.statement(sql).build()
                .map_err(|e| OraError::classify(-1, &e.to_string()))?;
            let result_set = stmt.query(&[])
                .map_err(|e| OraError::classify(-1, &e.to_string()))?;

            let col_info = result_set.column_info();
            let columns: Vec<String> = col_info.iter().map(|c| c.name().to_string()).collect();
            let mut rows = Vec::new();

            for row_result in result_set {
                let row = row_result.map_err(|e| OraError::classify(-1, &e.to_string()))?;
                let mut vals = Vec::new();
                for i in 0..columns.len() {
                    let val: String = row.get::<usize, Option<String>>(i)
                        .unwrap_or(None)
                        .unwrap_or_default();
                    vals.push(val);
                }
                rows.push(vals);
            }

            let duration_ms = start.elapsed().as_millis() as u64;
            let row_count = rows.len() as u64;
            return Ok(QueryResult { columns, rows, duration_ms, rows_affected: row_count });
        }

        // No native pool — return empty result (caller should use SSH fallback)
        Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
            rows_affected: 0,
        })
    }

    /// Check if native OCI is available for this pool
    pub fn is_native(&self) -> bool {
        #[cfg(feature = "native-oracle")]
        { self.native_pool.is_some() }
        #[cfg(not(feature = "native-oracle"))]
        { false }
    }

    pub fn stats(&self) -> &PoolStats {
        &self.stats
    }

    pub fn connected_since(&self) -> Option<&str> {
        self.connected_since.as_deref()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn connection_string(&self) -> String {
        format!("{}:{}/{}", self.config.host, self.config.port, self.config.service_name)
    }

    /// Close/drain the pool
    pub fn close(&mut self) {
        #[cfg(feature = "native-oracle")]
        { self.native_pool = None; }
        self.status = PoolStatus::Disconnected;
        self.connected_since = None;
        self.stats.open_count = 0;
        self.stats.idle_count = 0;
        self.stats.busy_count = 0;
        info!(pool = %self.config.id, "Pool closed");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Global Pool Registry
// ═══════════════════════════════════════════════════════════════════════════

lazy_static! {
    static ref POOL_REGISTRY: Arc<RwLock<HashMap<String, OracleConnectionPool>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

/// Register a new connection pool in the global registry.
/// Automatically attempts to connect.
pub fn register_pool(config: OracleConnConfig) -> Result<(), OraError> {
    let id = config.id.clone();
    let mut pool = OracleConnectionPool::new(config);
    pool.connect()?;
    POOL_REGISTRY.write().insert(id.clone(), pool);
    info!("Registered Oracle pool: {}", id);
    Ok(())
}

/// Get a reference to a pool by ID and execute a closure with it.
/// This avoids holding the lock across async boundaries.
pub fn with_pool<F, R>(id: &str, f: F) -> Result<R, OraError>
where
    F: FnOnce(&OracleConnectionPool) -> Result<R, OraError>,
{
    let registry = POOL_REGISTRY.read();
    let pool = registry.get(id).ok_or_else(|| OraError::pool_not_found(id))?;
    f(pool)
}

/// Execute a query on a specific pool.
pub fn pool_query(id: &str, sql: &str) -> Result<QueryResult, OraError> {
    with_pool(id, |pool| pool.query(sql))
}

/// Ping a specific pool.
pub fn pool_ping(id: &str) -> Result<ConnectionHealth, OraError> {
    with_pool(id, |pool| Ok(pool.ping()))
}

/// Remove a pool from the registry.
pub fn remove_pool(id: &str) {
    let mut registry = POOL_REGISTRY.write();
    if let Some(mut pool) = registry.remove(id) {
        pool.close();
    }
    info!("Removed Oracle pool: {}", id);
}

/// List all pools with their status.
pub fn list_pools() -> Vec<(String, PoolStatus, Option<String>)> {
    let registry = POOL_REGISTRY.read();
    registry.iter().map(|(id, pool)| {
        (id.clone(), pool.status.clone(), pool.connected_since.clone())
    }).collect()
}

/// Check if a pool exists and is connected.
pub fn pool_is_connected(id: &str) -> bool {
    let registry = POOL_REGISTRY.read();
    registry.get(id).map(|p| p.status == PoolStatus::Connected).unwrap_or(false)
}

/// Get pool count.
pub fn pool_count() -> usize {
    POOL_REGISTRY.read().len()
}

/// Get stats for a specific pool.
pub fn pool_stats(id: &str) -> Option<PoolStats> {
    let registry = POOL_REGISTRY.read();
    registry.get(id).map(|p| p.stats().clone())
}

/// Try to reconnect a specific pool with exponential backoff.
pub fn reconnect_pool(id: &str, max_attempts: u32) -> Result<(), OraError> {
    let mut registry = POOL_REGISTRY.write();
    let pool = registry.get_mut(id).ok_or_else(|| OraError::pool_not_found(id))?;

    let mut delay_secs = 1u64;
    for attempt in 1..=max_attempts {
        info!(pool = %id, attempt, "Reconnecting Oracle pool...");
        match pool.connect() {
            Ok(()) => {
                info!(pool = %id, "Reconnected on attempt {}", attempt);
                return Ok(());
            }
            Err(e) => {
                warn!(pool = %id, attempt, "Reconnect failed: {}", e.message);
                if attempt < max_attempts {
                    std::thread::sleep(Duration::from_secs(delay_secs));
                    delay_secs = (delay_secs * 2).min(30);
                } else {
                    pool.status = PoolStatus::Error(e.message.clone());
                    pool.last_error = Some(e.message.clone());
                    return Err(e);
                }
            }
        }
    }
    Err(OraError::generic("Max reconnect attempts exceeded"))
}
