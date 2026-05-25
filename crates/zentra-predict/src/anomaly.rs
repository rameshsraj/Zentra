//! Z-score based anomaly detection engine for Zentra.
//! Inspired by Netdata's ml_kmeans anomaly scoring approach.
//! Flags deviations when value exceeds 3σ from the rolling baseline.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};
use uuid::Uuid;

use crate::baseline::{BaselineEngine, MetricBaseline};

/// Anomaly severity classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl AnomalySeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn from_z_score(z: f64) -> Self {
        let abs_z = z.abs();
        if abs_z >= 5.0 {
            Self::Critical
        } else if abs_z >= 4.0 {
            Self::High
        } else if abs_z >= 3.5 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

/// A detected anomaly event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyEvent {
    pub anomaly_id: Uuid,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub metric_name: String,
    pub resource_id: String,
    pub observed_value: f64,
    pub expected_mean: f64,
    pub expected_stddev: f64,
    pub z_score: f64,
    pub severity: AnomalySeverity,
    pub description: String,
    pub detected_at: DateTime<Utc>,
    pub baseline_window_start: DateTime<Utc>,
    pub baseline_window_end: DateTime<Utc>,
}

/// Configuration for the anomaly detector.
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Minimum Z-score to flag an anomaly (default: 3.0)
    pub z_threshold: f64,
    /// Minimum samples required before anomaly detection activates
    pub min_samples: u64,
    /// Whether to detect low-side anomalies (value too low)
    pub detect_low: bool,
    /// Whether to detect high-side anomalies (value too high)
    pub detect_high: bool,
    /// Cooldown period in seconds before re-firing for the same metric
    pub cooldown_seconds: i64,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            z_threshold: 3.0,
            min_samples: 30,
            detect_low: true,
            detect_high: true,
            cooldown_seconds: 300,
        }
    }
}

/// Anomaly detection engine that operates on top of the baseline engine.
pub struct AnomalyDetector {
    config: AnomalyConfig,
    /// Track last anomaly time per metric stream to enforce cooldowns
    last_anomaly: std::collections::HashMap<(Uuid, String, String), DateTime<Utc>>,
    /// Accumulated anomaly events
    events: Vec<AnomalyEvent>,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        Self {
            config,
            last_anomaly: std::collections::HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(AnomalyConfig::default())
    }

    /// Check a metric value against its baseline and detect anomalies.
    pub fn check(
        &mut self,
        baseline: &MetricBaseline,
        value: f64,
        ts: DateTime<Utc>,
    ) -> Option<AnomalyEvent> {
        // Periodic eviction: keep only last 1000 events and last_anomaly entries
        if self.events.len() > 1000 {
            let cutoff = self.events.len() - 1000;
            self.events.drain(0..cutoff);
        }
        if self.last_anomaly.len() > 5000 {
            self.last_anomaly.retain(|_, v| (ts - *v).num_hours() < 24);
        }
        // Need sufficient samples
        if baseline.sample_count < self.config.min_samples {
            return None;
        }

        let z = baseline.z_score(value);
        let abs_z = z.abs();

        // Must exceed threshold
        if abs_z < self.config.z_threshold {
            return None;
        }

        // Direction filter
        if z > 0.0 && !self.config.detect_high {
            return None;
        }
        if z < 0.0 && !self.config.detect_low {
            return None;
        }

        // Cooldown check
        let key = (baseline.target_id, baseline.metric_name.clone(), baseline.resource_id.clone());
        if let Some(last) = self.last_anomaly.get(&key) {
            if (ts - *last).num_seconds() < self.config.cooldown_seconds {
                debug!(
                    metric = %baseline.metric_name,
                    "Anomaly cooldown active, skipping"
                );
                return None;
            }
        }

        let severity = AnomalySeverity::from_z_score(z);
        let direction = if z > 0.0 { "above" } else { "below" };

        let event = AnomalyEvent {
            anomaly_id: Uuid::now_v7(),
            org_id: baseline.org_id,
            target_id: baseline.target_id,
            metric_name: baseline.metric_name.clone(),
            resource_id: baseline.resource_id.clone(),
            observed_value: value,
            expected_mean: baseline.mean,
            expected_stddev: baseline.stddev,
            z_score: z,
            severity,
            description: format!(
                "{} is {:.1}σ {} normal: observed={:.2}, expected={:.2}±{:.2}",
                baseline.metric_name, abs_z, direction, value, baseline.mean, baseline.stddev
            ),
            detected_at: ts,
            baseline_window_start: baseline.window_start,
            baseline_window_end: baseline.window_end,
        };

        info!(
            metric = %event.metric_name,
            z_score = format!("{:.2}", z),
            severity = %severity.as_str(),
            "Anomaly detected"
        );

        self.last_anomaly.insert(key, ts);
        self.events.push(event.clone());
        Some(event)
    }

    /// Evaluate a batch of metrics against their baselines.
    pub fn evaluate_batch(
        &mut self,
        baselines: &BaselineEngine,
        metrics: &[zentra_core::models::metric::Metric],
    ) -> Vec<AnomalyEvent> {
        let mut anomalies = Vec::new();
        let now = Utc::now();

        for m in metrics {
            if let Some(bl) = baselines.get_baseline(m.target_id, &m.metric_name, &m.resource_id) {
                if let Some(event) = self.check(bl, m.value, now) {
                    anomalies.push(event);
                }
            }
        }

        anomalies
    }

    /// Get all detected anomaly events.
    pub fn events(&self) -> &[AnomalyEvent] {
        &self.events
    }

    /// Get recent anomaly events (within the last N seconds).
    pub fn recent_events(&self, seconds: i64) -> Vec<&AnomalyEvent> {
        let cutoff = Utc::now() - chrono::Duration::seconds(seconds);
        self.events.iter().filter(|e| e.detected_at >= cutoff).collect()
    }

    /// Clear old events.
    pub fn cleanup(&mut self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        self.events.retain(|e| e.detected_at >= cutoff);
        self.last_anomaly.retain(|_, ts| *ts >= cutoff);
    }

    /// Total number of anomalies ever detected.
    pub fn total_count(&self) -> usize {
        self.events.len()
    }
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::MetricBaseline;

    fn test_baseline() -> MetricBaseline {
        MetricBaseline {
            baseline_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            metric_name: "cpu_usage".into(),
            resource_id: "host-1".into(),
            mean: 50.0,
            stddev: 10.0,
            min: 20.0,
            max: 80.0,
            sample_count: 1000,
            window_start: Utc::now() - chrono::Duration::days(7),
            window_end: Utc::now(),
            computed_at: Utc::now(),
        }
    }

    #[test]
    fn test_anomaly_detection() {
        let bl = test_baseline();
        let mut detector = AnomalyDetector::with_defaults();

        // Normal value — no anomaly
        assert!(detector.check(&bl, 55.0, Utc::now()).is_none());

        // 3σ+ above — anomaly
        let event = detector.check(&bl, 85.0, Utc::now());
        assert!(event.is_some());
        let ev = event.unwrap();
        assert!(ev.z_score > 3.0);
        assert_eq!(ev.severity, AnomalySeverity::Low);
    }

    #[test]
    fn test_critical_anomaly() {
        let bl = test_baseline();
        let mut detector = AnomalyDetector::with_defaults();

        // 5σ above — critical
        let event = detector.check(&bl, 100.0, Utc::now()).unwrap();
        assert_eq!(event.severity, AnomalySeverity::Critical);
    }

    #[test]
    fn test_cooldown() {
        let bl = test_baseline();
        let mut detector = AnomalyDetector::new(AnomalyConfig {
            cooldown_seconds: 300,
            ..Default::default()
        });

        let now = Utc::now();
        // First anomaly fires
        assert!(detector.check(&bl, 85.0, now).is_some());
        // Second within cooldown does not fire
        assert!(detector.check(&bl, 86.0, now + chrono::Duration::seconds(10)).is_none());
        // After cooldown fires again
        assert!(detector.check(&bl, 87.0, now + chrono::Duration::seconds(301)).is_some());
    }

    #[test]
    fn test_insufficient_samples() {
        let mut bl = test_baseline();
        bl.sample_count = 5;
        let mut detector = AnomalyDetector::with_defaults();
        // Should not detect even with extreme value
        assert!(detector.check(&bl, 200.0, Utc::now()).is_none());
    }
}
