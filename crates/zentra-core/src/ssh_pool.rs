//! SSH Connection Multiplexing Pool
//!
//! Uses OpenSSH `ControlMaster` to maintain persistent SSH connections.
//! Instead of a full TCP handshake + key exchange per command, subsequent
//! SSH calls to the same host reuse the existing multiplexed channel —
//! reducing latency from ~200ms to ~5ms per command.
//!
//! Architecture:
//!   1. First connection to a host spawns a background ControlMaster process
//!   2. Subsequent connections attach to the existing master via Unix socket
//!   3. Master stays alive for `ControlPersist` seconds after last use
//!   4. Automatic cleanup of stale sockets on startup

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tracing::{debug, info, warn};

use crate::ssh_utils::SshResult;

/// Configuration for SSH connection multiplexing.
#[derive(Debug, Clone)]
pub struct SshPoolConfig {
    /// Directory for ControlMaster sockets (default: /tmp/zentra-ssh)
    pub control_dir: PathBuf,
    /// How long (seconds) the master stays alive after last use (default: 600)
    pub control_persist_secs: u64,
    /// SSH ConnectTimeout in seconds (default: 10)
    pub connect_timeout_secs: u64,
    /// ServerAliveInterval to detect dead connections (default: 30)
    pub server_alive_interval: u64,
    /// ServerAliveCountMax before disconnecting (default: 3)
    pub server_alive_count_max: u32,
}

impl Default for SshPoolConfig {
    fn default() -> Self {
        Self {
            control_dir: PathBuf::from(
                std::env::var("ZENTRA_SSH_CONTROL_DIR")
                    .unwrap_or_else(|_| "/tmp/zentra-ssh".into())
            ),
            control_persist_secs: 600,
            connect_timeout_secs: 10,
            server_alive_interval: 30,
            server_alive_count_max: 3,
        }
    }
}

/// Tracks a multiplexed SSH master connection.
struct MasterEntry {
    /// user@host key
    target: String,
    /// Path to the ControlMaster socket
    socket_path: PathBuf,
    /// When this master was first established
    created: Instant,
    /// Last time a command was executed over this master
    last_used: Instant,
    /// Number of commands executed over this master
    command_count: u64,
}

/// Global SSH connection pool with ControlMaster multiplexing.
pub struct SshPool {
    config: SshPoolConfig,
    /// Active masters: key = "user@host"
    masters: RwLock<HashMap<String, MasterEntry>>,
}

impl SshPool {
    pub fn new(config: SshPoolConfig) -> Self {
        // Ensure control directory exists
        if let Err(e) = std::fs::create_dir_all(&config.control_dir) {
            warn!(dir = %config.control_dir.display(), error = %e, "Failed to create SSH control dir");
        } else {
            // Restrictive permissions on the socket directory
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    &config.control_dir,
                    std::fs::Permissions::from_mode(0o700),
                );
            }
        }

        info!(
            control_dir = %config.control_dir.display(),
            persist_secs = config.control_persist_secs,
            "SSH connection pool initialized"
        );

        Self {
            config,
            masters: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(SshPoolConfig::default())
    }

    /// Execute a command on a remote host, using a multiplexed connection.
    ///
    /// If no ControlMaster exists for this host, one is established automatically.
    /// Subsequent calls reuse the existing connection.
    #[tracing::instrument(skip(self, script), fields(otel.kind = "client", net.peer.name = %host, net.peer.user = %user))]
    pub async fn exec(&self, host: &str, user: &str, script: &str) -> SshResult {
        let target_key = format!("{}@{}", user, host);
        let socket_path = self.socket_path(&target_key);

        // Check if master is alive
        if !self.is_master_alive(&socket_path).await {
            // Establish a new master
            if let Err(e) = self.establish_master(host, user, &socket_path).await {
                warn!(%target_key, error = %e, "ControlMaster failed, falling back to direct SSH");
                return self.direct_exec(host, user, script).await;
            }

            // Record the master
            let mut masters = self.masters.write();
            masters.insert(target_key.clone(), MasterEntry {
                target: target_key.clone(),
                socket_path: socket_path.clone(),
                created: Instant::now(),
                last_used: Instant::now(),
                command_count: 0,
            });
        }

        // Execute via the multiplexed connection
        let result = self.mux_exec(host, user, script, &socket_path).await;

        // Update stats
        if let Some(mut entry) = self.masters.write().get_mut(&target_key) {
            entry.last_used = Instant::now();
            entry.command_count += 1;
        }

        result
    }

    /// Build the socket path for a given target.
    fn socket_path(&self, target_key: &str) -> PathBuf {
        // Sanitize the key for filesystem use
        let safe_key = target_key.replace(['/', ':', '@'], "_");
        self.config.control_dir.join(format!("ctrl-{}", safe_key))
    }

    /// Check if a ControlMaster socket is alive.
    async fn is_master_alive(&self, socket_path: &PathBuf) -> bool {
        if !socket_path.exists() {
            return false;
        }
        let result = Command::new("ssh")
            .args([
                "-O", "check",
                "-o", &format!("ControlPath={}", socket_path.display()),
                "dummy",   // host is ignored for -O check
            ])
            .output()
            .await;

        match result {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    /// Establish a new ControlMaster connection.
    async fn establish_master(
        &self,
        host: &str,
        user: &str,
        socket_path: &PathBuf,
    ) -> Result<(), String> {
        let known_hosts = std::env::var("ZENTRA_SSH_KNOWN_HOSTS")
            .unwrap_or_else(|_| "/dev/null".to_string());

        debug!(host, user, socket = %socket_path.display(), "Establishing SSH ControlMaster");

        let identity = crate::ssh_utils::resolve_ssh_identity();
        let mut args = vec![
            "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
            "-o".to_string(), format!("UserKnownHostsFile={}", known_hosts),
            "-o".to_string(), format!("ConnectTimeout={}", self.config.connect_timeout_secs),
            "-o".to_string(), "BatchMode=yes".to_string(),
            "-o".to_string(), "LogLevel=ERROR".to_string(),
            "-o".to_string(), format!("ControlPath={}", socket_path.display()),
            "-o".to_string(), "ControlMaster=yes".to_string(),
            "-o".to_string(), format!("ControlPersist={}", self.config.control_persist_secs),
            "-o".to_string(), format!("ServerAliveInterval={}", self.config.server_alive_interval),
            "-o".to_string(), format!("ServerAliveCountMax={}", self.config.server_alive_count_max),
        ];
        if let Some(ref id) = identity {
            args.push("-i".to_string());
            args.push(id.clone());
        }
        args.push("-fN".to_string()); // go to background, no command
        args.push(format!("{}@{}", user, host));

        let output = Command::new("ssh")
            .args(&args)
            .output()
            .await
            .map_err(|e| format!("Failed to spawn SSH master: {}", e))?;

        if output.status.success() {
            info!(host, user, "SSH ControlMaster established");
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("SSH master failed: {}", stderr.trim()))
        }
    }

    /// Execute a command over an existing multiplexed connection.
    async fn mux_exec(
        &self,
        host: &str,
        user: &str,
        script: &str,
        socket_path: &PathBuf,
    ) -> SshResult {
        let identity = crate::ssh_utils::resolve_ssh_identity();
        let mut args = vec![
            "-o".to_string(), format!("ControlPath={}", socket_path.display()),
            "-o".to_string(), "BatchMode=yes".to_string(),
            "-o".to_string(), "LogLevel=ERROR".to_string(),
        ];
        if let Some(ref id) = identity {
            args.push("-i".to_string());
            args.push(id.clone());
        }
        args.push(format!("{}@{}", user, host));
        args.push(script.to_string());

        let output = Command::new("ssh")
            .args(&args)
            .output()
            .await;

        match output {
            Ok(out) => SshResult {
                success: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
            },
            Err(e) => SshResult {
                success: false,
                stdout: String::new(),
                stderr: format!("SSH mux exec failed: {}", e),
            },
        }
    }

    /// Direct SSH exec (fallback when ControlMaster fails).
    async fn direct_exec(&self, host: &str, user: &str, script: &str) -> SshResult {
        crate::ssh_utils::ssh_exec(host, user, script).await
    }

    /// Close a specific master connection.
    pub async fn close_master(&self, host: &str, user: &str) {
        let target_key = format!("{}@{}", user, host);
        let socket_path = self.socket_path(&target_key);

        if socket_path.exists() {
            let _ = Command::new("ssh")
                .args([
                    "-O", "exit",
                    "-o", &format!("ControlPath={}", socket_path.display()),
                    "dummy",
                ])
                .output()
                .await;
        }

        self.masters.write().remove(&target_key);
        debug!(%target_key, "SSH master closed");
    }

    /// Close all master connections (call on shutdown).
    pub async fn close_all(&self) {
        let targets: Vec<String> = self.masters.read().keys().cloned().collect();
        for target_key in &targets {
            let socket_path = self.socket_path(target_key);
            if socket_path.exists() {
                let _ = Command::new("ssh")
                    .args([
                        "-O", "exit",
                        "-o", &format!("ControlPath={}", socket_path.display()),
                        "dummy",
                    ])
                    .output()
                    .await;
            }
        }
        self.masters.write().clear();
        info!(count = targets.len(), "All SSH masters closed");
    }

    /// Get pool statistics.
    pub fn stats(&self) -> SshPoolStats {
        let masters = self.masters.read();
        let mut total_commands = 0u64;
        let mut connections = Vec::new();

        for (key, entry) in masters.iter() {
            total_commands += entry.command_count;
            connections.push(SshConnectionInfo {
                target: key.clone(),
                age_secs: entry.created.elapsed().as_secs(),
                idle_secs: entry.last_used.elapsed().as_secs(),
                command_count: entry.command_count,
            });
        }

        SshPoolStats {
            active_masters: masters.len(),
            total_commands,
            control_dir: self.config.control_dir.display().to_string(),
            persist_secs: self.config.control_persist_secs,
            connections,
        }
    }

    /// Clean up stale sockets that don't have a running master.
    pub async fn cleanup_stale_sockets(&self) {
        let dir = &self.config.control_dir;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("ctrl-"))
                    .unwrap_or(false)
                {
                    if !self.is_master_alive(&path).await {
                        let _ = std::fs::remove_file(&path);
                        debug!(path = %path.display(), "Removed stale SSH socket");
                    }
                }
            }
        }
    }
}

/// Statistics for the SSH connection pool.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SshPoolStats {
    pub active_masters: usize,
    pub total_commands: u64,
    pub control_dir: String,
    pub persist_secs: u64,
    pub connections: Vec<SshConnectionInfo>,
}

/// Info about a single SSH master connection.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SshConnectionInfo {
    pub target: String,
    pub age_secs: u64,
    pub idle_secs: u64,
    pub command_count: u64,
}

// Global singleton for the SSH pool (no external crate needed).
static GLOBAL_SSH_POOL: std::sync::OnceLock<Arc<SshPool>> = std::sync::OnceLock::new();

/// Get the global SSH pool instance.
pub fn global_ssh_pool() -> &'static Arc<SshPool> {
    GLOBAL_SSH_POOL.get_or_init(|| Arc::new(SshPool::with_defaults()))
}

/// Execute a command using the global SSH pool (convenience function).
pub async fn pooled_ssh_exec(host: &str, user: &str, script: &str) -> SshResult {
    global_ssh_pool().exec(host, user, script).await
}
