//! Trend-based predictive alerting for Zentra.
//! Generates alerts like "CPU trending to 95% in 4 hours" by combining
//! forecasts with anomaly detection into actionable predictive alerts.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;

use crate::anomaly::{AnomalyEvent, AnomalySeverity};
use crate::forecast::ForecastResult;

/// A predictive trend alert combining forecast + anomaly signals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAlert {
    pub alert_id: Uuid,
    pub org_id: Uuid,
    pub target_id: Uuid,
    pub metric_name: String,
    pub resource_id: String,
    pub alert_type: TrendAlertType,
    pub severity: TrendAlertSeverity,
    pub title: String,
    pub description: String,
    pub current_value: f64,
    pub predicted_value: Option<f64>,
    pub threshold: Option<f64>,
    pub hours_to_breach: Option<f64>,
    pub confidence: f64,
    pub recommendations: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of trend alert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrendAlertType {
    CapacityExhaustion,
    AnomalySpike,
    DegradationTrend,
    ResourceContention,
}

/// Severity of a trend alert.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrendAlertSeverity {
    Info,
    Warning,
    Critical,
}

impl TrendAlertSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }
}

/// Engine that generates trend alerts from forecasts and anomaly events.
pub struct TrendAlertEngine {
    alerts: Vec<TrendAlert>,
}

impl TrendAlertEngine {
    pub fn new() -> Self {
        Self { alerts: Vec::new() }
    }

    /// Generate trend alerts from forecast results.
    pub fn evaluate_forecasts(&mut self, forecasts: &[ForecastResult]) -> Vec<TrendAlert> {
        let mut new_alerts = Vec::new();

        for forecast in forecasts {
            if let Some(hours) = forecast.hours_to_threshold {
                if forecast.confidence < 0.5 {
                    continue;
                }

                let severity = if hours <= 4.0 {
                    TrendAlertSeverity::Critical
                } else if hours <= 24.0 {
                    TrendAlertSeverity::Warning
                } else {
                    TrendAlertSeverity::Info
                };

                let days = hours / 24.0;
                let title = format!(
                    "{} on {} approaching threshold",
                    forecast.metric_name, forecast.resource_id,
                );

                let mut recommendations = Vec::new();
                if forecast.metric_name.contains("cpu") {
                    recommendations.push("Scale up CPU cores or optimize high-CPU processes".into());
                    recommendations.push("Review running workloads for inefficient queries or loops".into());
                } else if forecast.metric_name.contains("memory") {
                    recommendations.push("Add RAM or restart memory-leaking services".into());
                    recommendations.push("Enable swap or increase swap size as temporary measure".into());
                } else if forecast.metric_name.contains("disk") || forecast.metric_name.contains("storage") {
                    recommendations.push("Extend disk or add storage volume".into());
                    recommendations.push("Clean up old logs, temp files, or unused data".into());
                    recommendations.push("Enable compression on eligible datasets".into());
                } else if forecast.metric_name.contains("connection") {
                    recommendations.push("Increase connection pool size".into());
                    recommendations.push("Review application for connection leaks".into());
                } else if forecast.metric_name.contains("tablespace") {
                    recommendations.push("Enable autoextend on tablespace data files".into());
                    recommendations.push("Add new data files to the tablespace".into());
                    recommendations.push("Purge old table partitions or archive data".into());
                }

                let alert = TrendAlert {
                    alert_id: Uuid::now_v7(),
                    org_id: forecast.org_id,
                    target_id: forecast.target_id,
                    metric_name: forecast.metric_name.clone(),
                    resource_id: forecast.resource_id.clone(),
                    alert_type: TrendAlertType::CapacityExhaustion,
                    severity,
                    title,
                    description: format!(
                        "{} is trending to reach {:.0} within {:.1} days. \
                        Current: {:.1}, Rate: {:.3}/hr, Confidence: {:.0}%",
                        forecast.metric_name, forecast.threshold, days,
                        forecast.current_value, forecast.rate_per_hour, forecast.confidence * 100.0,
                    ),
                    current_value: forecast.current_value,
                    predicted_value: Some(forecast.predicted_value),
                    threshold: Some(forecast.threshold),
                    hours_to_breach: Some(hours),
                    confidence: forecast.confidence,
                    recommendations,
                    created_at: Utc::now(),
                };

                info!(
                    metric = %alert.metric_name,
                    severity = %severity.as_str(),
                    hours = hours,
                    "Trend alert generated"
                );

                new_alerts.push(alert.clone());
                self.alerts.push(alert);
            }
        }

        new_alerts
    }

    /// Generate trend alerts from anomaly events.
    pub fn evaluate_anomalies(&mut self, anomalies: &[AnomalyEvent]) -> Vec<TrendAlert> {
        let mut new_alerts = Vec::new();

        for anomaly in anomalies {
            let severity = match anomaly.severity {
                AnomalySeverity::Critical | AnomalySeverity::High => TrendAlertSeverity::Critical,
                AnomalySeverity::Medium => TrendAlertSeverity::Warning,
                AnomalySeverity::Low => TrendAlertSeverity::Info,
            };

            let alert = TrendAlert {
                alert_id: Uuid::now_v7(),
                org_id: anomaly.org_id,
                target_id: anomaly.target_id,
                metric_name: anomaly.metric_name.clone(),
                resource_id: anomaly.resource_id.clone(),
                alert_type: TrendAlertType::AnomalySpike,
                severity,
                title: format!(
                    "Anomaly: {} on {}",
                    anomaly.metric_name, anomaly.resource_id,
                ),
                description: anomaly.description.clone(),
                current_value: anomaly.observed_value,
                predicted_value: None,
                threshold: None,
                hours_to_breach: None,
                confidence: 1.0 - (1.0 / anomaly.z_score.abs()),
                recommendations: vec![
                    "Investigate recent changes to the affected resource".into(),
                    "Check for correlated anomalies on related metrics".into(),
                ],
                created_at: Utc::now(),
            };

            new_alerts.push(alert.clone());
            self.alerts.push(alert);
        }

        new_alerts
    }

    /// Get all active trend alerts.
    pub fn alerts(&self) -> &[TrendAlert] {
        &self.alerts
    }

    /// Get alerts filtered by severity.
    pub fn alerts_by_severity(&self, severity: TrendAlertSeverity) -> Vec<&TrendAlert> {
        self.alerts.iter().filter(|a| a.severity == severity).collect()
    }

    /// Clean up old alerts.
    pub fn cleanup(&mut self, max_age: chrono::Duration) {
        let cutoff = Utc::now() - max_age;
        self.alerts.retain(|a| a.created_at >= cutoff);
    }

    /// Number of currently tracked trend alerts.
    pub fn active_count(&self) -> usize {
        self.alerts.len()
    }

    /// Get recent alerts (within the last N seconds).
    pub fn recent_alerts(&self) -> &[TrendAlert] {
        &self.alerts
    }
}

impl Default for TrendAlertEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TrendAlert {
    /// Get the severity as a string.
    pub fn severity_str(&self) -> &'static str {
        self.severity.as_str()
    }

    /// Get the first recommendation, or a default message.
    pub fn recommendation(&self) -> String {
        self.recommendations.first()
            .cloned()
            .unwrap_or_else(|| "Investigate the affected resource".into())
    }

    /// Alias for hours_to_breach for API compatibility.
    pub fn hours_to_threshold(&self) -> Option<f64> {
        self.hours_to_breach
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forecast::ForecastResult;

    #[test]
    fn test_trend_alert_from_forecast() {
        let mut engine = TrendAlertEngine::new();

        let forecast = ForecastResult {
            forecast_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            metric_name: "storage.disk_usage_percent".into(),
            resource_id: "sda1".into(),
            current_value: 82.0,
            predicted_value: 94.0,
            rate_per_hour: 0.5,
            hours_to_threshold: Some(16.0),
            threshold: 90.0,
            confidence: 0.85,
            sample_count: 1000,
            description: String::new(),
            computed_at: Utc::now(),
        };

        let alerts = engine.evaluate_forecasts(&[forecast]);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].severity, TrendAlertSeverity::Warning);
        assert_eq!(alerts[0].alert_type, TrendAlertType::CapacityExhaustion);
        assert!(!alerts[0].recommendations.is_empty());
    }

    #[test]
    fn test_critical_forecast_alert() {
        let mut engine = TrendAlertEngine::new();

        let forecast = ForecastResult {
            forecast_id: Uuid::new_v4(),
            org_id: Uuid::new_v4(),
            target_id: Uuid::new_v4(),
            metric_name: "server.cpu_usage".into(),
            resource_id: "host-prod-01".into(),
            current_value: 88.0,
            predicted_value: 97.0,
            rate_per_hour: 2.0,
            hours_to_threshold: Some(3.5),
            threshold: 95.0,
            confidence: 0.92,
            sample_count: 500,
            description: String::new(),
            computed_at: Utc::now(),
        };

        let alerts = engine.evaluate_forecasts(&[forecast]);
        assert_eq!(alerts[0].severity, TrendAlertSeverity::Critical);
    }
}
