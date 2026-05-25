use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Severity of a detected threat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl ThreatSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Category of the threat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatCategory {
    PortScan,
    BruteForce,
    DDoS,
    DataExfiltration,
    MalwareCallback,
    UnauthorizedAccess,
    AnomalousTraffic,
    PolicyViolation,
    SuspiciousConnection,
}

impl ThreatCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PortScan => "port_scan",
            Self::BruteForce => "brute_force",
            Self::DDoS => "ddos",
            Self::DataExfiltration => "data_exfiltration",
            Self::MalwareCallback => "malware_callback",
            Self::UnauthorizedAccess => "unauthorized_access",
            Self::AnomalousTraffic => "anomalous_traffic",
            Self::PolicyViolation => "policy_violation",
            Self::SuspiciousConnection => "suspicious_connection",
        }
    }
}

/// Status of a threat event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreatStatus {
    Active,
    Investigating,
    Mitigated,
    Resolved,
    FalsePositive,
}

/// A detected threat event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatEvent {
    pub threat_id: Uuid,
    pub org_id: Uuid,
    pub target_id: Option<Uuid>,
    pub category: ThreatCategory,
    pub severity: ThreatSeverity,
    pub status: ThreatStatus,
    pub source_ip: Option<String>,
    pub dest_ip: Option<String>,
    pub port: Option<u16>,
    pub description: String,
    pub evidence: Vec<String>,
    pub mitre_tactic: Option<String>,
    pub mitre_technique: Option<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub event_count: u64,
    pub metadata: Option<serde_json::Value>,
}

impl ThreatEvent {
    pub fn new(
        org_id: Uuid,
        category: ThreatCategory,
        severity: ThreatSeverity,
        description: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            threat_id: Uuid::now_v7(),
            org_id,
            target_id: None,
            category,
            severity,
            status: ThreatStatus::Active,
            source_ip: None,
            dest_ip: None,
            port: None,
            description: description.into(),
            evidence: Vec::new(),
            mitre_tactic: None,
            mitre_technique: None,
            first_seen: now,
            last_seen: now,
            event_count: 1,
            metadata: None,
        }
    }

    pub fn with_target(mut self, target_id: Uuid) -> Self {
        self.target_id = Some(target_id);
        self
    }

    pub fn with_source(mut self, ip: impl Into<String>) -> Self {
        self.source_ip = Some(ip.into());
        self
    }

    pub fn with_dest(mut self, ip: impl Into<String>) -> Self {
        self.dest_ip = Some(ip.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    pub fn with_mitre(mut self, tactic: impl Into<String>, technique: impl Into<String>) -> Self {
        self.mitre_tactic = Some(tactic.into());
        self.mitre_technique = Some(technique.into());
        self
    }

    pub fn increment(&mut self) {
        self.event_count += 1;
        self.last_seen = Utc::now();
    }

    pub fn resolve(&mut self) {
        self.status = ThreatStatus::Resolved;
    }

    pub fn mark_false_positive(&mut self) {
        self.status = ThreatStatus::FalsePositive;
    }
}

/// A threat detection rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatRule {
    pub rule_id: Uuid,
    pub name: String,
    pub description: String,
    pub category: ThreatCategory,
    pub severity: ThreatSeverity,
    pub enabled: bool,
    pub conditions: ThreatConditions,
}

/// Conditions that trigger a threat rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatConditions {
    /// Time window in seconds for detection
    pub window_seconds: u64,
    /// Threshold count for triggering
    pub threshold: u64,
    /// Optional source IP filter (CIDR)
    pub source_filter: Option<String>,
    /// Optional destination port filter
    pub dest_ports: Option<Vec<u16>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threat_event_creation() {
        let org_id = Uuid::new_v4();
        let threat = ThreatEvent::new(
            org_id,
            ThreatCategory::PortScan,
            ThreatSeverity::High,
            "Port scan detected from 10.0.0.1",
        )
        .with_source("10.0.0.1")
        .with_dest("192.168.1.1")
        .with_evidence("50 unique ports scanned in 10 seconds")
        .with_mitre("Reconnaissance", "T1046");

        assert_eq!(threat.category, ThreatCategory::PortScan);
        assert_eq!(threat.severity, ThreatSeverity::High);
        assert_eq!(threat.status, ThreatStatus::Active);
        assert_eq!(threat.source_ip.as_deref(), Some("10.0.0.1"));
        assert_eq!(threat.evidence.len(), 1);
        assert_eq!(threat.mitre_technique.as_deref(), Some("T1046"));
    }

    #[test]
    fn test_threat_lifecycle() {
        let mut threat = ThreatEvent::new(
            Uuid::new_v4(),
            ThreatCategory::BruteForce,
            ThreatSeverity::Medium,
            "SSH brute force",
        );
        assert_eq!(threat.event_count, 1);

        threat.increment();
        threat.increment();
        assert_eq!(threat.event_count, 3);

        threat.resolve();
        assert_eq!(threat.status, ThreatStatus::Resolved);
    }
}
