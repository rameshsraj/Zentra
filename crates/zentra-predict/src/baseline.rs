//! Metric baseline calculation using rolling mean + standard deviation.
//! Inspired by Netdata ML training_window approach (ml_config.h).
//! Uses a sliding 7-day window to compute normal ranges for each metric.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;
use uuid::Uuid;

/// Statistical baseline for a single metric stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricBaseline {
    pub baseline_id: Uuid,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub metric_name: String,
    pub resource_id: String,
    pub mean: f64,
    pub stddev: f64,
    pub min: f64,
    pub max: f64,
    pub sample_count: u64,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub computed_at: DateTime<Utc>,
}

impl MetricBaseline {
    /// Check if a value falls within the normal range (mean ± n*stddev).
    pub fn is_normal(&self, value: f64, sigma: f64) -> bool {
        let lower = self.mean - sigma * self.stddev;
        let upper = self.mean + sigma * self.stddev;
        value >= lower && value <= upper
    }

    /// Compute the Z-score for a given value.
    pub fn z_score(&self, value: f64) -> f64 {
        if self.stddev == 0.0 {
            return 0.0;
        }
        (value - self.mean) / self.stddev
    }
}

/// Sliding window of (timestamp, value) pairs for baseline computation.
#[derive(Debug, Clone)]
struct SlidingWindow {
    values: Vec<(DateTime<Utc>, f64)>,
    window_duration: Duration,
}

impl SlidingWindow {
    fn new(window_duration: Duration) -> Self {
        Self {
            values: Vec::new(),
            window_duration,
        }
    }

    fn push(&mut self, ts: DateTime<Utc>, value: f64) {
        self.values.push((ts, value));
        self.trim(ts);
    }

    fn trim(&mut self, now: DateTime<Utc>) {
        let cutoff = now - self.window_duration;
        self.values.retain(|(ts, _)| *ts >= cutoff);
    }

    fn compute_stats(&self) -> Option<(f64, f64, f64, f64, u64)> {
        if self.values.is_empty() {
            return None;
        }
        let vals: Vec<f64> = self.values.iter().map(|(_, v)| *v).collect();
        let n = vals.len() as f64;
        let mean = vals.iter().sum::<f64>() / n;

        let variance = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
        let stddev = variance.sqrt();

        let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        Some((mean, stddev, min, max, vals.len() as u64))
    }
}

/// Key identifying a unique metric stream.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct MetricKey {
    target_id: Uuid,
    metric_name: String,
    resource_id: String,
}

/// Engine that maintains rolling baselines for all metric streams.
pub struct BaselineEngine {
    windows: HashMap<MetricKey, SlidingWindow>,
    baselines: HashMap<MetricKey, MetricBaseline>,
    last_push: HashMap<MetricKey, DateTime<Utc>>,
    window_duration: Duration,
    recompute_interval: Duration,
    last_recompute: DateTime<Utc>,
    max_keys: usize,
}

impl BaselineEngine {
    /// Create a new baseline engine with a 7-day sliding window.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            baselines: HashMap::new(),
            last_push: HashMap::new(),
            window_duration: Duration::days(7),
            recompute_interval: Duration::hours(1),
            last_recompute: Utc::now(),
            max_keys: 10_000,
        }
    }

    /// Create with custom window duration.
    pub fn with_window(window_duration: Duration) -> Self {
        Self {
            windows: HashMap::new(),
            baselines: HashMap::new(),
            last_push: HashMap::new(),
            window_duration,
            recompute_interval: Duration::hours(1),
            last_recompute: Utc::now(),
            max_keys: 10_000,
        }
    }

    /// Ingest a metric data point.
    pub fn push(
        &mut self,
        org_id: Uuid,
        target_id: Uuid,
        metric_name: &str,
        resource_id: &str,
        value: f64,
        ts: DateTime<Utc>,
    ) {
        let key = MetricKey {
            target_id,
            metric_name: metric_name.to_string(),
            resource_id: resource_id.to_string(),
        };

        let window = self
            .windows
            .entry(key.clone())
            .or_insert_with(|| SlidingWindow::new(self.window_duration));
        window.push(ts, value);
        self.last_push.insert(key, ts);

        // Recompute baselines periodically
        if ts - self.last_recompute >= self.recompute_interval {
            self.recompute_all(org_id);
            self.last_recompute = ts;
        }
    }

    /// Get the baseline for a specific metric stream.
    pub fn get_baseline(&self, target_id: Uuid, metric_name: &str, resource_id: &str) -> Option<&MetricBaseline> {
        let key = MetricKey {
            target_id,
            metric_name: metric_name.to_string(),
            resource_id: resource_id.to_string(),
        };
        self.baselines.get(&key)
    }

    /// Get all computed baselines.
    pub fn all_baselines(&self) -> Vec<&MetricBaseline> {
        self.baselines.values().collect()
    }

    /// Evict stale keys and enforce the max_keys cap.
    pub fn evict(&mut self) {
        let now = Utc::now();
        let stale_cutoff = now - self.window_duration * 2;

        // Remove keys that haven't received data in 2× the window duration
        let stale_keys: Vec<MetricKey> = self
            .last_push
            .iter()
            .filter(|(_, ts)| **ts < stale_cutoff)
            .map(|(k, _)| k.clone())
            .collect();
        for key in &stale_keys {
            self.windows.remove(key);
            self.baselines.remove(key);
            self.last_push.remove(key);
        }
        if !stale_keys.is_empty() {
            debug!(evicted = stale_keys.len(), "Evicted stale baseline keys");
        }

        // If still over capacity, evict keys with fewest samples
        if self.windows.len() > self.max_keys {
            let mut by_count: Vec<(MetricKey, usize)> = self
                .windows
                .iter()
                .map(|(k, w)| (k.clone(), w.values.len()))
                .collect();
            by_count.sort_by_key(|(_, c)| *c);
            let to_remove = self.windows.len() - self.max_keys;
            for (key, _) in by_count.into_iter().take(to_remove) {
                self.windows.remove(&key);
                self.baselines.remove(&key);
                self.last_push.remove(&key);
            }
            debug!(removed = to_remove, cap = self.max_keys, "Evicted baseline keys over capacity");
        }
    }

    /// Recompute all baselines from current windows.
    pub fn recompute_all(&mut self, org_id: Uuid) {
        self.evict();
        let now = Utc::now();
        for (key, window) in &self.windows {
            if let Some((mean, stddev, min, max, count)) = window.compute_stats() {
                let baseline = MetricBaseline {
                    baseline_id: Uuid::now_v7(),
                    org_id,
                    target_id: key.target_id,
                    metric_name: key.metric_name.clone(),
                    resource_id: key.resource_id.clone(),
                    mean,
                    stddev,
                    min,
                    max,
                    sample_count: count,
                    window_start: now - self.window_duration,
                    window_end: now,
                    computed_at: now,
                };
                self.baselines.insert(key.clone(), baseline);
            }
        }
        debug!(count = self.baselines.len(), "Baselines recomputed");
    }

    /// Get the number of baselines currently tracked.
    pub fn baseline_count(&self) -> usize {
        self.baselines.len()
    }

    /// Get the number of active metric windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
}

impl Default for BaselineEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_computation() {
        let mut engine = BaselineEngine::with_window(Duration::hours(1));
        let org = Uuid::new_v4();
        let target = Uuid::new_v4();
        let now = Utc::now();

        // Push 100 data points around mean=50
        for i in 0..100 {
            let ts = now - Duration::minutes(60 - i);
            let value = 50.0 + (i as f64 % 10.0) - 5.0;
            engine.push(org, target, "cpu_usage", "host-1", value, ts);
        }

        engine.recompute_all(org);
        let bl = engine.get_baseline(target, "cpu_usage", "host-1").unwrap();

        assert!(bl.mean > 45.0 && bl.mean < 55.0);
        assert!(bl.stddev > 0.0);
        assert!(bl.sample_count == 100);
    }

    #[test]
    fn test_z_score() {
        let bl = MetricBaseline {
            baseline_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            metric_name: "test".into(),
            resource_id: "r".into(),
            mean: 50.0,
            stddev: 10.0,
            min: 30.0,
            max: 70.0,
            sample_count: 100,
            window_start: Utc::now(),
            window_end: Utc::now(),
            computed_at: Utc::now(),
        };

        assert!((bl.z_score(50.0)).abs() < f64::EPSILON);
        assert!((bl.z_score(60.0) - 1.0).abs() < f64::EPSILON);
        assert!((bl.z_score(80.0) - 3.0).abs() < f64::EPSILON);
        assert!(bl.is_normal(55.0, 3.0));
        assert!(!bl.is_normal(85.0, 3.0));
    }
}
