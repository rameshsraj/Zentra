//! Program/process identification for Zentra network plugin.
//!
//! Maps local ports to the process that owns them, enabling per-program
//! traffic breakdown. Uses the `sysinfo` crate for cross-platform process lookup.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::Serialize;
use tracing::{debug, warn};

use crate::capture::Protocol;

/// Information about a program generating network traffic
#[derive(Debug, Clone, Serialize)]
pub struct ProgramInfo {
    pub name: String,
    pub path: String,
    pub pid: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connections: u32,
}

impl Default for ProgramInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: String::new(),
            pid: 0,
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            connections: 0,
        }
    }
}

/// Cache entry for a port→process mapping
#[derive(Debug, Clone)]
struct CacheEntry {
    name: String,
    path: String,
    pid: u32,
    resolved_at: std::time::Instant,
}

/// Program lookup engine that maps local ports to owning processes.
///
/// Uses a cache with TTL to avoid repeated expensive lookups.
pub struct ProgramLookup {
    /// Cache: (port, protocol) → process info
    cache: RwLock<HashMap<(u16, Protocol), CacheEntry>>,
    /// Aggregated per-program traffic stats
    program_stats: RwLock<HashMap<String, ProgramInfo>>,
    /// Cache TTL in seconds
    cache_ttl_secs: u64,
    /// System info handle for process enumeration
    sys: RwLock<sysinfo::System>,
}

impl ProgramLookup {
    /// Create a new program lookup engine.
    pub fn new() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        Self {
            cache: RwLock::new(HashMap::with_capacity(256)),
            program_stats: RwLock::new(HashMap::with_capacity(64)),
            cache_ttl_secs: 60,
            sys: RwLock::new(sys),
        }
    }

    /// Look up the process owning a local port.
    /// Returns (name, path, pid) if found.
    pub fn lookup(&self, port: u16, _proto: Protocol) -> Option<(String, String, u32)> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(entry) = cache.get(&(port, _proto)) {
                if entry.resolved_at.elapsed().as_secs() < self.cache_ttl_secs {
                    return Some((entry.name.clone(), entry.path.clone(), entry.pid));
                }
            }
        }

        // Refresh processes and search
        let result = self.resolve_port(port);

        // Cache the result
        if let Some(ref r) = result {
            let mut cache = self.cache.write();
            cache.insert(
                (port, _proto),
                CacheEntry {
                    name: r.0.clone(),
                    path: r.1.clone(),
                    pid: r.2,
                    resolved_at: std::time::Instant::now(),
                },
            );
        }

        result
    }

    /// Record traffic data for a program.
    pub fn record_traffic(
        &self,
        program_name: &str,
        program_path: &str,
        pid: u32,
        bytes: u64,
        packets: u64,
        is_outgoing: bool,
    ) {
        let mut stats = self.program_stats.write();
        let entry = stats.entry(program_name.to_string()).or_insert_with(|| ProgramInfo {
            name: program_name.to_string(),
            path: program_path.to_string(),
            pid,
            ..Default::default()
        });

        if is_outgoing {
            entry.bytes_sent += bytes;
            entry.packets_sent += packets;
        } else {
            entry.bytes_received += bytes;
            entry.packets_received += packets;
        }
    }

    /// Increment connection count for a program.
    pub fn record_connection(&self, program_name: &str) {
        let mut stats = self.program_stats.write();
        if let Some(entry) = stats.get_mut(program_name) {
            entry.connections += 1;
        }
    }

    /// Get sorted list of programs by total bytes.
    pub fn get_program_stats(&self) -> Vec<ProgramInfo> {
        let stats = self.program_stats.read();
        let mut programs: Vec<ProgramInfo> = stats.values().cloned().collect();
        programs.sort_by(|a, b| {
            (b.bytes_sent + b.bytes_received).cmp(&(a.bytes_sent + a.bytes_received))
        });
        programs
    }

    /// Reset all program stats.
    pub fn reset(&self) {
        self.cache.write().clear();
        self.program_stats.write().clear();
    }

    /// Internal: resolve which process owns a port by scanning /proc or using sysinfo.
    fn resolve_port(&self, port: u16) -> Option<(String, String, u32)> {
        // Refresh process list periodically
        {
            let mut sys = self.sys.write();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        }

        // On Linux, we can parse /proc/net/tcp and /proc/net/udp to find the inode,
        // then match it to a process. This is a simplified version using sysinfo.
        #[cfg(target_os = "linux")]
        {
            if let Some(result) = self.resolve_port_linux(port) {
                return Some(result);
            }
        }

        // Fallback: try to match by checking well-known process/port associations
        self.resolve_port_heuristic(port)
    }

    #[cfg(target_os = "linux")]
    fn resolve_port_linux(&self, port: u16) -> Option<(String, String, u32)> {
        use std::fs;

        // Parse /proc/net/tcp and /proc/net/udp for the port
        for proto_file in &["/proc/net/tcp", "/proc/net/tcp6", "/proc/net/udp", "/proc/net/udp6"] {
            if let Ok(content) = fs::read_to_string(proto_file) {
                for line in content.lines().skip(1) {
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    if fields.len() < 10 {
                        continue;
                    }

                    // local_address is fields[1], format: hex_ip:hex_port
                    let local_addr = fields[1];
                    if let Some(port_hex) = local_addr.split(':').nth(1) {
                        if let Ok(local_port) = u16::from_str_radix(port_hex, 16) {
                            if local_port == port {
                                // inode is fields[9]
                                if let Ok(inode) = fields[9].parse::<u64>() {
                                    if inode == 0 {
                                        continue;
                                    }
                                    // Find process owning this inode
                                    if let Some(result) = self.find_process_by_inode(inode) {
                                        return Some(result);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    #[cfg(target_os = "linux")]
    fn find_process_by_inode(&self, inode: u64) -> Option<(String, String, u32)> {
        use std::fs;

        let target = format!("socket:[{inode}]");

        // Scan /proc/*/fd/ for the matching socket inode
        if let Ok(proc_entries) = fs::read_dir("/proc") {
            for entry in proc_entries.flatten() {
                let pid_str = entry.file_name();
                let pid_str = pid_str.to_string_lossy();

                // Only look at numeric directories (PIDs)
                if !pid_str.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                let fd_dir = format!("/proc/{pid_str}/fd");
                if let Ok(fds) = fs::read_dir(&fd_dir) {
                    for fd in fds.flatten() {
                        if let Ok(link) = fs::read_link(fd.path()) {
                            if link.to_string_lossy() == target {
                                // Found the process. Get its name and path.
                                let pid: u32 = pid_str.parse().unwrap_or(0);
                                let sys = self.sys.read();
                                let sysinfo_pid = sysinfo::Pid::from_u32(pid);
                                if let Some(process) = sys.process(sysinfo_pid) {
                                    let name = process.name().to_string_lossy().to_string();
                                    let path = process
                                        .exe()
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    return Some((name, path, pid));
                                }
                                // Fallback: read /proc/PID/comm
                                let comm_path = format!("/proc/{pid_str}/comm");
                                if let Ok(name) = fs::read_to_string(&comm_path) {
                                    let exe_path = format!("/proc/{pid_str}/exe");
                                    let path = fs::read_link(&exe_path)
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    return Some((name.trim().to_string(), path, pid));
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Heuristic fallback: check common port→program associations
    fn resolve_port_heuristic(&self, port: u16) -> Option<(String, String, u32)> {
        let sys = self.sys.read();

        // Iterate all processes and check if any are listening on well-known ports
        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_lowercase();

            let matches = match port {
                80 | 8080 | 8443 => {
                    name.contains("nginx") || name.contains("apache") || name.contains("httpd")
                        || name.contains("node") || name.contains("java")
                }
                443 => {
                    name.contains("nginx") || name.contains("apache") || name.contains("httpd")
                        || name.contains("node")
                }
                5432 => name.contains("postgres"),
                3306 => name.contains("mysql") || name.contains("mariadbd"),
                6379 => name.contains("redis"),
                27017 => name.contains("mongod"),
                22 => name.contains("sshd"),
                53 => name.contains("named") || name.contains("dnsmasq") || name.contains("systemd-resolve"),
                25 | 587 => name.contains("postfix") || name.contains("sendmail") || name.contains("exim"),
                _ => false,
            };

            if matches {
                let exe_path = process
                    .exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                return Some((
                    process.name().to_string_lossy().to_string(),
                    exe_path,
                    pid.as_u32(),
                ));
            }
        }

        None
    }
}

/// Create a shared program lookup instance.
pub fn new_shared() -> Arc<ProgramLookup> {
    Arc::new(ProgramLookup::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_program_lookup_creation() {
        let lookup = ProgramLookup::new();
        // Should not panic
        let stats = lookup.get_program_stats();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_record_traffic() {
        let lookup = ProgramLookup::new();
        lookup.record_traffic("nginx", "/usr/sbin/nginx", 1234, 5000, 10, true);
        lookup.record_traffic("nginx", "/usr/sbin/nginx", 1234, 3000, 8, false);
        lookup.record_traffic("postgres", "/usr/bin/postgres", 5678, 2000, 5, true);

        let stats = lookup.get_program_stats();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].name, "nginx"); // Most bytes first
        assert_eq!(stats[0].bytes_sent, 5000);
        assert_eq!(stats[0].bytes_received, 3000);
    }

    #[test]
    fn test_reset() {
        let lookup = ProgramLookup::new();
        lookup.record_traffic("test", "/test", 1, 100, 1, true);
        assert_eq!(lookup.get_program_stats().len(), 1);
        lookup.reset();
        assert!(lookup.get_program_stats().is_empty());
    }
}
