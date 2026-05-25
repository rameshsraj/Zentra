use std::collections::HashMap;

use chrono::{DateTime, Utc};
use tracing::{info, debug, warn};
use uuid::Uuid;

use zentra_core::models::alert::{
    Alert, AlertCondition, AlertRule, AlertSeverity, AlertStatus,
    ComparisonOp, Aggregation,
};
use zentra_core::models::event::Event;
use zentra_core::models::metric::Metric;

use zentra_predict::{
    BaselineEngine, AnomalyDetector, AnomalyEvent, AnomalySeverity,
    ForecastEngine,
    TrendAlertEngine, TrendAlert,
};

use crate::EventBus;

/// Sliding window of metric values for alert evaluation
#[derive(Debug, Clone)]
struct MetricWindow {
    values: Vec<(DateTime<Utc>, f64)>,
    window_seconds: u32,
}

impl MetricWindow {
    fn new(window_seconds: u32) -> Self {
        Self {
            values: Vec::new(),
            window_seconds,
        }
    }

    fn push(&mut self, ts: DateTime<Utc>, value: f64) {
        self.values.push((ts, value));
        self.trim(ts);
    }

    fn trim(&mut self, now: DateTime<Utc>) {
        let cutoff = now - chrono::Duration::seconds(self.window_seconds as i64);
        self.values.retain(|(ts, _)| *ts >= cutoff);
    }

    fn aggregate(&self, agg: Aggregation) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let vals: Vec<f64> = self.values.iter().map(|(_, v)| *v).collect();
        match agg {
            Aggregation::Avg => Some(vals.iter().sum::<f64>() / vals.len() as f64),
            Aggregation::Max => vals.iter().cloned().reduce(f64::max),
            Aggregation::Min => vals.iter().cloned().reduce(f64::min),
            Aggregation::Sum => Some(vals.iter().sum()),
            Aggregation::Count => Some(vals.len() as f64),
            Aggregation::Last => vals.last().cloned(),
            Aggregation::P95 => percentile(&vals, 95.0),
            Aggregation::P99 => percentile(&vals, 99.0),
        }
    }
}

fn percentile(values: &[f64], pct: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    Some(sorted[idx.min(sorted.len() - 1)])
}

fn evaluate_condition(value: f64, condition: &AlertCondition) -> bool {
    match condition.operator {
        ComparisonOp::GreaterThan => value > condition.threshold,
        ComparisonOp::GreaterThanOrEqual => value >= condition.threshold,
        ComparisonOp::LessThan => value < condition.threshold,
        ComparisonOp::LessThanOrEqual => value <= condition.threshold,
        ComparisonOp::Equal => (value - condition.threshold).abs() < f64::EPSILON,
        ComparisonOp::NotEqual => (value - condition.threshold).abs() >= f64::EPSILON,
    }
}

/// Check if value has crossed the hysteresis recovery threshold.
/// For GreaterThan alerts (threshold=90, hysteresis=80), the alert only
/// resolves when value drops below 80, not just below 90.
fn evaluate_hysteresis_recovery(value: f64, recovery: f64, condition: &AlertCondition) -> bool {
    match condition.operator {
        ComparisonOp::GreaterThan | ComparisonOp::GreaterThanOrEqual => value < recovery,
        ComparisonOp::LessThan | ComparisonOp::LessThanOrEqual => value > recovery,
        _ => true, // equality ops don't use hysteresis
    }
}

/// The alert evaluation engine. Processes metrics against alert rules
/// and publishes alerts to the event bus.
///
/// Also integrates the Zentra predictive alerting pipeline:
/// - **Baseline**: rolling mean/stddev per metric (7-day sliding window)
/// - **Anomaly Detector**: z-score deviation detection (> 3σ)
/// - **Forecast**: linear regression capacity forecasting
/// - **Trend Alert**: generates predictive alerts from forecasts + anomalies
pub struct AlertEngine {
    rules: Vec<AlertRule>,
    /// Sliding windows keyed by (rule_id, resource_id)
    windows: HashMap<(Uuid, String), MetricWindow>,
    /// Active (unresolved) alerts keyed by (rule_id, resource_id)
    active_alerts: HashMap<(Uuid, String), Alert>,
    /// Event bus for publishing alert events
    event_bus: EventBus,
    // ---- Predictive alerting engines ----
    /// Baseline engine — rolling statistical baselines per metric
    baseline_engine: BaselineEngine,
    /// Anomaly detector — z-score based anomaly detection
    anomaly_detector: AnomalyDetector,
    /// Forecast engine — linear regression capacity forecasting
    forecast_engine: ForecastEngine,
    /// Trend alert engine — generates predictive alerts from forecasts/anomalies
    trend_alert_engine: TrendAlertEngine,
    /// Whether predictive alerting is enabled
    predictive_enabled: bool,
    /// Counter for metrics processed through predictive pipeline
    predictive_metrics_processed: u64,
}

impl AlertEngine {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            rules: Vec::new(),
            windows: HashMap::new(),
            active_alerts: HashMap::new(),
            event_bus,
            baseline_engine: BaselineEngine::default(),
            anomaly_detector: AnomalyDetector::default(),
            forecast_engine: ForecastEngine::default(),
            trend_alert_engine: TrendAlertEngine::default(),
            predictive_enabled: true,
            predictive_metrics_processed: 0,
        }
    }

    /// Create with custom predictive engine configurations
    pub fn with_predict(
        event_bus: EventBus,
        baseline: BaselineEngine,
        anomaly: AnomalyDetector,
        forecast: ForecastEngine,
        trend: TrendAlertEngine,
    ) -> Self {
        Self {
            rules: Vec::new(),
            windows: HashMap::new(),
            active_alerts: HashMap::new(),
            event_bus,
            baseline_engine: baseline,
            anomaly_detector: anomaly,
            forecast_engine: forecast,
            trend_alert_engine: trend,
            predictive_enabled: true,
            predictive_metrics_processed: 0,
        }
    }

    /// Enable or disable predictive alerting
    pub fn set_predictive_enabled(&mut self, enabled: bool) {
        self.predictive_enabled = enabled;
        info!(enabled, "Predictive alerting toggled");
    }

    /// Load or reload alert rules
    pub fn load_rules(&mut self, rules: Vec<AlertRule>) {
        let enabled = rules.iter().filter(|r| r.enabled).count();
        info!(total = rules.len(), enabled, "Alert rules loaded");
        self.rules = rules;
    }

    /// Process a batch of metrics and evaluate against all rules.
    /// Supports the full state machine: Pending → Triggered → Resolved
    /// with hysteresis and repeat notification tracking.
    pub fn evaluate_metrics(&mut self, metrics: &[Metric]) {
        let now = Utc::now();

        // Clone rules to avoid borrow issues
        let rules = self.rules.clone();

        for metric in metrics {
            for rule in &rules {
                if !rule.enabled || rule.metric_name != metric.metric_name {
                    continue;
                }

                let key = (rule.rule_id, metric.resource_id.clone());

                // Push value into sliding window
                let window = self.windows
                    .entry(key.clone())
                    .or_insert_with(|| MetricWindow::new(rule.window_seconds));
                window.push(now, metric.value);

                // Compute aggregate
                let agg_value = match window.aggregate(rule.condition.aggregation) {
                    Some(v) => v,
                    None => continue,
                };

                let condition_met = evaluate_condition(agg_value, &rule.condition);

                if condition_met {
                    if let Some(alert) = self.active_alerts.get_mut(&key) {
                        // Already have an alert for this key
                        alert.metric_value = agg_value;

                        // Promote Pending → Triggered if for_duration elapsed
                        if alert.status == AlertStatus::Pending
                            && alert.should_promote(rule.for_duration_seconds)
                        {
                            alert.promote_to_triggered();
                            info!(
                                rule = %rule.name,
                                resource = %metric.resource_id,
                                "Alert promoted: Pending -> Triggered"
                            );
                            self.event_bus.publish(Event::AlertTriggered {
                                alert_id: alert.alert_id,
                                rule_id: alert.rule_id,
                                severity: alert.severity.as_str().to_string(),
                                timestamp: now,
                            });
                        }

                        // Check if repeat notification is due
                        if alert.status == AlertStatus::Triggered
                            && alert.should_repeat_notify(rule.repeat_interval_seconds)
                        {
                            alert.mark_notified();
                            debug!(
                                rule = %rule.name,
                                resource = %metric.resource_id,
                                count = alert.notification_count,
                                "Repeat notification sent"
                            );
                        }
                    } else {
                        // New alert
                        let alert = Alert::trigger(
                            rule,
                            metric.target_id,
                            &metric.resource_id,
                            agg_value,
                            format!(
                                "{}: {} {} ({:?} = {:.2}, threshold = {:.2})",
                                rule.name,
                                metric.metric_name,
                                rule.condition.operator_str(),
                                rule.condition.aggregation,
                                agg_value,
                                rule.condition.threshold,
                            ),
                        );

                        let state_str = if alert.status == AlertStatus::Pending {
                            "pending"
                        } else {
                            "triggered"
                        };

                        info!(
                            rule = %rule.name,
                            severity = %rule.severity.as_str(),
                            resource = %metric.resource_id,
                            value = agg_value,
                            threshold = rule.condition.threshold,
                            state = state_str,
                            "Alert created"
                        );

                        // Only publish AlertTriggered if it skips pending
                        if alert.status == AlertStatus::Triggered {
                            self.event_bus.publish(Event::AlertTriggered {
                                alert_id: alert.alert_id,
                                rule_id: alert.rule_id,
                                severity: alert.severity.as_str().to_string(),
                                timestamp: now,
                            });
                        }

                        self.active_alerts.insert(key, alert);
                    }
                } else {
                    // Condition NOT met — check for resolution
                    if let Some(alert) = self.active_alerts.get_mut(&key) {
                        // If still pending and condition dropped, just remove it
                        if alert.status == AlertStatus::Pending {
                            debug!(
                                rule = %rule.name,
                                resource = %metric.resource_id,
                                "Pending alert cleared (condition no longer met)"
                            );
                            self.active_alerts.remove(&key);
                            continue;
                        }

                        // Hysteresis check: if a recovery threshold is set,
                        // only resolve when value crosses past the recovery point
                        let should_resolve = if let Some(hysteresis) = rule.hysteresis_threshold {
                            evaluate_hysteresis_recovery(
                                agg_value,
                                hysteresis,
                                &rule.condition,
                            )
                        } else {
                            true // no hysteresis, resolve immediately
                        };

                        if should_resolve
                            && (alert.status == AlertStatus::Triggered
                                || alert.status == AlertStatus::Acknowledged)
                        {
                            alert.resolve();
                            info!(
                                rule = %rule.name,
                                resource = %metric.resource_id,
                                "Alert auto-resolved"
                            );

                            self.event_bus.publish(Event::AlertResolved {
                                alert_id: alert.alert_id,
                                timestamp: now,
                            });
                        }
                    }
                }
            }
        }

        // Also feed into predictive pipeline for anomaly detection & forecasting
        self.evaluate_predictive(metrics);
    }

    /// Feed metrics into the predictive alerting pipeline.
    /// This updates baselines, checks for anomalies, generates forecasts,
    /// and produces trend alerts — all without requiring manual rule configuration.
    pub fn evaluate_predictive(&mut self, metrics: &[Metric]) {
        if !self.predictive_enabled || metrics.is_empty() {
            return;
        }

        let now = Utc::now();

        for metric in metrics {
            // Step 1: Feed into baseline engine (rolling mean/stddev)
            self.baseline_engine.push(
                metric.org_id,
                metric.target_id,
                &metric.metric_name,
                &metric.resource_id,
                metric.value,
                now,
            );

            // Step 2: Feed into forecast engine (linear regression)
            self.forecast_engine.push(
                metric.target_id,
                &metric.metric_name,
                &metric.resource_id,
                metric.value,
                now,
            );

            // Step 3: Check for anomalies against baselines
            if let Some(baseline) = self.baseline_engine.get_baseline(
                metric.target_id,
                &metric.metric_name,
                &metric.resource_id,
            ) {
                if let Some(anomaly) = self.anomaly_detector.check(&baseline, metric.value, now) {
                    let severity = match anomaly.severity {
                        AnomalySeverity::Critical => AlertSeverity::Critical,
                        AnomalySeverity::High => AlertSeverity::Critical,
                        AnomalySeverity::Medium => AlertSeverity::Warning,
                        AnomalySeverity::Low => AlertSeverity::Info,
                    };

                    info!(
                        metric = %metric.metric_name,
                        resource = %metric.resource_id,
                        z_score = anomaly.z_score,
                        severity = severity.as_str(),
                        "Anomaly detected by predictive engine"
                    );

                    self.event_bus.publish(Event::AnomalyDetected {
                        metric_name: metric.metric_name.clone(),
                        resource_id: metric.resource_id.clone(),
                        target_id: metric.target_id,
                        z_score: anomaly.z_score,
                        severity: severity.as_str().to_string(),
                        timestamp: now,
                    });
                }
            }

            self.predictive_metrics_processed += 1;
        }

        // Step 4: Generate forecasts periodically (every 1000 metrics batch)
        if self.predictive_metrics_processed % 1000 == 0 && !metrics.is_empty() {
            let org_id = metrics[0].org_id;
            let forecasts = self.forecast_engine.generate_forecasts(org_id);

            if !forecasts.is_empty() {
                debug!(count = forecasts.len(), "Generated capacity forecasts");

                // Step 5: Generate trend alerts from forecasts
                let trend_alerts = self.trend_alert_engine.evaluate_forecasts(&forecasts);
                for alert in &trend_alerts {
                    warn!(
                        metric = %alert.metric_name,
                        hours_to_breach = alert.hours_to_threshold().unwrap_or(0.0),
                        severity = %alert.severity_str(),
                        recommendation = %alert.recommendation(),
                        "Trend alert generated"
                    );

                    self.event_bus.publish(Event::TrendAlertGenerated {
                        metric_name: alert.metric_name.clone(),
                        resource_id: alert.resource_id.clone(),
                        target_id: alert.target_id,
                        hours_to_threshold: alert.hours_to_threshold(),
                        severity: alert.severity_str().to_string(),
                        description: alert.description.clone(),
                        timestamp: now,
                    });
                }
            }
        }
    }

    /// Get recent anomaly events from the detector (last hour)
    pub fn recent_anomalies(&self) -> Vec<&AnomalyEvent> {
        self.anomaly_detector.recent_events(3600)
    }

    /// Get recent trend alerts
    pub fn recent_trend_alerts(&self) -> &[TrendAlert] {
        self.trend_alert_engine.recent_alerts()
    }

    /// Get predictive engine statistics
    pub fn predictive_stats(&self) -> PredictiveStats {
        PredictiveStats {
            baselines_tracked: self.baseline_engine.baseline_count(),
            anomalies_detected: self.anomaly_detector.total_count(),
            forecasts_generated: self.forecast_engine.forecast_count(),
            trend_alerts_active: self.trend_alert_engine.active_count(),
            metrics_processed: self.predictive_metrics_processed,
            enabled: self.predictive_enabled,
        }
    }

    /// Get all active (unresolved) alerts including pending
    pub fn active_alerts(&self) -> Vec<&Alert> {
        self.active_alerts
            .values()
            .filter(|a| {
                a.status == AlertStatus::Pending
                    || a.status == AlertStatus::Triggered
                    || a.status == AlertStatus::Acknowledged
            })
            .collect()
    }

    /// Acknowledge an alert by ID
    pub fn acknowledge_alert(&mut self, alert_id: Uuid, by: &str) -> bool {
        for alert in self.active_alerts.values_mut() {
            if alert.alert_id == alert_id {
                alert.acknowledge(by);
                return true;
            }
        }
        false
    }

    /// Manually resolve an alert by ID
    pub fn resolve_alert(&mut self, alert_id: Uuid) -> bool {
        for (_key, alert) in self.active_alerts.iter_mut() {
            if alert.alert_id == alert_id {
                alert.resolve();
                self.event_bus.publish(Event::AlertResolved {
                    alert_id,
                    timestamp: Utc::now(),
                });
                return true;
            }
        }
        false
    }

    /// Get alert count by severity
    pub fn alert_counts(&self) -> HashMap<AlertSeverity, usize> {
        let mut counts = HashMap::new();
        for alert in self.active_alerts.values() {
            if alert.status == AlertStatus::Triggered {
                *counts.entry(alert.severity).or_default() += 1;
            }
        }
        counts
    }

    /// Clean up resolved alerts older than the given duration
    pub fn cleanup_resolved(&mut self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        self.active_alerts.retain(|_, alert| {
            if alert.status == AlertStatus::Resolved {
                alert.resolved_at.map_or(true, |t| t > cutoff)
            } else {
                true
            }
        });
    }
}

/// Statistics from the predictive alerting engines.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PredictiveStats {
    pub baselines_tracked: usize,
    pub anomalies_detected: usize,
    pub forecasts_generated: usize,
    pub trend_alerts_active: usize,
    pub metrics_processed: u64,
    pub enabled: bool,
}

/// Extension trait to get human-readable operator string
trait ConditionExt {
    fn operator_str(&self) -> &'static str;
}

impl ConditionExt for AlertCondition {
    fn operator_str(&self) -> &'static str {
        match self.operator {
            ComparisonOp::GreaterThan => ">",
            ComparisonOp::GreaterThanOrEqual => ">=",
            ComparisonOp::LessThan => "<",
            ComparisonOp::LessThanOrEqual => "<=",
            ComparisonOp::Equal => "==",
            ComparisonOp::NotEqual => "!=",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zentra_core::models::alert::AlertCondition;

    fn make_rule(name: &str, metric: &str, op: ComparisonOp, threshold: f64, sev: AlertSeverity) -> AlertRule {
        AlertRule {
            rule_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            metric_name: metric.into(),
            condition: AlertCondition {
                operator: op,
                threshold,
                aggregation: Aggregation::Last,
            },
            window_seconds: 300,
            severity: sev,
            enabled: true,
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            for_duration_seconds: 0,
            hysteresis_threshold: None,
            repeat_interval_seconds: 3600,
            notification_policy_id: None,
        }
    }

    fn make_metric(name: &str, value: f64) -> Metric {
        Metric::new(name, value, "test-resource")
            .with_context(Uuid::new_v4(), Uuid::new_v4(), "test-plugin")
    }

    #[tokio::test]
    async fn test_alert_trigger() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let mut engine = AlertEngine::new(bus);

        let rule = make_rule("High CPU", "cpu_usage", ComparisonOp::GreaterThan, 90.0, AlertSeverity::Critical);
        engine.load_rules(vec![rule]);

        // Below threshold — no alert
        engine.evaluate_metrics(&[make_metric("cpu_usage", 80.0)]);
        assert!(engine.active_alerts().is_empty());

        // Above threshold — triggers alert
        engine.evaluate_metrics(&[make_metric("cpu_usage", 95.0)]);
        assert_eq!(engine.active_alerts().len(), 1);
        assert_eq!(engine.active_alerts()[0].severity, AlertSeverity::Critical);

        // Verify event was published
        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type(), "alert_triggered");
    }

    #[tokio::test]
    async fn test_alert_auto_resolve() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let mut engine = AlertEngine::new(bus);

        let rule = make_rule("High CPU", "cpu_usage", ComparisonOp::GreaterThan, 90.0, AlertSeverity::Warning);
        engine.load_rules(vec![rule]);

        // Trigger
        engine.evaluate_metrics(&[make_metric("cpu_usage", 95.0)]);
        assert_eq!(engine.active_alerts().len(), 1);
        let _trigger_event = rx.recv().await.unwrap();

        // Goes below threshold — auto-resolve
        engine.evaluate_metrics(&[make_metric("cpu_usage", 70.0)]);
        let resolve_event = rx.recv().await.unwrap();
        assert_eq!(resolve_event.event_type(), "alert_resolved");
    }

    #[tokio::test]
    async fn test_no_duplicate_alerts() {
        let bus = EventBus::new();
        let mut engine = AlertEngine::new(bus);

        let rule = make_rule("High CPU", "cpu_usage", ComparisonOp::GreaterThan, 90.0, AlertSeverity::Critical);
        engine.load_rules(vec![rule]);

        engine.evaluate_metrics(&[make_metric("cpu_usage", 95.0)]);
        engine.evaluate_metrics(&[make_metric("cpu_usage", 96.0)]);
        engine.evaluate_metrics(&[make_metric("cpu_usage", 97.0)]);

        // Should still only have 1 active alert
        assert_eq!(engine.active_alerts().len(), 1);
    }

    #[test]
    fn test_percentile() {
        let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(percentile(&vals, 50.0), Some(6.0)); // idx = round(0.5 * 9) = 5 → 6.0
        assert_eq!(percentile(&vals, 95.0), Some(10.0));
    }
}
