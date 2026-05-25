use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Parsed HTTP log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpLogEntry {
    pub timestamp: DateTime<Utc>,
    pub uid: Option<String>,
    pub src_ip: String,
    pub dest_ip: String,
    pub method: String,
    pub host: String,
    pub uri: String,
    pub status_code: Option<u16>,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
    pub mime_type: Option<String>,
    pub request_body_len: i64,
    pub response_body_len: i64,
}

/// HTTP analytics
#[derive(Debug, Clone, Serialize)]
pub struct HttpAnalytics {
    pub total_requests: u64,
    pub status_dist: HashMap<String, u64>,
    pub method_dist: HashMap<String, u64>,
    pub top_hosts: Vec<(String, u64)>,
    pub top_user_agents: Vec<(String, u64)>,
    pub anomalies: Vec<HttpAnomaly>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HttpAnomaly {
    pub src_ip: String,
    pub reason: String,
    pub count: u64,
}

/// HTTP Analyzer
pub struct HttpAnalyzer {
    host_counts: HashMap<String, u64>,
    status_counts: HashMap<String, u64>,
    method_counts: HashMap<String, u64>,
    ua_counts: HashMap<String, u64>,
    ip_error_counts: HashMap<String, u64>,
    total: u64,
}

impl HttpAnalyzer {
    pub fn new() -> Self {
        Self {
            host_counts: HashMap::new(),
            status_counts: HashMap::new(),
            method_counts: HashMap::new(),
            ua_counts: HashMap::new(),
            ip_error_counts: HashMap::new(),
            total: 0,
        }
    }

    pub fn process(&mut self, entry: &HttpLogEntry) {
        self.total += 1;
        *self.host_counts.entry(entry.host.clone()).or_insert(0) += 1;
        *self.method_counts.entry(entry.method.clone()).or_insert(0) += 1;

        if let Some(code) = entry.status_code {
            let bucket = format!("{}xx", code / 100);
            *self.status_counts.entry(bucket).or_insert(0) += 1;

            // Track error-causing IPs
            if code >= 400 {
                *self.ip_error_counts.entry(entry.src_ip.clone()).or_insert(0) += 1;
            }
        }

        if let Some(ua) = &entry.user_agent {
            let short_ua = ua.chars().take(80).collect::<String>();
            *self.ua_counts.entry(short_ua).or_insert(0) += 1;
        }
    }

    /// Detect anomalies: high error rates from single IPs, suspicious URIs
    pub fn detect_anomalies(&self) -> Vec<HttpAnomaly> {
        let mut anomalies = Vec::new();

        for (ip, &count) in &self.ip_error_counts {
            if count > 100 {
                anomalies.push(HttpAnomaly {
                    src_ip: ip.clone(),
                    reason: format!("High HTTP error count: {} errors", count),
                    count,
                });
            }
        }

        anomalies
    }

    pub fn analytics(&self) -> HttpAnalytics {
        let mut top_hosts: Vec<_> = self.host_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top_hosts.sort_by(|a, b| b.1.cmp(&a.1));
        top_hosts.truncate(20);

        let mut top_ua: Vec<_> = self.ua_counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        top_ua.sort_by(|a, b| b.1.cmp(&a.1));
        top_ua.truncate(10);

        HttpAnalytics {
            total_requests: self.total,
            status_dist: self.status_counts.clone(),
            method_dist: self.method_counts.clone(),
            top_hosts,
            top_user_agents: top_ua,
            anomalies: self.detect_anomalies(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

impl Default for HttpAnalyzer {
    fn default() -> Self { Self::new() }
}
