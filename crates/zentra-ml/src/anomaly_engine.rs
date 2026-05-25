//! AnomalyEngine — orchestrates per-metric anomaly detection,
//! persists anomalies to DB, and exposes results for the API.

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::PgPool;
use tracing::{info, error};
use serde::{Deserialize, Serialize};
use super::{zscore::ZScoreDetector, ewma::EwmaDetector, seasonal::SeasonalDetector};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyResult {
    pub id: Uuid,
    pub detected_at: DateTime<Utc>,
    pub target_id: Uuid,
    pub metric_name: String,
    pub value: f64,
    pub expected_mean: f64,
    pub expected_stddev: f64,
    pub deviation_sigma: f64,
    pub detector: String,
    pub severity: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetectorConfig {
    pub zscore_window: Option<usize>,
    pub zscore_sigma: Option<f64>,
    pub ewma_alpha: Option<f64>,
    pub ewma_sigma: Option<f64>,
    pub seasonal_sigma: Option<f64>,
    pub seasonal_min_samples: Option<u64>,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            zscore_window: Some(200),
            zscore_sigma: Some(3.0),
            ewma_alpha: Some(0.1),
            ewma_sigma: Some(3.5),
            seasonal_sigma: Some(3.0),
            seasonal_min_samples: Some(14),
        }
    }
}

struct MetricState {
    zscore: ZScoreDetector,
    ewma: EwmaDetector,
    seasonal: SeasonalDetector,
    last_update: DateTime<Utc>,
}

pub struct AnomalyEngine {
    pool: PgPool,
    states: RwLock<HashMap<String, MetricState>>, // key = "target_id:metric_name"
    config: DetectorConfig,
    last_eviction: RwLock<DateTime<Utc>>,
}

impl AnomalyEngine {
    pub fn new(pool: PgPool, config: DetectorConfig) -> Arc<Self> {
        Arc::new(Self {
            pool,
            states: RwLock::new(HashMap::new()),
            config,
            last_eviction: RwLock::new(Utc::now()),
        })
    }

    /// Process a new metric value. Returns any anomalies detected.
    pub async fn process(
        self: &Arc<Self>,
        target_id: Uuid,
        metric_name: &str,
        value: f64,
        timestamp: DateTime<Utc>,
    ) -> Vec<AnomalyResult> {
        let key = format!("{}:{}", target_id, metric_name);

        // Periodic eviction: remove states not updated in 1 hour
        {
            let mut last = self.last_eviction.write();
            if (timestamp - *last).num_minutes() >= 10 {
                let mut states = self.states.write();
                states.retain(|_, s| (timestamp - s.last_update).num_hours() < 1);
                *last = timestamp;
            }
        }

        let mut anomalies = Vec::new();

        // Run detectors under write lock
        let results = {
            let mut states = self.states.write();
            let state = states.entry(key).or_insert_with(|| {
                let cfg = &self.config;
                MetricState {
                    zscore: ZScoreDetector::new(
                        cfg.zscore_window.unwrap_or(200),
                        cfg.zscore_sigma.unwrap_or(3.0),
                    ),
                    ewma: EwmaDetector::new(
                        cfg.ewma_alpha.unwrap_or(0.1),
                        cfg.ewma_sigma.unwrap_or(3.5),
                        30,
                    ),
                    seasonal: SeasonalDetector::new(
                        cfg.seasonal_sigma.unwrap_or(3.0),
                        cfg.seasonal_min_samples.unwrap_or(14),
                    ),
                    last_update: timestamp,
                }
            });
            state.last_update = timestamp;

            let (z_anom, z_score, z_mean, z_std) = state.zscore.update(value);
            let (e_anom, e_sigma, e_mean, e_std) = state.ewma.update(value);
            let (s_anom, s_sigma, s_mean, s_std, _) = state.seasonal.update(value, timestamp.timestamp());

            [
                (z_anom, z_score, z_mean, z_std, "zscore"),
                (e_anom, e_sigma, e_mean, e_std, "ewma"),
                (s_anom, s_sigma, s_mean, s_std, "seasonal"),
            ]
        };

        for (is_anom, sigma, mean, stddev, detector) in results {
            if !is_anom { continue; }

            let severity = if sigma > 5.0 { "critical" } else if sigma > 4.0 { "warning" } else { "info" };
            let result = AnomalyResult {
                id: Uuid::new_v4(),
                detected_at: timestamp,
                target_id,
                metric_name: metric_name.to_string(),
                value,
                expected_mean: mean,
                expected_stddev: stddev,
                deviation_sigma: sigma,
                detector: detector.to_string(),
                severity: severity.to_string(),
            };

            if let Err(e) = self.persist_anomaly(&result).await {
                error!(error = %e, metric = metric_name, "Failed to persist anomaly");
            } else {
                info!(
                    metric = metric_name, target = %target_id,
                    sigma = sigma, detector = detector,
                    "Anomaly detected"
                );
            }

            anomalies.push(result);
        }

        anomalies
    }

    async fn persist_anomaly(&self, a: &AnomalyResult) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            INSERT INTO anomaly_detections
                (id, detected_at, target_id, metric_name, value, expected_mean, expected_stddev,
                 deviation_sigma, detector, severity)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            a.id, a.detected_at, a.target_id, a.metric_name,
            a.value, a.expected_mean, a.expected_stddev,
            a.deviation_sigma, a.detector, a.severity,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load seasonal baselines from DB to warm up detectors on restart
    pub async fn load_seasonal_baselines(&self, target_id: Uuid, metric_name: &str) {
        let key = format!("{}:{}", target_id, metric_name);

        let rows = match sqlx::query!(
            r#"
            SELECT bucket_key, mean, stddev, sample_count
            FROM anomaly_seasonal_baselines
            WHERE target_id = $1 AND metric_name = $2
            "#,
            target_id, metric_name,
        )
        .fetch_all(&self.pool)
        .await {
            Ok(r) => r,
            Err(_) => return,
        };

        if rows.is_empty() { return; }

        let mut states = self.states.write();
        let state = states.entry(key).or_insert_with(|| {
            let cfg = &self.config;
            MetricState {
                zscore: ZScoreDetector::new(cfg.zscore_window.unwrap_or(200), cfg.zscore_sigma.unwrap_or(3.0)),
                ewma: EwmaDetector::new(cfg.ewma_alpha.unwrap_or(0.1), cfg.ewma_sigma.unwrap_or(3.5), 30),
                seasonal: SeasonalDetector::new(cfg.seasonal_sigma.unwrap_or(3.0), cfg.seasonal_min_samples.unwrap_or(14)),
                last_update: Utc::now(),
            }
        });

        for row in rows {
            state.seasonal.load_baseline(
                row.bucket_key as u32,
                row.mean,
                row.stddev,
                row.sample_count as u64,
            );
        }
    }
}
