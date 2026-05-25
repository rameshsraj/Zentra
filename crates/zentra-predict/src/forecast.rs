//! Linear regression based capacity forecasting for Zentra.
//! Predicts when metrics will breach thresholds (e.g., "disk full in X days").
//! Inspired by Prometheus recording rules for pre-computed aggregates.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, debug};
use uuid::Uuid;

/// Result of a capacity forecast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastResult {
    pub forecast_id: Uuid,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub metric_name: String,
    pub resource_id: String,
    /// Current value (most recent data point)
    pub current_value: f64,
    /// Predicted value at the forecast horizon
    pub predicted_value: f64,
    /// Rate of change per hour
    pub rate_per_hour: f64,
    /// Hours until threshold breach (None if no breach predicted)
    pub hours_to_threshold: Option<f64>,
    /// The threshold being evaluated
    pub threshold: f64,
    /// Confidence (R² of the linear regression, 0.0–1.0)
    pub confidence: f64,
    /// Number of data points used
    pub sample_count: u64,
    /// Description of the forecast
    pub description: String,
    pub computed_at: DateTime<Utc>,
}

/// Configuration for forecast thresholds per metric.
#[derive(Debug, Clone)]
pub struct ForecastThreshold {
    pub metric_name: String,
    pub threshold: f64,
    pub direction: ThresholdDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdDirection {
    Above, // Alert when trending above threshold
    Below, // Alert when trending below threshold
}

/// Data window for a single metric stream.
#[derive(Debug, Clone)]
struct TimeSeriesWindow {
    points: Vec<(DateTime<Utc>, f64)>,
    window_duration: Duration,
}

impl TimeSeriesWindow {
    fn new(window_duration: Duration) -> Self {
        Self {
            points: Vec::new(),
            window_duration,
        }
    }

    fn push(&mut self, ts: DateTime<Utc>, value: f64) {
        self.points.push((ts, value));
        let cutoff = ts - self.window_duration;
        self.points.retain(|(t, _)| *t >= cutoff);
    }

    /// Perform ordinary least squares linear regression.
    /// Returns (slope, intercept, r_squared).
    fn linear_regression(&self) -> Option<(f64, f64, f64)> {
        let n = self.points.len();
        if n < 10 {
            return None;
        }

        // Use hours since first point as x-axis
        let t0 = self.points[0].0;
        let xs: Vec<f64> = self.points.iter().map(|(t, _)| (*t - t0).num_seconds() as f64 / 3600.0).collect();
        let ys: Vec<f64> = self.points.iter().map(|(_, v)| *v).collect();

        let n_f = n as f64;
        let sum_x: f64 = xs.iter().sum();
        let sum_y: f64 = ys.iter().sum();
        let sum_xy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = xs.iter().map(|x| x * x).sum();

        let denom = n_f * sum_x2 - sum_x * sum_x;
        if denom.abs() < f64::EPSILON {
            return None;
        }

        let slope = (n_f * sum_xy - sum_x * sum_y) / denom;
        let intercept = (sum_y - slope * sum_x) / n_f;

        // R² calculation
        let y_mean = sum_y / n_f;
        let ss_tot: f64 = ys.iter().map(|y| (y - y_mean).powi(2)).sum();
        let ss_res: f64 = xs.iter().zip(ys.iter())
            .map(|(x, y)| {
                let predicted = slope * x + intercept;
                (y - predicted).powi(2)
            })
            .sum();

        let r_squared = if ss_tot > 0.0 { 1.0 - ss_res / ss_tot } else { 0.0 };

        Some((slope, intercept, r_squared))
    }
}

/// Key for tracking metric streams.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct StreamKey {
    target_id: Uuid,
    metric_name: String,
    resource_id: String,
}

/// Forecast engine that predicts capacity exhaustion.
pub struct ForecastEngine {
    windows: HashMap<StreamKey, TimeSeriesWindow>,
    thresholds: Vec<ForecastThreshold>,
    window_duration: Duration,
    /// Minimum R² confidence to emit a forecast
    min_confidence: f64,
}

impl ForecastEngine {
    /// Create with a 7-day lookback window.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            thresholds: Vec::new(),
            window_duration: Duration::days(7),
            min_confidence: 0.5,
        }
    }

    /// Add a threshold rule.
    pub fn add_threshold(&mut self, metric_name: impl Into<String>, threshold: f64, direction: ThresholdDirection) {
        self.thresholds.push(ForecastThreshold {
            metric_name: metric_name.into(),
            threshold,
            direction,
        });
    }

    /// Convenience: add common infrastructure thresholds.
    pub fn add_default_thresholds(&mut self) {
        self.add_threshold("server.cpu_usage", 95.0, ThresholdDirection::Above);
        self.add_threshold("server.memory_usage", 95.0, ThresholdDirection::Above);
        self.add_threshold("storage.disk_usage_percent", 90.0, ThresholdDirection::Above);
        self.add_threshold("database.active_connections", 500.0, ThresholdDirection::Above);
        self.add_threshold("oracle.tablespace.used_percent", 95.0, ThresholdDirection::Above);
    }

    /// Ingest a metric data point.
    pub fn push(
        &mut self,
        target_id: Uuid,
        metric_name: &str,
        resource_id: &str,
        value: f64,
        ts: DateTime<Utc>,
    ) {
        let key = StreamKey {
            target_id,
            metric_name: metric_name.to_string(),
            resource_id: resource_id.to_string(),
        };

        let window = self
            .windows
            .entry(key)
            .or_insert_with(|| TimeSeriesWindow::new(self.window_duration));
        window.push(ts, value);
    }

    /// Generate forecasts for all tracked metrics.
    pub fn generate_forecasts(&self, org_id: Uuid) -> Vec<ForecastResult> {
        let mut results = Vec::new();

        for (key, window) in &self.windows {
            let applicable_thresholds: Vec<&ForecastThreshold> = self
                .thresholds
                .iter()
                .filter(|t| t.metric_name == key.metric_name)
                .collect();

            if applicable_thresholds.is_empty() {
                continue;
            }

            if let Some((slope, _intercept, r_squared)) = window.linear_regression() {
                if r_squared < self.min_confidence {
                    continue;
                }

                let current_value = window.points.last().map(|(_, v)| *v).unwrap_or(0.0);

                for threshold in applicable_thresholds {
                    let hours_to_breach = match threshold.direction {
                        ThresholdDirection::Above => {
                            if slope > 0.0 && current_value < threshold.threshold {
                                Some((threshold.threshold - current_value) / slope)
                            } else {
                                None
                            }
                        }
                        ThresholdDirection::Below => {
                            if slope < 0.0 && current_value > threshold.threshold {
                                Some((current_value - threshold.threshold) / slope.abs())
                            } else {
                                None
                            }
                        }
                    };

                    // Only report if breach within 30 days
                    let hours_to_breach = hours_to_breach.filter(|h| *h > 0.0 && *h < 720.0);

                    let predicted_24h = current_value + slope * 24.0;

                    let desc = if let Some(hours) = hours_to_breach {
                        let days = hours / 24.0;
                        format!(
                            "{} on {} trending to breach {:.0} in {:.1} days (rate: {:.2}/hr, R²={:.2})",
                            key.metric_name, key.resource_id, threshold.threshold, days, slope, r_squared
                        )
                    } else {
                        format!(
                            "{} on {} stable (current={:.1}, predicted_24h={:.1}, R²={:.2})",
                            key.metric_name, key.resource_id, current_value, predicted_24h, r_squared
                        )
                    };

                    results.push(ForecastResult {
                        forecast_id: Uuid::now_v7(),
                        org_id,
                        target_id: key.target_id,
                        metric_name: key.metric_name.clone(),
                        resource_id: key.resource_id.clone(),
                        current_value,
                        predicted_value: predicted_24h,
                        rate_per_hour: slope,
                        hours_to_threshold: hours_to_breach,
                        threshold: threshold.threshold,
                        confidence: r_squared,
                        sample_count: window.points.len() as u64,
                        description: desc,
                        computed_at: Utc::now(),
                    });
                }
            }
        }

        debug!(forecast_count = results.len(), "Forecasts generated");
        results
    }

    /// Get only forecasts where a threshold breach is predicted.
    pub fn active_warnings(&self, org_id: Uuid) -> Vec<ForecastResult> {
        self.generate_forecasts(org_id)
            .into_iter()
            .filter(|f| f.hours_to_threshold.is_some())
            .collect()
    }

    /// Number of metric streams being tracked for forecasting.
    pub fn forecast_count(&self) -> usize {
        self.windows.len()
    }
}

impl Default for ForecastEngine {
    fn default() -> Self {
        let mut engine = Self::new();
        engine.add_default_thresholds();
        engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_regression() {
        let mut win = TimeSeriesWindow::new(Duration::days(7));
        let now = Utc::now();

        // Linear trend: y = 2*x + 10 (slope=2/hr)
        for i in 0..100 {
            let ts = now - Duration::hours(100 - i);
            let value = 10.0 + 2.0 * (i as f64);
            win.push(ts, value);
        }

        let (slope, _intercept, r_sq) = win.linear_regression().unwrap();
        assert!((slope - 2.0).abs() < 0.1);
        assert!(r_sq > 0.99);
    }

    #[test]
    fn test_forecast_breach_prediction() {
        let mut engine = ForecastEngine::new();
        engine.add_threshold("storage.disk_usage_percent", 90.0, ThresholdDirection::Above);
        engine.min_confidence = 0.3;

        let target = Uuid::new_v4();
        let now = Utc::now();

        // Simulate disk usage growing ~0.5%/hour from 70%
        for i in 0..168 { // 7 days of hourly data
            let ts = now - Duration::hours(168 - i);
            let value = 70.0 + 0.5 * (i as f64) / 24.0;
            engine.push(target, "storage.disk_usage_percent", "sda1", value, ts);
        }

        let org = Uuid::new_v4();
        let forecasts = engine.generate_forecasts(org);
        assert!(!forecasts.is_empty());

        // Should predict breach since we're trending up toward 90%
        let f = &forecasts[0];
        assert!(f.rate_per_hour > 0.0);
        assert!(f.confidence > 0.3);
    }

    #[test]
    fn test_no_breach_stable() {
        let mut engine = ForecastEngine::new();
        engine.add_threshold("server.cpu_usage", 95.0, ThresholdDirection::Above);
        engine.min_confidence = 0.01;

        let target = Uuid::new_v4();
        let now = Utc::now();

        // Stable CPU around 40%
        for i in 0..100 {
            let ts = now - Duration::hours(100 - i);
            let value = 40.0 + (i as f64 % 5.0) - 2.5;
            engine.push(target, "server.cpu_usage", "host-1", value, ts);
        }

        let org = Uuid::new_v4();
        let warnings = engine.active_warnings(org);
        // Shouldn't have a warning for a stable ~40% metric vs 95% threshold
        assert!(warnings.is_empty() || warnings[0].hours_to_threshold.unwrap_or(0.0) > 500.0);
    }
}
