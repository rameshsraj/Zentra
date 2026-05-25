//! Rule Evaluator — Recording Rules Eval Loop + Alert Rule Groups
//!
//! Implements the Prometheus-style rule evaluation pipeline:
//! - Recording rules: periodically evaluate PromQL expressions and store results as new metrics
//! - Alert rule groups: group rules with independent evaluation intervals
//!
//! Reference: Prometheus `rules/recording.go`, `rules/group.go`

use std::collections::HashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingRule {
    pub rule_id: Uuid,
    pub record: String,         // new metric name
    pub expr: String,           // PromQL expression
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingRule {
    pub rule_id: Uuid,
    pub alert: String,          // alert name
    pub expr: String,           // PromQL condition
    pub for_duration: Option<String>,
    pub severity: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rule {
    Recording(RecordingRule),
    Alerting(AlertingRule),
}

impl Rule {
    pub fn name(&self) -> &str {
        match self {
            Rule::Recording(r) => &r.record,
            Rule::Alerting(r) => &r.alert,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleGroup {
    pub group_id: Uuid,
    pub name: String,
    pub file: String,
    pub interval_seconds: u64,
    pub rules: Vec<Rule>,
    pub last_evaluation: Option<DateTime<Utc>>,
    pub evaluation_duration_ms: Option<u64>,
}

impl RuleGroup {
    pub fn new(name: impl Into<String>, interval_seconds: u64) -> Self {
        Self {
            group_id: Uuid::now_v7(),
            name: name.into(),
            file: String::new(),
            interval_seconds,
            rules: Vec::new(),
            last_evaluation: None,
            evaluation_duration_ms: None,
        }
    }

    pub fn with_rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }
}

/// Evaluation result for a single rule
#[derive(Debug, Clone, Serialize)]
pub struct RuleEvalResult {
    pub rule_name: String,
    pub success: bool,
    pub samples_produced: u64,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub evaluated_at: DateTime<Utc>,
}

/// Health status for a rule group
#[derive(Debug, Clone, Serialize)]
pub struct RuleGroupHealth {
    pub group_name: String,
    pub rule_count: usize,
    pub last_evaluation: Option<DateTime<Utc>>,
    pub evaluation_duration_ms: Option<u64>,
    pub rules: Vec<RuleHealth>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleHealth {
    pub name: String,
    pub rule_type: String,
    pub health: String,
    pub last_error: Option<String>,
    pub evaluations_total: u64,
    pub evaluation_failures: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Rule Evaluator Engine
// ═══════════════════════════════════════════════════════════════════════════

pub struct RuleEvaluator {
    groups: Arc<RwLock<Vec<RuleGroup>>>,
    eval_results: Arc<RwLock<HashMap<String, Vec<RuleEvalResult>>>>,
    eval_counters: Arc<RwLock<HashMap<String, (u64, u64)>>>, // (total, failures) per rule
}

impl RuleEvaluator {
    pub fn new() -> Self {
        Self {
            groups: Arc::new(RwLock::new(Vec::new())),
            eval_results: Arc::new(RwLock::new(HashMap::new())),
            eval_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load rule groups from configuration
    pub async fn load_groups(&self, groups: Vec<RuleGroup>) {
        let mut locked = self.groups.write().await;
        info!(count = groups.len(), "Loading rule groups");
        *locked = groups;
    }

    /// Add a single rule group
    pub async fn add_group(&self, group: RuleGroup) {
        info!(name = %group.name, rules = group.rules.len(), interval = group.interval_seconds, "Adding rule group");
        self.groups.write().await.push(group);
    }

    /// Start the evaluation loop for all groups
    pub async fn run(&self, cancel: tokio_util::sync::CancellationToken) {
        let groups = self.groups.read().await.clone();

        if groups.is_empty() {
            info!("No rule groups loaded, evaluator idle");
            cancel.cancelled().await;
            return;
        }

        // Spawn one task per group with its own interval
        let mut handles = Vec::new();
        for group in groups {
            let groups_ref = self.groups.clone();
            let results_ref = self.eval_results.clone();
            let counters_ref = self.eval_counters.clone();
            let cancel_clone = cancel.clone();

            let handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(
                    std::time::Duration::from_secs(group.interval_seconds)
                );

                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            Self::evaluate_group(
                                &group,
                                &groups_ref,
                                &results_ref,
                                &counters_ref,
                            ).await;
                        }
                        _ = cancel_clone.cancelled() => {
                            info!(group = %group.name, "Rule group evaluator stopped");
                            return;
                        }
                    }
                }
            });
            handles.push(handle);
        }

        // Wait for cancellation
        cancel.cancelled().await;
        for h in handles {
            h.abort();
        }
        info!("Rule evaluator shutdown complete");
    }

    /// Evaluate all rules in a group
    async fn evaluate_group(
        group: &RuleGroup,
        _groups_ref: &Arc<RwLock<Vec<RuleGroup>>>,
        results_ref: &Arc<RwLock<HashMap<String, Vec<RuleEvalResult>>>>,
        counters_ref: &Arc<RwLock<HashMap<String, (u64, u64)>>>,
    ) {
        let start = std::time::Instant::now();
        debug!(group = %group.name, "Evaluating rule group");

        let mut group_results = Vec::new();

        for rule in &group.rules {
            let rule_start = std::time::Instant::now();
            let eval_result = match rule {
                Rule::Recording(rec) => Self::evaluate_recording_rule(rec).await,
                Rule::Alerting(alert) => Self::evaluate_alerting_rule(alert).await,
            };

            let duration = rule_start.elapsed().as_millis() as u64;
            let success = eval_result.is_ok();
            let samples = eval_result.as_ref().map(|s| *s).unwrap_or(0);

            let result = RuleEvalResult {
                rule_name: rule.name().to_string(),
                success,
                samples_produced: samples,
                duration_ms: duration,
                error: eval_result.err().map(|e| e.to_string()),
                evaluated_at: Utc::now(),
            };

            // Update counters
            {
                let mut counters = counters_ref.write().await;
                let entry = counters.entry(rule.name().to_string()).or_insert((0, 0));
                entry.0 += 1;
                if !success {
                    entry.1 += 1;
                }
            }

            group_results.push(result);
        }

        let total_duration = start.elapsed().as_millis() as u64;
        debug!(
            group = %group.name,
            rules = group_results.len(),
            duration_ms = total_duration,
            "Rule group evaluation complete"
        );

        // Store results (keep last 100 per group)
        let mut results = results_ref.write().await;
        let entry = results.entry(group.name.clone()).or_insert_with(Vec::new);
        entry.extend(group_results);
        if entry.len() > 100 {
            let drain_count = entry.len() - 100;
            entry.drain(..drain_count);
        }
    }

    /// Evaluate a recording rule — compute the expression and store as a new metric
    async fn evaluate_recording_rule(rule: &RecordingRule) -> Result<u64, String> {
        // In production:
        // 1. Parse rule.expr using promql::Parser
        // 2. Compile to SQL using promql::SqlCompiler
        // 3. Execute query against TimescaleDB
        // 4. Store results as new metrics with name = rule.record
        debug!(
            record = %rule.record,
            expr = %rule.expr,
            "Evaluating recording rule"
        );

        // Simulated: would produce N samples
        Ok(1)
    }

    /// Evaluate an alerting rule — check condition and manage alert state
    async fn evaluate_alerting_rule(rule: &AlertingRule) -> Result<u64, String> {
        // In production:
        // 1. Parse rule.expr using promql::Parser
        // 2. Execute against current metrics
        // 3. If result > 0, check for_duration and transition alert state
        // 4. Publish AlertTriggered/AlertResolved events
        debug!(
            alert = %rule.alert,
            expr = %rule.expr,
            severity = %rule.severity,
            "Evaluating alerting rule"
        );

        Ok(0)
    }

    /// Get health status for all rule groups
    pub async fn health(&self) -> Vec<RuleGroupHealth> {
        let groups = self.groups.read().await;
        let counters = self.eval_counters.read().await;
        let results = self.eval_results.read().await;

        groups.iter().map(|g| {
            let rules: Vec<RuleHealth> = g.rules.iter().map(|r| {
                let name = r.name().to_string();
                let (total, failures) = counters.get(&name).copied().unwrap_or((0, 0));
                let last_error = results.get(&g.name)
                    .and_then(|rs| rs.iter().rev().find(|r| r.rule_name == name && r.error.is_some()))
                    .and_then(|r| r.error.clone());

                RuleHealth {
                    name,
                    rule_type: match r {
                        Rule::Recording(_) => "recording".into(),
                        Rule::Alerting(_) => "alerting".into(),
                    },
                    health: if failures == 0 { "ok".into() } else { "err".into() },
                    last_error,
                    evaluations_total: total,
                    evaluation_failures: failures,
                }
            }).collect();

            RuleGroupHealth {
                group_name: g.name.clone(),
                rule_count: g.rules.len(),
                last_evaluation: g.last_evaluation,
                evaluation_duration_ms: g.evaluation_duration_ms,
                rules,
            }
        }).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Boolean Condition Trees
// ═══════════════════════════════════════════════════════════════════════════

/// A compound condition tree supporting AND, OR, NOT logic across multiple
/// PromQL-style expressions. Used for multi-condition alert rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionTree {
    /// Leaf node: a PromQL expression (evaluates to true if result > 0)
    Expr { query: String },
    /// Both children must be true
    And { left: Box<ConditionTree>, right: Box<ConditionTree> },
    /// At least one child must be true
    Or { left: Box<ConditionTree>, right: Box<ConditionTree> },
    /// Invert the child result
    Not { child: Box<ConditionTree> },
    /// Threshold comparison against a sigma-adjusted baseline
    DynamicThreshold {
        query: String,
        sigma: f64,
        direction: ThresholdDirection,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThresholdDirection {
    Above,
    Below,
}

/// Holds a rolling baseline (mean + stddev) for sigma-based thresholding.
#[derive(Debug, Clone)]
pub struct MetricBaseline {
    pub mean: f64,
    pub stddev: f64,
    pub sample_count: u64,
}

impl MetricBaseline {
    /// Returns the dynamic threshold value at `n` sigma above/below mean.
    pub fn threshold_at(&self, sigma: f64, direction: &ThresholdDirection) -> f64 {
        match direction {
            ThresholdDirection::Above => self.mean + sigma * self.stddev,
            ThresholdDirection::Below => self.mean - sigma * self.stddev,
        }
    }

    /// Welford's online algorithm: update mean and variance with a new sample.
    pub fn update(&mut self, value: f64) {
        self.sample_count += 1;
        let n = self.sample_count as f64;
        let delta = value - self.mean;
        self.mean += delta / n;
        let delta2 = value - self.mean;
        // Running sum of squares (M2); we store stddev as sqrt(M2/n)
        let m2 = (self.stddev * self.stddev) * (n - 1.0) + delta * delta2;
        self.stddev = if n > 1.0 { (m2 / n).sqrt() } else { 0.0 };
    }
}

/// Evaluates a ConditionTree against current metric snapshot (values keyed by metric name).
/// Returns true if the condition is satisfied.
pub fn evaluate_condition(
    tree: &ConditionTree,
    metrics: &HashMap<String, f64>,
    baselines: &HashMap<String, MetricBaseline>,
) -> bool {
    match tree {
        ConditionTree::Expr { query } => {
            // Simple: if metric name is in snapshot, check > 0
            metrics.get(query.trim()).copied().unwrap_or(0.0) > 0.0
        }
        ConditionTree::And { left, right } => {
            evaluate_condition(left, metrics, baselines)
                && evaluate_condition(right, metrics, baselines)
        }
        ConditionTree::Or { left, right } => {
            evaluate_condition(left, metrics, baselines)
                || evaluate_condition(right, metrics, baselines)
        }
        ConditionTree::Not { child } => {
            !evaluate_condition(child, metrics, baselines)
        }
        ConditionTree::DynamicThreshold { query, sigma, direction } => {
            let current = match metrics.get(query.trim()) {
                Some(v) => *v,
                None => return false,
            };
            let baseline = match baselines.get(query.trim()) {
                Some(b) => b,
                None => return false,
            };
            let threshold = baseline.threshold_at(*sigma, direction);
            match direction {
                ThresholdDirection::Above => current > threshold,
                ThresholdDirection::Below => current < threshold,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_group_builder() {
        let group = RuleGroup::new("cpu_rules", 60)
            .with_rule(Rule::Recording(RecordingRule {
                rule_id: Uuid::new_v4(),
                record: "instance:cpu_usage:avg5m".into(),
                expr: "avg(rate(cpu_usage_seconds_total[5m])) by (instance)".into(),
                labels: HashMap::new(),
            }))
            .with_rule(Rule::Alerting(AlertingRule {
                rule_id: Uuid::new_v4(),
                alert: "HighCPU".into(),
                expr: "instance:cpu_usage:avg5m > 0.9".into(),
                for_duration: Some("5m".into()),
                severity: "critical".into(),
                labels: HashMap::from([("team".into(), "sre".into())]),
                annotations: HashMap::from([("summary".into(), "High CPU on {{ $labels.instance }}".into())]),
            }));

        assert_eq!(group.name, "cpu_rules");
        assert_eq!(group.interval_seconds, 60);
        assert_eq!(group.rules.len(), 2);
        assert_eq!(group.rules[0].name(), "instance:cpu_usage:avg5m");
        assert_eq!(group.rules[1].name(), "HighCPU");
    }

    #[tokio::test]
    async fn test_evaluator_health() {
        let evaluator = RuleEvaluator::new();
        let group = RuleGroup::new("test", 15)
            .with_rule(Rule::Recording(RecordingRule {
                rule_id: Uuid::new_v4(),
                record: "test:metric".into(),
                expr: "avg(cpu)".into(),
                labels: HashMap::new(),
            }));

        evaluator.add_group(group).await;
        let health = evaluator.health().await;
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].group_name, "test");
        assert_eq!(health[0].rule_count, 1);
    }

    #[test]
    fn test_condition_tree_and() {
        let tree = ConditionTree::And {
            left: Box::new(ConditionTree::Expr { query: "cpu_high".into() }),
            right: Box::new(ConditionTree::Expr { query: "mem_high".into() }),
        };
        let mut metrics = HashMap::new();
        metrics.insert("cpu_high".into(), 1.0);
        // AND requires both — mem_high missing → false
        assert!(!evaluate_condition(&tree, &metrics, &HashMap::new()));
        metrics.insert("mem_high".into(), 1.0);
        assert!(evaluate_condition(&tree, &metrics, &HashMap::new()));
    }

    #[test]
    fn test_condition_tree_not() {
        let tree = ConditionTree::Not {
            child: Box::new(ConditionTree::Expr { query: "maintenance_mode".into() }),
        };
        let metrics = HashMap::new();
        // maintenance_mode absent (=0) → Not(false) = true
        assert!(evaluate_condition(&tree, &metrics, &HashMap::new()));
    }

    #[test]
    fn test_dynamic_threshold_sigma() {
        let mut baseline = MetricBaseline { mean: 50.0, stddev: 5.0, sample_count: 100 };
        // 2-sigma threshold above = 60.0
        let t = baseline.threshold_at(2.0, &ThresholdDirection::Above);
        assert!((t - 60.0).abs() < 0.01);

        // Update baseline with Welford's algorithm
        baseline.update(55.0);
        assert_eq!(baseline.sample_count, 101);

        let tree = ConditionTree::DynamicThreshold {
            query: "cpu_usage".into(),
            sigma: 2.0,
            direction: ThresholdDirection::Above,
        };
        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".into(), 65.0); // above 60.0 threshold
        let mut baselines = HashMap::new();
        baselines.insert("cpu_usage".into(), MetricBaseline { mean: 50.0, stddev: 5.0, sample_count: 100 });
        assert!(evaluate_condition(&tree, &metrics, &baselines));

        metrics.insert("cpu_usage".into(), 55.0); // below threshold
        assert!(!evaluate_condition(&tree, &metrics, &baselines));
    }
}
