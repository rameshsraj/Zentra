//! Input validation & shell-safety helpers for security-sensitive operations.
//!
//! **RULE**: Every value that flows into a shell command string MUST pass through
//! [`shell_quote`] or be validated by one of the `is_safe_*` functions first.
//! Failing to do so creates a **command injection** vulnerability.

// ═══════════════════════════════════════════════════════════════════════════
// Shell safety
// ═══════════════════════════════════════════════════════════════════════════

/// POSIX-safe shell quoting: wraps `s` in single quotes, escaping any interior
/// single quotes as `'\''`.  Inside single quotes the shell performs **zero**
/// expansion — no `$`, no backticks, no semicolons, no globbing.
///
/// Use this on **every** user-supplied value before interpolating it into a
/// shell command string that will be executed via `ssh <host> <script>`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ═══════════════════════════════════════════════════════════════════════════
// Host / IP validation
// ═══════════════════════════════════════════════════════════════════════════

/// Characters that must NEVER appear in a hostname or IP passed to SSH/SNMP.
/// Any of these would allow shell metacharacter injection even without quotes.
const SHELL_DANGEROUS: &[char] = &[
    ';', '&', '|', '`', '$', '(', ')', '{', '}', '<', '>', '!', '\\', '"', '\'',
    '\n', '\r', '\t', '\0', ' ', '#', '~', '*', '?', '[', ']',
];

/// Check if a host string is safe to use in SSH/SNMP commands.
///
/// Validates:
/// - Non-empty, max 253 chars (DNS limit)
/// - Contains only safe characters (alphanumeric, `.`, `-`, `:` for IPv6)
/// - Not a loopback, link-local, or cloud metadata address
/// - Not a UNIX path or protocol prefix
pub fn is_safe_ssh_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }

    // Reject any shell-dangerous characters
    if host.chars().any(|c| SHELL_DANGEROUS.contains(&c)) {
        return false;
    }

    // Only allow: a-z A-Z 0-9 . - : (colon for IPv6)
    if !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':') {
        return false;
    }

    let host_lower = host.to_lowercase();

    // Reject loopback
    if host_lower == "localhost" || host.starts_with("127.") || host == "::1" || host == "0.0.0.0" {
        return false;
    }

    // Reject link-local (includes AWS/GCP/Azure metadata endpoint 169.254.169.254)
    if host.starts_with("169.254.") {
        return false;
    }

    // Reject cloud metadata endpoints (some providers use non-link-local too)
    if host_lower == "metadata.google.internal" || host_lower == "metadata.google" {
        return false;
    }

    // Reject if it looks like a protocol prefix (http://, ssh://, ftp://, etc.)
    if host_lower.contains("://") {
        return false;
    }

    true
}

/// Strict IPv4 address validation. Returns `true` only for well-formed `N.N.N.N`.
pub fn is_valid_ipv4(ip: &str) -> bool {
    if ip.chars().any(|c| SHELL_DANGEROUS.contains(&c)) {
        return false;
    }
    ip.parse::<std::net::Ipv4Addr>().is_ok()
}

/// Validate a port number is in the valid range (1–65535).
pub fn is_safe_port(port: u16) -> bool {
    port >= 1
}

// ═══════════════════════════════════════════════════════════════════════════
// Username validation
// ═══════════════════════════════════════════════════════════════════════════

/// Validate an SSH/system username.
/// Allows only: a-z, A-Z, 0-9, _, -, .  (max 64 chars, must start with letter or _)
pub fn is_safe_username(username: &str) -> bool {
    if username.is_empty() || username.len() > 64 {
        return false;
    }
    let first = username.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

// ═══════════════════════════════════════════════════════════════════════════
// SNMP community string validation
// ═══════════════════════════════════════════════════════════════════════════

/// Validate an SNMP community string.
/// Allows only: a-z, A-Z, 0-9, _, -, .  (max 64 chars, no shell metacharacters)
pub fn is_safe_snmp_community(community: &str) -> bool {
    if community.is_empty() || community.len() > 64 {
        return false;
    }
    community.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

// ═══════════════════════════════════════════════════════════════════════════
// CIDR validation
// ═══════════════════════════════════════════════════════════════════════════

/// Validate a CIDR notation string (e.g. "10.88.0.0/24").
/// Must be a valid IPv4 + prefix 8–30.
pub fn is_safe_cidr(cidr: &str) -> bool {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return false;
    }
    if !is_valid_ipv4(parts[0]) {
        return false;
    }
    match parts[1].parse::<u8>() {
        Ok(prefix) => (8..=30).contains(&prefix),
        Err(_) => false,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_quote() {
        assert_eq!(shell_quote("hello"), "'hello'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote("$(rm -rf /)"), "'$(rm -rf /)'");
        assert_eq!(shell_quote("`whoami`"), "'`whoami`'");
        assert_eq!(shell_quote("a;b"), "'a;b'");
    }

    #[test]
    fn test_safe_host_rejects_injection() {
        assert!(!is_safe_ssh_host(""));
        assert!(!is_safe_ssh_host("localhost"));
        assert!(!is_safe_ssh_host("127.0.0.1"));
        assert!(!is_safe_ssh_host("169.254.169.254"));
        assert!(!is_safe_ssh_host("::1"));
        assert!(!is_safe_ssh_host("0.0.0.0"));
        assert!(!is_safe_ssh_host("10.0.0.1; rm -rf /"));
        assert!(!is_safe_ssh_host("10.0.0.1`whoami`"));
        assert!(!is_safe_ssh_host("$(cat /etc/passwd)"));
        assert!(!is_safe_ssh_host("host\ninjection"));
        assert!(!is_safe_ssh_host("host name"));
        assert!(!is_safe_ssh_host("metadata.google.internal"));
    }

    #[test]
    fn test_safe_host_allows_valid() {
        assert!(is_safe_ssh_host("10.88.11.101"));
        assert!(is_safe_ssh_host("192.168.1.1"));
        assert!(is_safe_ssh_host("server-01.example.com"));
        assert!(is_safe_ssh_host("2001:db8::1"));
    }

    #[test]
    fn test_safe_username() {
        assert!(is_safe_username("root"));
        assert!(is_safe_username("zentra_admin"));
        assert!(is_safe_username("user-01"));
        assert!(!is_safe_username(""));
        assert!(!is_safe_username("root;rm -rf /"));
        assert!(!is_safe_username("$(whoami)"));
        assert!(!is_safe_username("user name"));
    }

    #[test]
    fn test_safe_snmp_community() {
        assert!(is_safe_snmp_community("public"));
        assert!(is_safe_snmp_community("my_community.v2"));
        assert!(!is_safe_snmp_community(""));
        assert!(!is_safe_snmp_community("pub;rm -rf /"));
        assert!(!is_safe_snmp_community("$(whoami)"));
    }

    #[test]
    fn test_safe_cidr() {
        assert!(is_safe_cidr("10.88.0.0/24"));
        assert!(is_safe_cidr("192.168.1.0/16"));
        assert!(!is_safe_cidr("10.88.0.0/7"));
        assert!(!is_safe_cidr("10.88.0.0/31"));
        assert!(!is_safe_cidr("not-a-cidr"));
        assert!(!is_safe_cidr("10.88.0.0;rm/24"));
    }
}
