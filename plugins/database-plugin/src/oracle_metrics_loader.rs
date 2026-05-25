//! Oracle Metrics Loader — TOML/YAML parser for metric definitions
//!
//! Parses [[metric]] TOML format and `metrics:` YAML format into
//! OracleMetricDefinition, validates SQL, merges custom overrides with defaults.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// Metric Definition Model
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleMetricDefinition {
    pub id: String,
    pub context: String,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub metrics_desc: HashMap<String, String>,
    #[serde(default)]
    pub metrics_type: HashMap<String, String>,
    #[serde(default)]
    pub metrics_buckets: HashMap<String, Vec<f64>>,
    pub field_to_append: Option<String>,
    pub request: String,
    #[serde(default)]
    pub ignore_zero_result: bool,
    pub query_timeout: Option<u32>,
    pub scrape_interval: Option<u32>,
    #[serde(default)]
    pub databases: Vec<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_true")]
    pub is_active: bool,
    pub source_file: Option<String>,
}

fn default_true() -> bool { true }

#[derive(Debug, Deserialize)]
struct TomlMetricFile {
    metric: Vec<TomlMetric>,
}

#[derive(Debug, Deserialize)]
struct TomlMetric {
    context: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    metrics_desc: HashMap<String, String>,
    #[serde(default)]
    metrics_type: HashMap<String, String>,
    #[serde(default)]
    metrics_buckets: HashMap<String, Vec<f64>>,
    field_to_append: Option<String>,
    request: String,
    #[serde(default)]
    ignore_zero_result: bool,
    query_timeout: Option<u32>,
    scrape_interval: Option<u32>,
    #[serde(default)]
    databases: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Parser: TOML
// ═══════════════════════════════════════════════════════════════════════════

pub fn parse_toml(content: &str, source_file: &str) -> Result<Vec<OracleMetricDefinition>, String> {
    let parsed: TomlMetricFile = toml::from_str(content)
        .map_err(|e| format!("TOML parse error: {}", e))?;

    let mut defs = Vec::new();
    for (i, m) in parsed.metric.into_iter().enumerate() {
        validate_sql(&m.request)?;
        defs.push(OracleMetricDefinition {
            id: format!("{}_{}", source_file.replace('.', "_"), i),
            context: m.context,
            labels: m.labels,
            metrics_desc: m.metrics_desc,
            metrics_type: m.metrics_type,
            metrics_buckets: m.metrics_buckets,
            field_to_append: m.field_to_append,
            request: m.request.trim().to_string(),
            ignore_zero_result: m.ignore_zero_result,
            query_timeout: m.query_timeout,
            scrape_interval: m.scrape_interval,
            databases: m.databases,
            is_default: true,
            is_active: true,
            source_file: Some(source_file.to_string()),
        });
    }
    Ok(defs)
}

// ═══════════════════════════════════════════════════════════════════════════
// Parser: YAML
// ═══════════════════════════════════════════════════════════════════════════

pub fn parse_yaml(content: &str, source_file: &str) -> Result<Vec<OracleMetricDefinition>, String> {
    let docs: Vec<serde_json::Value> = serde_json::from_str(content)
        .or_else(|_| {
            // Try parsing as YAML-style JSON array
            serde_json::from_str(&format!("[{}]", content))
        })
        .map_err(|e| format!("YAML/JSON parse error: {}", e))?;

    let mut defs = Vec::new();
    for (i, doc) in docs.into_iter().enumerate() {
        let context = doc.get("context").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
        let request = doc.get("request").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !request.is_empty() {
            validate_sql(&request)?;
        }
        defs.push(OracleMetricDefinition {
            id: format!("{}_{}", source_file.replace('.', "_"), i),
            context,
            labels: doc.get("labels").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
            metrics_desc: doc.get("metrics_desc").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
            metrics_type: doc.get("metrics_type").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
            metrics_buckets: HashMap::new(),
            field_to_append: doc.get("field_to_append").and_then(|v| v.as_str()).map(String::from),
            request,
            ignore_zero_result: doc.get("ignore_zero_result").and_then(|v| v.as_bool()).unwrap_or(false),
            query_timeout: doc.get("query_timeout").and_then(|v| v.as_u64()).map(|v| v as u32),
            scrape_interval: doc.get("scrape_interval").and_then(|v| v.as_u64()).map(|v| v as u32),
            databases: doc.get("databases").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
            is_default: false,
            is_active: true,
            source_file: Some(source_file.to_string()),
        });
    }
    Ok(defs)
}

// ═══════════════════════════════════════════════════════════════════════════
// SQL Validator (basic)
// ═══════════════════════════════════════════════════════════════════════════

fn validate_sql(sql: &str) -> Result<(), String> {
    let upper = sql.trim().to_uppercase();
    if upper.is_empty() {
        return Err("SQL request cannot be empty".into());
    }
    // Only allow SELECT statements
    if !upper.starts_with("SELECT") {
        return Err(format!("Only SELECT statements allowed, got: {}", &upper[..upper.len().min(30)]));
    }
    // Block dangerous keywords
    for kw in &["DROP", "DELETE", "INSERT", "UPDATE", "ALTER", "CREATE", "GRANT", "REVOKE", "TRUNCATE"] {
        if upper.contains(kw) {
            return Err(format!("Forbidden keyword '{}' in SQL", kw));
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Merge: custom overrides defaults (by context)
// ═══════════════════════════════════════════════════════════════════════════

pub fn merge_definitions(
    defaults: Vec<OracleMetricDefinition>,
    custom: Vec<OracleMetricDefinition>,
) -> Vec<OracleMetricDefinition> {
    let mut merged = defaults;
    for c in custom {
        if let Some(existing) = merged.iter_mut().find(|d| d.context == c.context && d.is_default) {
            // Custom overrides default
            existing.request = c.request;
            existing.labels = c.labels;
            existing.metrics_desc = c.metrics_desc;
            existing.metrics_type = c.metrics_type;
            existing.query_timeout = c.query_timeout.or(existing.query_timeout);
            existing.scrape_interval = c.scrape_interval.or(existing.scrape_interval);
            existing.source_file = c.source_file;
        } else {
            merged.push(c);
        }
    }
    merged
}

/// Load default metrics from embedded TOML
pub fn load_defaults() -> Vec<OracleMetricDefinition> {
    let content = include_str!("oracle_default_metrics.toml");
    parse_toml(content, "default-metrics.toml").unwrap_or_default()
}

/// Alias for load_defaults — used by scheduler
pub fn load_default_metrics() -> Vec<OracleMetricDefinition> {
    load_defaults()
}

// ═══════════════════════════════════════════════════════════════════════════
// P4: Hot Reload — SHA256 file watcher
// ═══════════════════════════════════════════════════════════════════════════

use std::io::Read;

#[derive(Debug, Clone)]
pub struct MetricFileWatcher {
    pub file_path: String,
    pub last_hash: String,
    pub last_loaded: String,
    pub reload_count: u64,
}

impl MetricFileWatcher {
    pub fn new(file_path: &str) -> Self {
        let hash = Self::compute_hash(file_path).unwrap_or_default();
        Self {
            file_path: file_path.to_string(),
            last_hash: hash,
            last_loaded: chrono::Utc::now().to_rfc3339(),
            reload_count: 0,
        }
    }

    /// Check if the file content has changed since last load.
    pub fn has_changed(&self) -> bool {
        match Self::compute_hash(&self.file_path) {
            Ok(h) => h != self.last_hash,
            Err(_) => false,
        }
    }

    /// Reload definitions from the watched file.
    pub fn reload(&mut self) -> Result<Vec<OracleMetricDefinition>, String> {
        let mut content = String::new();
        std::fs::File::open(&self.file_path)
            .and_then(|mut f| f.read_to_string(&mut content))
            .map_err(|e| format!("Failed to read {}: {}", self.file_path, e))?;

        let defs = if self.file_path.ends_with(".toml") {
            parse_toml(&content, &self.file_path)?
        } else {
            parse_yaml(&content, &self.file_path)?
        };

        self.last_hash = Self::compute_hash(&self.file_path).unwrap_or_default();
        self.last_loaded = chrono::Utc::now().to_rfc3339();
        self.reload_count += 1;

        Ok(defs)
    }

    fn compute_hash(path: &str) -> Result<String, String> {
        let mut content = String::new();
        std::fs::File::open(path)
            .and_then(|mut f| f.read_to_string(&mut content))
            .map_err(|e| format!("{}", e))?;

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        Ok(format!("{:016x}", hasher.finish()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// P4: Histogram bucket distribution
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistogramResult {
    pub bucket_boundaries: Vec<f64>,
    pub bucket_counts: Vec<u64>,
    pub sum: f64,
    pub count: u64,
}

/// Distribute numeric values into histogram buckets.
pub fn distribute_histogram(values: &[f64], boundaries: &[f64]) -> HistogramResult {
    let mut counts = vec![0u64; boundaries.len() + 1]; // +1 for +Inf
    let mut sum = 0.0f64;

    for &v in values {
        sum += v;
        let mut placed = false;
        for (i, &b) in boundaries.iter().enumerate() {
            if v <= b {
                counts[i] += 1;
                placed = true;
                break;
            }
        }
        if !placed {
            *counts.last_mut().unwrap() += 1;
        }
    }

    HistogramResult {
        bucket_boundaries: boundaries.to_vec(),
        bucket_counts: counts,
        sum,
        count: values.len() as u64,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// P4: Database-scoped metric filtering
// ═══════════════════════════════════════════════════════════════════════════

/// Filter metrics to only those applicable to a given connection ID.
/// If a metric has an empty `databases` list, it applies to all.
/// If `databases` is non-empty, only execute against listed connection IDs.
pub fn filter_for_connection(
    defs: &[OracleMetricDefinition],
    connection_id: &str,
) -> Vec<OracleMetricDefinition> {
    defs.iter()
        .filter(|d| d.is_active)
        .filter(|d| d.databases.is_empty() || d.databases.contains(&connection_id.to_string()))
        .cloned()
        .collect()
}

/// Inject connection-level labels into a metric's label set.
pub fn inject_connection_labels(
    labels: &mut HashMap<String, String>,
    connection_labels: &HashMap<String, String>,
) {
    for (k, v) in connection_labels {
        labels.entry(k.clone()).or_insert_with(|| v.clone());
    }
}
