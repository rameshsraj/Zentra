//! Prometheus-style recording rules for Zentra.
//! Pre-computes metric aggregates and stores them back as synthetic metrics.
//! Reference: prometheus-main/rules/ recording rule evaluation.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug};
use uuid::Uuid;

use zentra_core::models::metric::Metric;

/// Aggregation function for recording rules.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleAggregation {
    Avg,
    Sum,
    Min,
    Max,
    Count,
    P50,
    P95,
    P99,
    Rate,
    StdDev,
}

/// A recording rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingRule {
    pub rule_id: Uuid,
    pub name: String,
    pub source_metric: String,
    pub output_metric: String,
    pub aggregation: RuleAggregation,
    pub window_seconds: u32,
    pub group_by: Vec<String>,
    pub enabled: bool,
    pub evaluation_interval_seconds: u32,
    pub labels: HashMap<String, String>,
}

impl RecordingRule {
    pub fn new(
        name: impl Into<String>,
        source_metric: impl Into<String>,
        output_metric: impl Into<String>,
        aggregation: RuleAggregation,
        window_seconds: u32,
    ) -> Self {
        Self {
            rule_id: Uuid::now_v7(),
            name: name.into(),
            source_metric: source_metric.into(),
            output_metric: output_metric.into(),
            aggregation,
            window_seconds,
            group_by: Vec::new(),
            enabled: true,
            evaluation_interval_seconds: 60,
            labels: HashMap::new(),
        }
    }

    pub fn with_group_by(mut self, fields: Vec<String>) -> Self {
        self.group_by = fields;
        self
    }
}

/// Window of values for a single recording rule evaluation.
struct RuleWindow {
    values: Vec<(DateTime<Utc>, f64)>,
    window_seconds: u32,
}

impl RuleWindow {
    fn new(window_seconds: u32) -> Self {
        Self {
            values: Vec::new(),
            window_seconds,
        }
    }

    fn push(&mut self, ts: DateTime<Utc>, value: f64) {
        self.values.push((ts, value));
        let cutoff = ts - Duration::seconds(self.window_seconds as i64);
        self.values.retain(|(t, _)| *t >= cutoff);
    }

    fn evaluate(&self, agg: RuleAggregation) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let vals: Vec<f64> = self.values.iter().map(|(_, v)| *v).collect();
        let n = vals.len() as f64;

        match agg {
            RuleAggregation::Avg => Some(vals.iter().sum::<f64>() / n),
            RuleAggregation::Sum => Some(vals.iter().sum()),
            RuleAggregation::Min => vals.iter().cloned().reduce(f64::min),
            RuleAggregation::Max => vals.iter().cloned().reduce(f64::max),
            RuleAggregation::Count => Some(n),
            RuleAggregation::P50 => percentile_sorted(&vals, 50.0),
            RuleAggregation::P95 => percentile_sorted(&vals, 95.0),
            RuleAggregation::P99 => percentile_sorted(&vals, 99.0),
            RuleAggregation::Rate => {
                if self.values.len() < 2 {
                    return None;
                }
                let first = &self.values[0];
                let last = self.values.last().unwrap();
                let dt = (last.0 - first.0).num_seconds() as f64;
                if dt > 0.0 {
                    Some((last.1 - first.1) / dt)
                } else {
                    None
                }
            }
            RuleAggregation::StdDev => {
                let mean = vals.iter().sum::<f64>() / n;
                let variance = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
                Some(variance.sqrt())
            }
        }
    }
}

fn percentile_sorted(values: &[f64], pct: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    Some(sorted[idx.min(sorted.len() - 1)])
}

/// Recording rules evaluation engine.
pub struct RecordingRuleEngine {
    rules: Vec<RecordingRule>,
    /// Windows keyed by (rule_id, resource_id)
    windows: HashMap<(Uuid, String), RuleWindow>,
    /// Last evaluation time per rule
    last_eval: HashMap<Uuid, DateTime<Utc>>,
}

impl RecordingRuleEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            windows: HashMap::new(),
            last_eval: HashMap::new(),
        }
    }

    pub fn load_rules(&mut self, rules: Vec<RecordingRule>) {
        let enabled = rules.iter().filter(|r| r.enabled).count();
        info!(total = rules.len(), enabled, "Recording rules loaded");
        self.rules = rules;
    }

    pub fn add_rule(&mut self, rule: RecordingRule) {
        self.rules.push(rule);
    }

    /// Ingest metrics and produce synthetic recording rule outputs.
    pub fn evaluate(&mut self, metrics: &[Metric]) -> Vec<Metric> {
        let now = Utc::now();
        let mut output = Vec::new();

        for metric in metrics {
            for rule in &self.rules {
                if !rule.enabled || rule.source_metric != metric.metric_name {
                    continue;
                }

                let key = (rule.rule_id, metric.resource_id.clone());
                let window = self
                    .windows
                    .entry(key.clone())
                    .or_insert_with(|| RuleWindow::new(rule.window_seconds));
                window.push(now, metric.value);

                // Check if enough time has passed for evaluation
                let should_eval = match self.last_eval.get(&rule.rule_id) {
                    Some(last) => (now - *last).num_seconds() >= rule.evaluation_interval_seconds as i64,
                    None => true,
                };

                if should_eval {
                    if let Some(result) = window.evaluate(rule.aggregation) {
                        let mut synthetic = Metric::new(
                            &rule.output_metric,
                            result,
                            &metric.resource_id,
                        )
                        .with_context(metric.org_id, metric.target_id, "zentra-predict")
                        .with_resource_type(&metric.resource_type)
                        .with_tag("recording_rule", &rule.name)
                        .with_tag("source_metric", &rule.source_metric)
                        .with_tag("aggregation", &format!("{:?}", rule.aggregation));

                        for (k, v) in &rule.labels {
                            synthetic = synthetic.with_tag(k, v);
                        }

                        output.push(synthetic);
                        self.last_eval.insert(rule.rule_id, now);
                    }
                }
            }
        }

        if !output.is_empty() {
            debug!(count = output.len(), "Recording rules produced synthetic metrics");
        }

        output
    }

    /// Create default infrastructure recording rules.
    pub fn add_default_rules(&mut self) {
        self.add_rule(RecordingRule::new(
            "cpu_avg_5m", "server.cpu_usage", "recording:cpu_usage:avg_5m",
            RuleAggregation::Avg, 300,
        ));
        self.add_rule(RecordingRule::new(
            "cpu_max_5m", "server.cpu_usage", "recording:cpu_usage:max_5m",
            RuleAggregation::Max, 300,
        ));
        self.add_rule(RecordingRule::new(
            "memory_avg_5m", "server.memory_usage", "recording:memory_usage:avg_5m",
            RuleAggregation::Avg, 300,
        ));
        self.add_rule(RecordingRule::new(
            "disk_p95_5m", "storage.disk_usage_percent", "recording:disk_usage:p95_5m",
            RuleAggregation::P95, 300,
        ));
        self.add_rule(RecordingRule::new(
            "net_bytes_rate", "network.bytes_received", "recording:net_rx:rate_1m",
            RuleAggregation::Rate, 60,
        ));
        self.add_rule(RecordingRule::new(
            "db_connections_avg_5m", "database.active_connections", "recording:db_connections:avg_5m",
            RuleAggregation::Avg, 300,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_rule_evaluation() {
        let mut engine = RecordingRuleEngine::new();
        engine.add_rule(RecordingRule::new(
            "cpu_avg", "server.cpu_usage", "recording:cpu:avg",
            RuleAggregation::Avg, 300,
        ));

        let metrics = vec![
            Metric::new("server.cpu_usage", 50.0, "host-1")
                .with_context(Uuid::new_v4(), Uuid::new_v4(), "server-plugin"),
            Metric::new("server.cpu_usage", 60.0, "host-1")
                .with_context(Uuid::new_v4(), Uuid::new_v4(), "server-plugin"),
        ];

        let output = engine.evaluate(&metrics);
        assert!(!output.is_empty());
        assert_eq!(output[0].metric_name, "recording:cpu:avg");
    }

    #[test]
    fn test_rate_aggregation() {
        let mut win = RuleWindow::new(60);
        let now = Utc::now();

        win.push(now - Duration::seconds(60), 100.0);
        win.push(now, 200.0);

        let rate = win.evaluate(RuleAggregation::Rate).unwrap();
        // (200 - 100) / 60 ≈ 1.67
        assert!((rate - 100.0 / 60.0).abs() < 0.01);
    }

    #[test]
    fn test_percentile() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p50 = percentile_sorted(&vals, 50.0);
        assert!(p50.is_some());
        let p95 = percentile_sorted(&vals, 95.0);
        assert!(p95.is_some());
    }
}
