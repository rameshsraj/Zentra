use zentra_core::models::log_entry::{LogEntry, LogSeverity};
use crate::LogFormat;
use regex::RegexBuilder;
use uuid::Uuid;
use std::sync::Mutex;
use std::collections::HashMap;

/// Cache compiled regex patterns to avoid recompilation on every log line.
static REGEX_CACHE: std::sync::LazyLock<Mutex<HashMap<String, regex::Regex>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Parse raw log lines into structured LogEntry objects
pub struct LogParser;

impl LogParser {
    /// Parse a raw log line using the given format
    pub fn parse(
        raw: &str,
        format: &LogFormat,
        org_id: Uuid,
        target_id: Uuid,
        source: &str,
    ) -> Option<LogEntry> {
        match format {
            LogFormat::Plain => Self::parse_plain(raw, org_id, target_id, source),
            LogFormat::Json => Self::parse_json(raw, org_id, target_id, source),
            LogFormat::Syslog => Self::parse_syslog(raw, org_id, target_id, source),
            LogFormat::PostgresCsv => Self::parse_postgres_csv(raw, org_id, target_id, source),
            LogFormat::Custom(pattern) => Self::parse_custom(raw, pattern, org_id, target_id, source),
        }
    }

    fn parse_plain(raw: &str, org_id: Uuid, target_id: Uuid, source: &str) -> Option<LogEntry> {
        if raw.trim().is_empty() {
            return None;
        }
        let severity = detect_severity(raw);
        Some(
            LogEntry::new(org_id, target_id, source, severity, raw.trim())
                .with_raw(raw)
        )
    }

    fn parse_json(raw: &str, org_id: Uuid, target_id: Uuid, source: &str) -> Option<LogEntry> {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        let message = v.get("message").or(v.get("msg"))
            .and_then(|m| m.as_str())
            .unwrap_or(raw);
        let severity = v.get("level").or(v.get("severity"))
            .and_then(|l| l.as_str())
            .map(LogSeverity::from_str_loose)
            .unwrap_or(LogSeverity::Info);

        let mut entry = LogEntry::new(org_id, target_id, source, severity, message)
            .with_raw(raw);

        // Extract any additional fields
        if let Some(obj) = v.as_object() {
            for (k, v) in obj {
                if k != "message" && k != "msg" && k != "level" && k != "severity" && k != "timestamp" {
                    entry = entry.with_parsed_field(k, v.clone());
                }
            }
        }

        Some(entry)
    }

    fn parse_syslog(raw: &str, org_id: Uuid, target_id: Uuid, source: &str) -> Option<LogEntry> {
        // Basic syslog format: <priority>timestamp hostname app[pid]: message
        let severity = detect_severity(raw);
        Some(
            LogEntry::new(org_id, target_id, source, severity, raw.trim())
                .with_raw(raw)
        )
    }

    fn parse_postgres_csv(raw: &str, org_id: Uuid, target_id: Uuid, source: &str) -> Option<LogEntry> {
        // PostgreSQL CSV log format
        let parts: Vec<&str> = raw.splitn(14, ',').collect();
        if parts.len() < 12 {
            return Self::parse_plain(raw, org_id, target_id, source);
        }
        let severity = LogSeverity::from_str_loose(parts[11]);
        let message = parts.get(13).unwrap_or(&"");
        Some(
            LogEntry::new(org_id, target_id, source, severity, message.trim())
                .with_raw(raw)
                .with_component("postgresql", "database")
                .with_parsed_field("user", serde_json::json!(parts[1]))
                .with_parsed_field("database", serde_json::json!(parts[2]))
                .with_parsed_field("pid", serde_json::json!(parts[3]))
        )
    }

    fn parse_custom(raw: &str, pattern: &str, org_id: Uuid, target_id: Uuid, source: &str) -> Option<LogEntry> {
        // ReDoS protection: reject excessively long patterns
        if pattern.len() > 1000 {
            tracing::warn!("Custom regex pattern too long ({} chars), rejecting", pattern.len());
            return None;
        }

        // Check cache first
        let re = {
            let cache = REGEX_CACHE.lock().ok()?;
            cache.get(pattern).cloned()
        };

        let re = match re {
            Some(cached) => cached,
            None => {
                let compiled = RegexBuilder::new(pattern)
                    .size_limit(1_000_000)
                    .build()
                    .map_err(|e| {
                        tracing::warn!("Invalid custom regex pattern: {}", e);
                    })
                    .ok()?;
                if let Ok(mut cache) = REGEX_CACHE.lock() {
                    cache.insert(pattern.to_string(), compiled.clone());
                }
                compiled
            }
        };
        let caps = re.captures(raw)?;
        let message = caps.name("message").map(|m| m.as_str()).unwrap_or(raw);
        let severity = caps.name("level")
            .map(|m| LogSeverity::from_str_loose(m.as_str()))
            .unwrap_or(LogSeverity::Info);

        let mut entry = LogEntry::new(org_id, target_id, source, severity, message)
            .with_raw(raw);

        for name in re.capture_names().flatten() {
            if name != "message" && name != "level" {
                if let Some(m) = caps.name(name) {
                    entry = entry.with_parsed_field(name, serde_json::json!(m.as_str()));
                }
            }
        }

        Some(entry)
    }
}

/// Detect severity from raw log text (heuristic)
fn detect_severity(text: &str) -> LogSeverity {
    let upper = text.to_uppercase();
    if upper.contains("FATAL") || upper.contains("PANIC") || upper.contains("EMERG") {
        LogSeverity::Fatal
    } else if upper.contains("CRITICAL") || upper.contains("CRIT") {
        LogSeverity::Critical
    } else if upper.contains("ERROR") || upper.contains("ERR ") {
        LogSeverity::Error
    } else if upper.contains("WARN") {
        LogSeverity::Warning
    } else if upper.contains("DEBUG") {
        LogSeverity::Debug
    } else if upper.contains("TRACE") {
        LogSeverity::Trace
    } else {
        LogSeverity::Info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain() {
        let org = Uuid::new_v4();
        let target = Uuid::new_v4();
        let entry = LogParser::parse(
            "2024-01-01 ERROR: connection refused",
            &LogFormat::Plain,
            org, target, "test",
        ).unwrap();
        assert_eq!(entry.severity, LogSeverity::Error);
    }

    #[test]
    fn test_parse_json() {
        let org = Uuid::new_v4();
        let target = Uuid::new_v4();
        let raw = r#"{"level":"warn","message":"disk space low","host":"srv1"}"#;
        let entry = LogParser::parse(raw, &LogFormat::Json, org, target, "test").unwrap();
        assert_eq!(entry.severity, LogSeverity::Warning);
        assert_eq!(entry.message, "disk space low");
        assert!(entry.parsed_fields.contains_key("host"));
    }

    #[test]
    fn test_severity_detection() {
        assert_eq!(detect_severity("something went WRONG ERROR here"), LogSeverity::Error);
        assert_eq!(detect_severity("WARNING: disk full"), LogSeverity::Warning);
        assert_eq!(detect_severity("all good"), LogSeverity::Info);
    }
}
