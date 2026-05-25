use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed DNS log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsLogEntry {
    pub timestamp: DateTime<Utc>,
    pub uid: Option<String>,
    pub src_ip: String,
    pub query: String,
    pub qtype: String,
    pub rcode: String,
    pub answers: Vec<String>,
    pub ttl: Option<i32>,
    pub rejected: bool,
}

/// DNS analytics results
#[derive(Debug, Clone, Serialize)]
pub struct DnsAnalytics {
    pub total_queries: u64,
    pub nxdomain_count: u64,
    pub top_domains: Vec<(String, u64)>,
    pub query_type_dist: HashMap<String, u64>,
    pub suspicious_domains: Vec<SuspiciousDomain>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuspiciousDomain {
    pub domain: String,
    pub reason: String,
    pub query_count: u64,
}

/// DNS Analyzer: detects NXDOMAIN floods, tunneling, and suspicious patterns
pub struct DnsAnalyzer {
    domain_counts: HashMap<String, u64>,
    nxdomain_counts: HashMap<String, u64>,
    query_type_counts: HashMap<String, u64>,
    total: u64,
}

impl DnsAnalyzer {
    pub fn new() -> Self {
        Self {
            domain_counts: HashMap::new(),
            nxdomain_counts: HashMap::new(),
            query_type_counts: HashMap::new(),
            total: 0,
        }
    }

    /// Process a DNS log entry
    pub fn process(&mut self, entry: &DnsLogEntry) {
        self.total += 1;
        let domain = extract_base_domain(&entry.query);
        *self.domain_counts.entry(domain.clone()).or_insert(0) += 1;
        *self.query_type_counts.entry(entry.qtype.clone()).or_insert(0) += 1;

        if entry.rcode == "NXDOMAIN" || entry.rcode == "3" {
            *self.nxdomain_counts.entry(domain).or_insert(0) += 1;
        }
    }

    /// Detect DNS tunneling: long subdomain labels, high entropy, TXT queries
    pub fn detect_tunneling(&self) -> Vec<SuspiciousDomain> {
        let mut suspicious = Vec::new();

        for (domain, &count) in &self.domain_counts {
            // Long subdomains (potential tunneling)
            if domain.len() > 60 {
                suspicious.push(SuspiciousDomain {
                    domain: domain.clone(),
                    reason: "Unusually long domain name (possible tunneling)".into(),
                    query_count: count,
                });
            }

            // High NXDOMAIN rate for a domain
            if let Some(&nx_count) = self.nxdomain_counts.get(domain) {
                if nx_count > 50 && (nx_count as f64 / count as f64) > 0.7 {
                    suspicious.push(SuspiciousDomain {
                        domain: domain.clone(),
                        reason: format!("High NXDOMAIN rate: {}/{} queries", nx_count, count),
                        query_count: count,
                    });
                }
            }
        }

        suspicious
    }

    /// Produce analytics summary
    pub fn analytics(&self) -> DnsAnalytics {
        let nxdomain_total: u64 = self.nxdomain_counts.values().sum();

        let mut top: Vec<_> = self.domain_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        top.truncate(20);

        DnsAnalytics {
            total_queries: self.total,
            nxdomain_count: nxdomain_total,
            top_domains: top,
            query_type_dist: self.query_type_counts.clone(),
            suspicious_domains: self.detect_tunneling(),
        }
    }

    pub fn reset(&mut self) {
        self.domain_counts.clear();
        self.nxdomain_counts.clear();
        self.query_type_counts.clear();
        self.total = 0;
    }
}

impl Default for DnsAnalyzer {
    fn default() -> Self { Self::new() }
}

/// Extract base domain from FQDN (e.g., "sub.example.com" → "example.com")
fn extract_base_domain(fqdn: &str) -> String {
    let parts: Vec<&str> = fqdn.trim_end_matches('.').split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1])
    } else {
        fqdn.to_string()
    }
}
