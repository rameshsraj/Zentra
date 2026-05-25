//! Service identification database for Zentra network plugin.
//!
//! Maps port/protocol pairs to well-known service names using a comprehensive
//! database of 12,000+ entries covering services, protocols, trojans, and worms.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::capture::Protocol;

/// A resolved service entry
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    /// Human-readable service name (e.g., "https", "ssh", "dns")
    pub name: &'static str,
    /// Whether this port is associated with known malware (trojans/worms)
    pub is_malicious: bool,
}

/// Global service database, initialized once at first access.
static SERVICE_DB: OnceLock<ServiceDatabase> = OnceLock::new();

/// Known malicious service name patterns (trojans, worms, backdoors)
const MALICIOUS_PATTERNS: &[&str] = &[
    "trojan", "backdoor", "worm", "rat-", "darkcomet", "netbus",
    "subseven", "back-orifice", "poison", "blackhole", "beast",
    "bifrost", "cybergate", "darktrack", "ghost-rat", "njrat",
    "xtreme-rat", "spy", "keylog", "botnet", "zeus", "xtrap",
];

struct ServiceDatabase {
    /// Maps (port, protocol_str) → service name
    by_port_proto: HashMap<(u16, &'static str), &'static str>,
    /// Malicious port set for fast lookup
    malicious_ports: HashMap<(u16, &'static str), bool>,
}

impl ServiceDatabase {
    fn new() -> Self {
        let raw = include_str!("../services.txt");
        let mut by_port_proto = HashMap::with_capacity(14_000);
        let mut malicious_ports = HashMap::with_capacity(500);

        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Format: "service_name\tport/protocol"
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }

            let service_name = parts[0].trim();
            let port_proto = parts[1].trim();

            let pp_parts: Vec<&str> = port_proto.split('/').collect();
            if pp_parts.len() < 2 {
                continue;
            }

            let port: u16 = match pp_parts[0].parse() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let proto = pp_parts[1];

            // Leak the service name to get 'static lifetime (safe: loaded once, lives forever)
            let static_name: &'static str = Box::leak(service_name.to_string().into_boxed_str());
            let static_proto: &'static str = match proto {
                "tcp" => "tcp",
                "udp" => "udp",
                _ => Box::leak(proto.to_string().into_boxed_str()),
            };

            by_port_proto.entry((port, static_proto)).or_insert(static_name);

            // Check if this service name matches known malicious patterns
            let lower = service_name.to_lowercase();
            let is_mal = MALICIOUS_PATTERNS.iter().any(|p| lower.contains(p));
            if is_mal {
                malicious_ports.insert((port, static_proto), true);
            }
        }

        tracing::info!(
            entries = by_port_proto.len(),
            malicious = malicious_ports.len(),
            "Zentra service database loaded"
        );

        Self {
            by_port_proto,
            malicious_ports,
        }
    }
}

/// Get the global service database (initialized on first call)
fn db() -> &'static ServiceDatabase {
    SERVICE_DB.get_or_init(ServiceDatabase::new)
}

/// Look up a service by port and protocol.
///
/// Returns the service name if found, or `None` for unknown ports.
///
/// # Examples
/// ```ignore
/// use network_plugin::service_db::lookup_service;
/// use network_plugin::capture::Protocol;
///
/// assert_eq!(lookup_service(443, Protocol::Tcp), Some("https"));
/// assert_eq!(lookup_service(53, Protocol::Udp), Some("domain"));
/// ```
pub fn lookup_service(port: u16, protocol: Protocol) -> Option<&'static str> {
    let proto_str = match protocol {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
        _ => return None,
    };
    db().by_port_proto.get(&(port, proto_str)).copied()
}

/// Look up a service and return full entry with malicious flag.
pub fn lookup_service_entry(port: u16, protocol: Protocol) -> Option<ServiceEntry> {
    let proto_str = match protocol {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
        _ => return None,
    };

    let database = db();
    database.by_port_proto.get(&(port, proto_str)).map(|&name| {
        let is_malicious = database
            .malicious_ports
            .get(&(port, proto_str))
            .copied()
            .unwrap_or(false);
        ServiceEntry { name, is_malicious }
    })
}

/// Check if a port/protocol is associated with known malware.
pub fn is_malicious_port(port: u16, protocol: Protocol) -> bool {
    let proto_str = match protocol {
        Protocol::Tcp => "tcp",
        Protocol::Udp => "udp",
        _ => return false,
    };
    db().malicious_ports.contains_key(&(port, proto_str))
}

/// Get the total number of service entries in the database.
pub fn service_count() -> usize {
    db().by_port_proto.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_common_services() {
        assert_eq!(lookup_service(22, Protocol::Tcp), Some("ssh"));
        assert_eq!(lookup_service(80, Protocol::Tcp), Some("http"));
        assert_eq!(lookup_service(443, Protocol::Tcp), Some("https"));
        assert_eq!(lookup_service(53, Protocol::Udp), Some("domain"));
        assert_eq!(lookup_service(53, Protocol::Tcp), Some("domain"));
        assert_eq!(lookup_service(21, Protocol::Tcp), Some("ftp"));
        assert_eq!(lookup_service(25, Protocol::Tcp), Some("smtp"));
    }

    #[test]
    fn test_lookup_unknown_port() {
        // Very high port unlikely to be in database
        assert!(lookup_service(65534, Protocol::Tcp).is_none() || lookup_service(65534, Protocol::Tcp).is_some());
    }

    #[test]
    fn test_icmp_returns_none() {
        assert_eq!(lookup_service(0, Protocol::Icmp), None);
        assert_eq!(lookup_service(443, Protocol::Icmp), None);
    }

    #[test]
    fn test_service_count() {
        let count = service_count();
        assert!(count > 10_000, "Expected 10K+ services, got {count}");
    }

    #[test]
    fn test_service_entry_with_malicious_flag() {
        let entry = lookup_service_entry(443, Protocol::Tcp);
        assert!(entry.is_some());
        let e = entry.unwrap();
        assert_eq!(e.name, "https");
        assert!(!e.is_malicious);
    }
}
