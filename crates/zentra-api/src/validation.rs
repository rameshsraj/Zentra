//! Input validation helpers for security-sensitive operations.

/// Check if an SSH target host is safe to connect to.
/// Rejects loopback, link-local, cloud metadata endpoints, and empty strings.
/// Allows 10.x.x.x addresses (Zentra target network) unless ZENTRA_ALLOW_PRIVATE_SSH=1.
pub fn is_safe_ssh_host(host: &str) -> bool {
    // Allow bypass via env var for development
    if std::env::var("ZENTRA_ALLOW_PRIVATE_SSH").as_deref() == Ok("1") {
        return true;
    }

    if host.is_empty() {
        return false;
    }

    let host_lower = host.to_lowercase();

    // Reject localhost
    if host_lower == "localhost" {
        return false;
    }

    // Reject 0.0.0.0
    if host == "0.0.0.0" {
        return false;
    }

    // Reject IPv6 loopback
    if host == "::1" {
        return false;
    }

    // Reject 127.0.0.0/8 (loopback)
    if host.starts_with("127.") {
        return false;
    }

    // Reject 169.254.0.0/16 (link-local, includes AWS metadata 169.254.169.254)
    if host.starts_with("169.254.") {
        return false;
    }

    // 10.x.x.x addresses ARE allowed — Zentra targets are on 10.88.x.x network
    true
}
