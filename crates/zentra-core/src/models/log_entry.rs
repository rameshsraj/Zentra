use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Severity level for log entries
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LogSeverity {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Critical,
    Fatal,
}

impl LogSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
            Self::Fatal => "fatal",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" | "notice" => Self::Info,
            "warn" | "warning" => Self::Warning,
            "error" | "err" => Self::Error,
            "critical" | "crit" => Self::Critical,
            "fatal" | "emerg" | "emergency" | "panic" | "alert" => Self::Fatal,
            _ => Self::Info,
        }
    }
}

/// Core log entry model for centralized log observability.
///
/// Carries full drill-down context:
/// org → target → component → severity → message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub log_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub source: String,
    pub component: String,
    pub component_type: String,
    pub severity: LogSeverity,
    pub message: String,
    pub raw_log: Option<String>,
    pub parsed_fields: BTreeMap<String, serde_json::Value>,
    pub tags: BTreeMap<String, String>,
}

impl LogEntry {
    pub fn new(
        org_id: Uuid,
        target_id: Uuid,
        source: impl Into<String>,
        severity: LogSeverity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            log_id: Uuid::now_v7(),
            timestamp: Utc::now(),
            org_id,
            target_id,
            source: source.into(),
            component: String::new(),
            component_type: String::new(),
            severity,
            message: message.into(),
            raw_log: None,
            parsed_fields: BTreeMap::new(),
            tags: BTreeMap::new(),
        }
    }

    pub fn with_component(mut self, component: impl Into<String>, component_type: impl Into<String>) -> Self {
        self.component = component.into();
        self.component_type = component_type.into();
        self
    }

    pub fn with_raw(mut self, raw: impl Into<String>) -> Self {
        self.raw_log = Some(raw.into());
        self
    }

    pub fn with_parsed_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.parsed_fields.insert(key.into(), value);
        self
    }

    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }
}

/// Batch of log entries for efficient ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogBatch {
    pub entries: Vec<LogEntry>,
    pub source: String,
    pub ingested_at: DateTime<Utc>,
}

impl LogBatch {
    pub fn new(source: impl Into<String>, entries: Vec<LogEntry>) -> Self {
        Self {
            source: source.into(),
            ingested_at: Utc::now(),
            entries,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry() {
        let org = Uuid::new_v4();
        let target = Uuid::new_v4();
        let entry = LogEntry::new(org, target, "postgresql", LogSeverity::Error, "connection refused")
            .with_component("pg-main", "database")
            .with_tag("host", "db-01");

        assert_eq!(entry.severity, LogSeverity::Error);
        assert_eq!(entry.source, "postgresql");
        assert_eq!(entry.component, "pg-main");
    }

    #[test]
    fn test_severity_ordering() {
        assert!(LogSeverity::Trace < LogSeverity::Fatal);
        assert!(LogSeverity::Warning < LogSeverity::Error);
    }

    #[test]
    fn test_severity_from_str() {
        assert_eq!(LogSeverity::from_str_loose("WARN"), LogSeverity::Warning);
        assert_eq!(LogSeverity::from_str_loose("err"), LogSeverity::Error);
        assert_eq!(LogSeverity::from_str_loose("emerg"), LogSeverity::Fatal);
    }
}
