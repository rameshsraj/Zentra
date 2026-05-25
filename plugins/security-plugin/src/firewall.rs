use serde::{Deserialize, Serialize};

/// Firewall log parser — normalizes iptables, firewalld, pf formats to common schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallEvent {
    pub timestamp: String,
    pub src_ip: String,
    pub dest_ip: String,
    pub src_port: Option<u16>,
    pub dest_port: Option<u16>,
    pub proto: String,
    pub action: FirewallAction,
    pub rule_name: Option<String>,
    pub interface: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FirewallAction {
    Accept,
    Drop,
    Reject,
}

impl std::fmt::Display for FirewallAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accept => write!(f, "accept"),
            Self::Drop => write!(f, "drop"),
            Self::Reject => write!(f, "reject"),
        }
    }
}

/// Parse iptables log line
pub fn parse_iptables(line: &str) -> Option<FirewallEvent> {
    // Example: "Jan  1 00:00:00 host kernel: [12345.678] iptables DROP: IN=eth0 ... SRC=1.2.3.4 DST=5.6.7.8 ... PROTO=TCP SPT=12345 DPT=80 ..."
    let action = if line.contains("DROP") { FirewallAction::Drop }
        else if line.contains("REJECT") { FirewallAction::Reject }
        else { FirewallAction::Accept };

    let extract = |prefix: &str| -> Option<String> {
        line.find(prefix).map(|i| {
            let start = i + prefix.len();
            let end = line[start..].find(' ').map(|e| start + e).unwrap_or(line.len());
            line[start..end].to_string()
        })
    };

    let src_ip = extract("SRC=")?;
    let dest_ip = extract("DST=")?;
    let proto = extract("PROTO=").unwrap_or_else(|| "unknown".into());
    let src_port = extract("SPT=").and_then(|s| s.parse().ok());
    let dest_port = extract("DPT=").and_then(|s| s.parse().ok());
    let interface = extract("IN=");

    Some(FirewallEvent {
        timestamp: String::new(), // caller fills in
        src_ip, dest_ip, src_port, dest_port,
        proto, action,
        rule_name: None,
        interface,
        bytes: extract("LEN=").and_then(|s| s.parse().ok()),
    })
}

/// Parse firewalld log line (similar to iptables with zone info)
pub fn parse_firewalld(line: &str) -> Option<FirewallEvent> {
    // Reuse iptables parser since firewalld uses similar kernel logging
    parse_iptables(line)
}

/// Parse pf (OpenBSD) log line
pub fn parse_pf(line: &str) -> Option<FirewallEvent> {
    // Example: "... rule 5/(match) pass in on em0: 1.2.3.4.12345 > 5.6.7.8.80: ..."
    let action = if line.contains(" block ") { FirewallAction::Drop }
        else { FirewallAction::Accept };

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 8 { return None; }

    // Simplified — real parser would handle full pflog format
    Some(FirewallEvent {
        timestamp: String::new(),
        src_ip: parts.get(6).unwrap_or(&"0.0.0.0").to_string(),
        dest_ip: parts.get(8).unwrap_or(&"0.0.0.0").replace(':', ""),
        src_port: None, dest_port: None,
        proto: "tcp".into(),
        action,
        rule_name: None,
        interface: parts.get(5).map(|s| s.replace(':', "")),
        bytes: None,
    })
}
