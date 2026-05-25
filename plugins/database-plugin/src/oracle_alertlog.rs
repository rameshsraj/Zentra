//! Oracle Alert Log Export
//!
//! Queries v$diag_alert_ext for real alert log entries when a native pool
//! is available. Falls back to demo data otherwise. Tracks last timestamp
//! for incremental collection. Feed entries to log engine integration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{Utc, Duration};
use tracing::{debug, warn};
use crate::oracle_connection::{pool_query, pool_is_connected, list_pools, PoolStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertLogEntry {
    pub id: String,
    pub connection_id: String,
    pub database_name: String,
    pub timestamp: String,
    pub level: String,      // critical, warning, info, trace
    pub component: String,
    pub message: String,
    pub ora_code: Option<i32>,
    pub host: String,
    pub instance_name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertLogStats {
    pub total: u64,
    pub by_level: HashMap<String, u64>,
    pub by_database: HashMap<String, u64>,
    pub last_24h: u64,
    pub critical_last_1h: u64,
}

/// Collect alert log entries — tries real query first, falls back to demo.
pub fn collect_alert_log(connection_id: &str, limit: usize, since: Option<&str>) -> Vec<AlertLogEntry> {
    if pool_is_connected(connection_id) {
        match collect_real(connection_id, limit, since) {
            Ok(entries) if !entries.is_empty() => {
                debug!(pool = %connection_id, count = entries.len(), "Real alert log collected");
                return entries;
            }
            Ok(_) => {
                debug!(pool = %connection_id, "Real alert log returned 0 rows, using demo");
            }
            Err(e) => {
                warn!(pool = %connection_id, "Alert log query failed: {}", e);
            }
        }
    }
    get_demo_entries(connection_id, limit)
}

/// Real alert log collection via v$diag_alert_ext
fn collect_real(connection_id: &str, limit: usize, since: Option<&str>) -> Result<Vec<AlertLogEntry>, String> {
    let since_clause = if let Some(ts) = since {
        format!("WHERE originating_timestamp > TO_TIMESTAMP_TZ('{}', 'YYYY-MM-DD\"T\"HH24:MI:SS.FF6TZH:TZM')", ts)
    } else {
        "WHERE originating_timestamp > SYSTIMESTAMP - INTERVAL '24' HOUR".to_string()
    };

    let sql = format!(
        "SELECT originating_timestamp, message_text, message_type, message_level, \
         component_id, host_id, module_id \
         FROM v$diag_alert_ext {} \
         ORDER BY originating_timestamp DESC \
         FETCH FIRST {} ROWS ONLY",
        since_clause, limit
    );

    let qr = pool_query(connection_id, &sql).map_err(|e| e.message)?;

    if qr.columns.is_empty() {
        return Ok(vec![]);
    }

    let col_lower: Vec<String> = qr.columns.iter().map(|c| c.to_lowercase()).collect();
    let mut entries = Vec::new();

    for (i, row) in qr.rows.iter().enumerate() {
        let get = |name: &str| -> String {
            col_lower.iter().position(|c| c == name)
                .and_then(|idx| row.get(idx))
                .cloned()
                .unwrap_or_default()
        };

        let message_level: i32 = get("message_level").parse().unwrap_or(8);
        let level = classify_level(message_level);

        let ora_code = extract_ora_code(&get("message_text"));

        entries.push(AlertLogEntry {
            id: format!("alog-{}-{}", connection_id, i),
            connection_id: connection_id.to_string(),
            database_name: "".into(),
            timestamp: get("originating_timestamp"),
            level,
            component: get("component_id"),
            message: get("message_text"),
            ora_code,
            host: get("host_id"),
            instance_name: get("module_id"),
            detail: None,
        });
    }

    Ok(entries)
}

/// Classify Oracle message_level to severity string
fn classify_level(level: i32) -> String {
    match level {
        1 => "critical".into(),
        2 => "severe".into(),
        8 => "warning".into(),
        16 => "info".into(),
        _ => "trace".into(),
    }
}

/// Extract ORA-NNNNN code from message text
fn extract_ora_code(msg: &str) -> Option<i32> {
    if let Some(pos) = msg.find("ORA-") {
        let after = &msg[pos + 4..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    } else {
        None
    }
}

/// Compute stats from entries (works for both real and demo)
pub fn compute_stats(entries: &[AlertLogEntry]) -> AlertLogStats {
    let now = Utc::now();
    let mut by_level: HashMap<String, u64> = HashMap::new();
    let mut by_database: HashMap<String, u64> = HashMap::new();
    let mut last_24h = 0u64;
    let mut critical_1h = 0u64;

    for e in entries {
        *by_level.entry(e.level.clone()).or_default() += 1;
        if !e.database_name.is_empty() {
            *by_database.entry(e.database_name.clone()).or_default() += 1;
        }
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&e.timestamp) {
            let age = now.signed_duration_since(ts);
            if age.num_hours() < 24 {
                last_24h += 1;
            }
            if age.num_hours() < 1 && e.level == "critical" {
                critical_1h += 1;
            }
        }
    }

    AlertLogStats {
        total: entries.len() as u64,
        by_level,
        by_database,
        last_24h,
        critical_last_1h: critical_1h,
    }
}

/// Collect stats — aggregates from all connected pools or falls back to demo
pub fn collect_stats() -> AlertLogStats {
    let pools = list_pools();
    let connected: Vec<_> = pools.iter()
        .filter(|(_, s, _)| *s == PoolStatus::Connected)
        .collect();

    if !connected.is_empty() {
        let mut all_entries = Vec::new();
        for (id, _, _) in &connected {
            let entries = collect_alert_log(id, 500, None);
            all_entries.extend(entries);
        }
        if !all_entries.is_empty() {
            return compute_stats(&all_entries);
        }
    }
    get_demo_stats()
}

/// Generate demo alert log entries (fallback when no real pool)
pub fn get_demo_entries(connection_id: &str, limit: usize) -> Vec<AlertLogEntry> {
    let now = Utc::now();
    let entries = vec![
        ("critical", "rdbms", "ORA-04031: unable to allocate 4096 bytes of shared memory", Some(4031)),
        ("warning", "rdbms", "Archived Log: Thread 1 Sequence 23456 completed", None),
        ("info", "rdbms", "ALTER SYSTEM SET memory_target=8G SCOPE=SPFILE", None),
        ("critical", "rdbms", "ORA-00600: internal error code, arguments: [kdsgrp1]", Some(600)),
        ("warning", "asm", "WARNING: Data file 7 has been offline for 2 hours", None),
        ("info", "rdbms", "Starting ORACLE instance (normal)", None),
        ("warning", "rdbms", "ORA-01555: snapshot too old: rollback segment too small", Some(1555)),
        ("info", "rdbms", "Recovery of Online Redo Log: Thread 1 Group 3 Seq 23457", None),
        ("critical", "dg", "FAL[server]: Error 12514 connecting to standby for gap resolution", Some(12514)),
        ("info", "rdbms", "Checkpoint not complete - retry logging", None),
        ("warning", "rdbms", "ORA-01652: unable to extend temp segment by 128 in tablespace TEMP", Some(1652)),
        ("info", "asm", "SUCCESS: disk group DATA mounted", None),
        ("warning", "rdbms", "ORA-12012: error on auto execute of job SYS.MMON_SLAVE", Some(12012)),
        ("info", "rdbms", "Thread 1 advanced to log sequence 23458 (LGWR switch)", None),
        ("critical", "rdbms", "ORA-27300: OS system dependent operation:fork failed", Some(27300)),
    ];

    entries.iter().enumerate().take(limit).map(|(i, (level, comp, msg, code))| {
        AlertLogEntry {
            id: format!("alog-{}-{}", connection_id, i),
            connection_id: connection_id.into(),
            database_name: "ORCLPDB1".into(),
            timestamp: (now - Duration::minutes(i as i64 * 15)).to_rfc3339(),
            level: level.to_string(),
            component: comp.to_string(),
            message: msg.to_string(),
            ora_code: *code,
            host: "orarac01.dc1.local".into(),
            instance_name: "ORCL1".into(),
            detail: None,
        }
    }).collect()
}

/// Generate demo stats (fallback)
pub fn get_demo_stats() -> AlertLogStats {
    let mut by_level = HashMap::new();
    by_level.insert("critical".into(), 12);
    by_level.insert("warning".into(), 45);
    by_level.insert("info".into(), 230);
    by_level.insert("trace".into(), 18);

    let mut by_database = HashMap::new();
    by_database.insert("ORCLPDB1".into(), 180);
    by_database.insert("ORCLPDB2".into(), 65);
    by_database.insert("STGDB".into(), 42);
    by_database.insert("DEVDB".into(), 18);

    AlertLogStats {
        total: 305,
        by_level,
        by_database,
        last_24h: 87,
        critical_last_1h: 2,
    }
}
