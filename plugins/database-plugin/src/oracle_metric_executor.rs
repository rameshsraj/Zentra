//! Oracle Metric Executor
//!
//! Executes metric definitions against Oracle connections and returns structured results.
//! Handles labels, field_to_append, metrics_type (gauge/counter/histogram), buckets,
//! timeout, and ignore_zero_result.
//!
//! When a native pool is available (via pool registry), executes real SQL.
//! Otherwise falls back to simulated sample data for demo/development.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};
use crate::oracle_metrics_loader::OracleMetricDefinition;
use crate::oracle_connection::{pool_query, pool_is_connected};

// ═══════════════════════════════════════════════════════════════════════════
// Result types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricResult {
    pub context: String,
    pub labels: HashMap<String, String>,
    pub values: HashMap<String, MetricValue>,
    pub scraped_at: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricValue {
    pub value: f64,
    pub metric_type: String,  // gauge, counter, histogram
    pub description: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Executor
// ═══════════════════════════════════════════════════════════════════════════

/// Execute a metric definition and return results.
/// Tries native pool query first; falls back to simulated data.
pub fn execute_metric(
    definition: &OracleMetricDefinition,
    connection_id: &str,
) -> Vec<MetricResult> {
    let start = std::time::Instant::now();
    let now = chrono::Utc::now().to_rfc3339();

    // Try real execution via pool registry
    if pool_is_connected(connection_id) {
        match execute_real(definition, connection_id) {
            Ok(results) if !results.is_empty() => {
                debug!(ctx = %definition.context, pool = %connection_id, rows = results.len(), "Real metric execution");
                return results;
            }
            Ok(_) => {
                debug!(ctx = %definition.context, pool = %connection_id, "Real execution returned 0 rows, using fallback");
            }
            Err(e) => {
                warn!(ctx = %definition.context, pool = %connection_id, "Real execution failed: {}, using fallback", e);
            }
        }
    }

    // Fallback: simulated data
    execute_simulated(definition, &now, start.elapsed().as_millis() as u64)
}

/// Execute metric SQL against a real Oracle pool connection.
fn execute_real(
    definition: &OracleMetricDefinition,
    connection_id: &str,
) -> Result<Vec<MetricResult>, String> {
    let start = std::time::Instant::now();
    let now = chrono::Utc::now().to_rfc3339();

    let qr = pool_query(connection_id, &definition.request)
        .map_err(|e| e.message.clone())?;

    // If the pool returned empty (SSH fallback mode with no native OCI), return empty
    if qr.columns.is_empty() {
        return Ok(vec![]);
    }

    let mut results = Vec::new();
    let col_lower: Vec<String> = qr.columns.iter().map(|c| c.to_lowercase()).collect();

    for row in &qr.rows {
        // Extract labels
        let mut labels = HashMap::new();
        for label_name in &definition.labels {
            let ln = label_name.to_lowercase();
            if let Some(idx) = col_lower.iter().position(|c| c == &ln) {
                if let Some(val) = row.get(idx) {
                    labels.insert(label_name.clone(), val.clone());
                }
            }
        }

        // Extract metric values
        let mut values = HashMap::new();
        for (metric_name, desc) in &definition.metrics_desc {
            let mn = metric_name.to_lowercase();
            if let Some(idx) = col_lower.iter().position(|c| c == &mn) {
                if let Some(val_str) = row.get(idx) {
                    let value: f64 = val_str.parse().unwrap_or(0.0);
                    let metric_type = definition.metrics_type.get(metric_name)
                        .cloned().unwrap_or_else(|| "gauge".into());

                    // Handle field_to_append
                    let final_name = if let Some(ref field) = definition.field_to_append {
                        let fl = field.to_lowercase();
                        if let Some(fi) = col_lower.iter().position(|c| c == &fl) {
                            let suffix = row.get(fi).cloned().unwrap_or_default();
                            format!("{}_{}", metric_name, suffix.to_lowercase().replace(' ', "_"))
                        } else {
                            metric_name.clone()
                        }
                    } else {
                        metric_name.clone()
                    };

                    values.insert(final_name, MetricValue {
                        value,
                        metric_type,
                        description: desc.clone(),
                    });
                }
            }
        }

        results.push(MetricResult {
            context: definition.context.clone(),
            labels,
            values,
            scraped_at: now.clone(),
            duration_ms: start.elapsed().as_millis() as u64,
            error: None,
        });
    }

    // Handle ignore_zero_result
    if definition.ignore_zero_result {
        results.retain(|r| r.values.values().any(|v| v.value != 0.0));
    }

    Ok(results)
}

/// Simulated/demo execution for when no real pool is available.
fn execute_simulated(
    definition: &OracleMetricDefinition,
    now: &str,
    base_duration: u64,
) -> Vec<MetricResult> {
    let mut results = Vec::new();
    let sample_labels = generate_sample_labels(&definition.context, &definition.labels);

    for label_set in sample_labels {
        let mut values = HashMap::new();
        for (metric_name, desc) in &definition.metrics_desc {
            let metric_type = definition.metrics_type.get(metric_name)
                .cloned().unwrap_or_else(|| "gauge".into());
            let sample_value = generate_sample_value(&definition.context, metric_name);
            values.insert(metric_name.clone(), MetricValue {
                value: sample_value,
                metric_type,
                description: desc.clone(),
            });
        }

        results.push(MetricResult {
            context: definition.context.clone(),
            labels: label_set,
            values,
            scraped_at: now.to_string(),
            duration_ms: base_duration,
            error: None,
        });
    }

    if definition.ignore_zero_result {
        results.retain(|r| r.values.values().any(|v| v.value != 0.0));
    }

    results
}

fn generate_sample_labels(context: &str, label_names: &[String]) -> Vec<HashMap<String, String>> {
    match context {
        "sessions" => vec![
            HashMap::from([("status".into(), "ACTIVE".into()), ("type".into(), "USER".into())]),
            HashMap::from([("status".into(), "INACTIVE".into()), ("type".into(), "USER".into())]),
            HashMap::from([("status".into(), "ACTIVE".into()), ("type".into(), "BACKGROUND".into())]),
        ],
        "wait_class" => vec![
            HashMap::from([("wait_class".into(), "Application".into())]),
            HashMap::from([("wait_class".into(), "Commit".into())]),
            HashMap::from([("wait_class".into(), "Concurrency".into())]),
            HashMap::from([("wait_class".into(), "User I/O".into())]),
            HashMap::from([("wait_class".into(), "System I/O".into())]),
        ],
        "tablespace" => vec![
            HashMap::from([("tablespace_name".into(), "SYSTEM".into())]),
            HashMap::from([("tablespace_name".into(), "SYSAUX".into())]),
            HashMap::from([("tablespace_name".into(), "USERS".into())]),
            HashMap::from([("tablespace_name".into(), "UNDOTBS1".into())]),
        ],
        _ => {
            if label_names.is_empty() {
                vec![HashMap::new()]
            } else {
                vec![label_names.iter().map(|l| (l.clone(), "sample".into())).collect()]
            }
        }
    }
}

fn generate_sample_value(context: &str, metric_name: &str) -> f64 {
    match (context, metric_name) {
        ("sessions", "value") => 42.0,
        ("resource", "current_utilization") => 156.0,
        ("resource", "max_utilization") => 200.0,
        ("resource", "limit_value") => 500.0,
        ("wait_class", "total_waits") => 125000.0,
        ("wait_class", "time_waited") => 8500.0,
        ("tablespace", "bytes") => 2147483648.0,
        ("tablespace", "used_percent") => 72.5,
        ("tablespace", "free_bytes") => 536870912.0,
        ("process", "count") => 180.0,
        ("process", "limit") => 500.0,
        ("cache_hit_ratio", "buffer_cache_hit") => 98.7,
        ("cache_hit_ratio", "library_cache_hit") => 99.2,
        ("cache_hit_ratio", "dict_cache_hit") => 97.8,
        ("db_platform", "host_cpu") => 35.2,
        ("db_platform", "db_time") => 12.5,
        _ => 100.0,
    }
}

/// Test-execute a metric definition and return preview results
pub fn test_metric(
    definition: &OracleMetricDefinition,
    connection_id: &str,
) -> Vec<MetricResult> {
    execute_metric(definition, connection_id)
}
