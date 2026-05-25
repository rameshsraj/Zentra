//! Scrape-level recording rules for Zentra.
//! Pre-computes aggregates from scraped metrics and writes them back
//! as synthetic metrics into the ingestion pipeline.
//! Reference: prometheus-main/rules/ recording rule evaluation.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;
use uuid::Uuid;

use zentra_core::models::metric::Metric;
use crate::parser::PrometheusSample;

/// Aggregation operation for scrape recording rules.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrapeAggregation {
    Sum,
    Avg,
    Min,
    Max,
    Count,
    Rate,
}

/// A scrape-level recording rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapeRecordingRule {
    pub rule_id: Uuid,
    pub name: String,
    pub source_metric: String,
    pub output_metric: String,
    pub aggregation: ScrapeAggregation,
    /// Group-by label keys (e.g., ["job", "instance"])
    pub group_by: Vec<String>,
    pub enabled: bool,
}

impl ScrapeRecordingRule {
    pub fn new(
        name: impl Into<String>,
        source: impl Into<String>,
        output: impl Into<String>,
        agg: ScrapeAggregation,
    ) -> Self {
        Self {
            rule_id: Uuid::now_v7(),
            name: name.into(),
            source_metric: source.into(),
            output_metric: output.into(),
            aggregation: agg,
            group_by: Vec::new(),
            enabled: true,
        }
    }

    pub fn with_group_by(mut self, keys: Vec<String>) -> Self {
        self.group_by = keys;
        self
    }
}

/// Evaluate recording rules against a batch of scraped samples.
pub fn evaluate_rules(
    rules: &[ScrapeRecordingRule],
    samples: &[PrometheusSample],
    org_id: Uuid,
    target_id: Uuid,
) -> Vec<Metric> {
    let mut output = Vec::new();

    for rule in rules {
        if !rule.enabled {
            continue;
        }

        // Filter matching samples
        let matching: Vec<&PrometheusSample> = samples
            .iter()
            .filter(|s| s.metric_name == rule.source_metric)
            .collect();

        if matching.is_empty() {
            continue;
        }

        // Group by specified label keys
        let mut groups: HashMap<Vec<(String, String)>, Vec<f64>> = HashMap::new();
        for sample in &matching {
            let group_key: Vec<(String, String)> = rule
                .group_by
                .iter()
                .map(|k| {
                    let v = sample.labels.get(k).cloned().unwrap_or_default();
                    (k.clone(), v)
                })
                .collect();
            groups.entry(group_key).or_default().push(sample.value);
        }

        // Compute aggregates per group
        for (group_key, values) in &groups {
            let result = match rule.aggregation {
                ScrapeAggregation::Sum => values.iter().sum(),
                ScrapeAggregation::Avg => values.iter().sum::<f64>() / values.len() as f64,
                ScrapeAggregation::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
                ScrapeAggregation::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                ScrapeAggregation::Count => values.len() as f64,
                ScrapeAggregation::Rate => {
                    // Simple rate: (last - first) / count
                    if values.len() >= 2 {
                        (values.last().unwrap() - values.first().unwrap()) / (values.len() - 1) as f64
                    } else {
                        0.0
                    }
                }
            };

            let resource = group_key
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",");

            let resource_id = if resource.is_empty() { "global".to_string() } else { resource };

            let mut m = Metric::new(&rule.output_metric, result, &resource_id)
                .with_context(org_id, target_id, "zentra-scrape")
                .with_resource_type("recording_rule")
                .with_tag("recording_rule", &rule.name);

            for (k, v) in group_key {
                m = m.with_tag(k, v);
            }

            output.push(m);
        }
    }

    debug!(count = output.len(), "Scrape recording rules evaluated");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str, value: f64, labels: &[(&str, &str)]) -> PrometheusSample {
        let mut lmap = HashMap::new();
        for (k, v) in labels {
            lmap.insert(k.to_string(), v.to_string());
        }
        PrometheusSample {
            metric_name: name.into(),
            labels: lmap,
            value,
            timestamp: None,
        }
    }

    #[test]
    fn test_sum_rule() {
        let rule = ScrapeRecordingRule::new(
            "total_requests", "http_requests_total", "recording:http_requests:sum",
            ScrapeAggregation::Sum,
        );

        let samples = vec![
            sample("http_requests_total", 100.0, &[("job", "web")]),
            sample("http_requests_total", 200.0, &[("job", "api")]),
        ];

        let output = evaluate_rules(&[rule], &samples, Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].value, 300.0);
    }

    #[test]
    fn test_avg_grouped_rule() {
        let rule = ScrapeRecordingRule::new(
            "avg_by_job", "cpu_usage", "recording:cpu:avg_by_job",
            ScrapeAggregation::Avg,
        ).with_group_by(vec!["job".into()]);

        let samples = vec![
            sample("cpu_usage", 50.0, &[("job", "web")]),
            sample("cpu_usage", 70.0, &[("job", "web")]),
            sample("cpu_usage", 30.0, &[("job", "api")]),
        ];

        let output = evaluate_rules(&[rule], &samples, Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(output.len(), 2); // Two groups: web, api
    }
}
