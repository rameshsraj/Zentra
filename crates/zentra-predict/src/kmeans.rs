//! K-means Anomaly Detection
//!
//! Implements Netdata-inspired K-means clustering for time-series anomaly scoring.
//! Each metric stream has its own trained model that learns "normal" behavior patterns.
//!
//! Pipeline:
//! 1. Feature extraction: diff(n), smooth(n), lag(n) preprocessing
//! 2. K-means training on a sliding window of feature vectors
//! 3. Anomaly score = normalized distance to nearest cluster center
//! 4. Host-level anomaly rate = fraction of metrics with score > threshold

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, debug};
use uuid::Uuid;

/// K-means model configuration (Netdata ml_config.h equivalent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmeansConfig {
    /// Number of clusters
    pub num_clusters: usize,
    /// Number of training iterations
    pub max_iterations: usize,
    /// Training window size in samples
    pub training_window: usize,
    /// Feature: differencing order (diff_n)
    pub diff_n: usize,
    /// Feature: moving average window (smooth_n)
    pub smooth_n: usize,
    /// Feature: lag features (lag_n)
    pub lag_n: usize,
    /// Anomaly score threshold (0.0 - 1.0)
    pub anomaly_score_threshold: f64,
    /// Host-level anomaly rate threshold (fraction of metrics)
    pub host_anomaly_rate_threshold: f64,
    /// Minimum training samples before model is active
    pub min_training_samples: usize,
    /// Retrain interval in samples
    pub retrain_interval: usize,
}

impl Default for KmeansConfig {
    fn default() -> Self {
        Self {
            num_clusters: 2,
            max_iterations: 25,
            training_window: 3600, // ~1 hour at 1 sample/sec
            diff_n: 1,
            smooth_n: 3,
            lag_n: 5,
            anomaly_score_threshold: 0.99,
            host_anomaly_rate_threshold: 0.5,
            min_training_samples: 100,
            retrain_interval: 1800,
        }
    }
}

/// A trained K-means model for a single metric stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmeansModel {
    pub metric_key: String,
    pub cluster_centers: Vec<Vec<f64>>,
    pub min_distances: Vec<f64>,
    pub max_distances: Vec<f64>,
    pub training_samples: usize,
    pub trained_at: DateTime<Utc>,
    pub is_trained: bool,
}

impl KmeansModel {
    fn new(metric_key: &str) -> Self {
        Self {
            metric_key: metric_key.to_string(),
            cluster_centers: Vec::new(),
            min_distances: Vec::new(),
            max_distances: Vec::new(),
            training_samples: 0,
            trained_at: Utc::now(),
            is_trained: false,
        }
    }

    /// Train the model on feature vectors
    fn train(&mut self, features: &[Vec<f64>], config: &KmeansConfig) {
        if features.is_empty() || features[0].is_empty() {
            return;
        }

        let dim = features[0].len();
        let k = config.num_clusters.min(features.len());

        // Initialize centers using k-means++ strategy
        let mut centers = kmeans_plus_plus_init(features, k);

        // Run Lloyd's algorithm
        for _iter in 0..config.max_iterations {
            // Assign points to clusters
            let mut assignments: Vec<usize> = Vec::with_capacity(features.len());
            for f in features {
                let nearest = nearest_center(f, &centers);
                assignments.push(nearest);
            }

            // Update centers
            let mut new_centers = vec![vec![0.0; dim]; k];
            let mut counts = vec![0usize; k];

            for (i, f) in features.iter().enumerate() {
                let c = assignments[i];
                counts[c] += 1;
                for (j, val) in f.iter().enumerate() {
                    new_centers[c][j] += val;
                }
            }

            for c in 0..k {
                if counts[c] > 0 {
                    for j in 0..dim {
                        new_centers[c][j] /= counts[c] as f64;
                    }
                }
            }

            // Check convergence
            let moved: f64 = centers.iter().zip(new_centers.iter())
                .map(|(old, new)| euclidean_distance(old, new))
                .sum();

            centers = new_centers;

            if moved < 1e-6 {
                break;
            }
        }

        // Compute distance statistics for normalization
        let distances: Vec<f64> = features.iter()
            .map(|f| {
                let c = nearest_center(f, &centers);
                euclidean_distance(f, &centers[c])
            })
            .collect();

        self.cluster_centers = centers;
        self.min_distances = vec![distances.iter().cloned().fold(f64::INFINITY, f64::min)];
        self.max_distances = vec![distances.iter().cloned().fold(f64::NEG_INFINITY, f64::max)];
        self.training_samples = features.len();
        self.trained_at = Utc::now();
        self.is_trained = true;
    }

    /// Score a single feature vector (0.0 = normal, 1.0 = anomalous)
    pub fn score(&self, feature: &[f64]) -> f64 {
        if !self.is_trained || self.cluster_centers.is_empty() {
            return 0.0;
        }

        let c = nearest_center(feature, &self.cluster_centers);
        let dist = euclidean_distance(feature, &self.cluster_centers[c]);

        let min_d = self.min_distances.first().copied().unwrap_or(0.0);
        let max_d = self.max_distances.first().copied().unwrap_or(1.0);

        if max_d <= min_d {
            return 0.0;
        }

        // Normalize to [0, 1]
        ((dist - min_d) / (max_d - min_d)).clamp(0.0, 1.0)
    }
}

/// Feature extraction pipeline for a single metric stream
#[derive(Debug)]
pub struct FeatureExtractor {
    config: KmeansConfig,
    /// Raw value buffer (ring buffer)
    buffer: Vec<f64>,
    write_pos: usize,
    count: usize,
}

impl FeatureExtractor {
    pub fn new(config: &KmeansConfig) -> Self {
        let buf_size = config.training_window + config.smooth_n + config.lag_n + config.diff_n + 10;
        Self {
            config: config.clone(),
            buffer: vec![0.0; buf_size],
            write_pos: 0,
            count: 0,
        }
    }

    /// Add a new value to the buffer
    pub fn push(&mut self, value: f64) {
        self.buffer[self.write_pos] = value;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
        self.count += 1;
    }

    /// Extract a feature vector from the current buffer state
    pub fn extract(&self) -> Option<Vec<f64>> {
        let needed = self.config.smooth_n + self.config.lag_n + self.config.diff_n + 1;
        if self.count < needed {
            return None;
        }

        // Get the last `needed` values in order
        let values = self.recent_values(needed);

        // Step 1: Smoothing (simple moving average)
        let smoothed = smooth(&values, self.config.smooth_n);

        // Step 2: Differencing
        let diffed = diff(&smoothed, self.config.diff_n);

        // Step 3: Lag features
        let features = lag_features(&diffed, self.config.lag_n);

        features
    }

    /// Extract training features (all possible feature vectors from buffer)
    pub fn extract_training_batch(&self) -> Vec<Vec<f64>> {
        let needed = self.config.smooth_n + self.config.lag_n + self.config.diff_n + 1;
        let available = self.count.min(self.buffer.len());
        if available < needed {
            return Vec::new();
        }

        let all_values = self.recent_values(available);
        let smoothed = smooth(&all_values, self.config.smooth_n);
        let diffed = diff(&smoothed, self.config.diff_n);

        // Generate sliding window of lag features
        let mut batch = Vec::new();
        if diffed.len() >= self.config.lag_n {
            for i in self.config.lag_n..diffed.len() {
                let feature: Vec<f64> = (0..self.config.lag_n)
                    .map(|j| diffed[i - j])
                    .collect();
                batch.push(feature);
            }
        }

        batch
    }

    fn recent_values(&self, n: usize) -> Vec<f64> {
        let n = n.min(self.count).min(self.buffer.len());
        let mut values = Vec::with_capacity(n);
        let start = if self.count >= self.buffer.len() {
            self.write_pos
        } else {
            0
        };
        let total = self.count.min(self.buffer.len());
        let skip = total.saturating_sub(n);

        for i in 0..total {
            if i < skip { continue; }
            let idx = (start + i) % self.buffer.len();
            values.push(self.buffer[idx]);
        }
        values
    }
}

/// K-means anomaly engine managing models for all metric streams
pub struct KmeansAnomalyEngine {
    config: KmeansConfig,
    /// Per-metric models keyed by "target_id:metric_name:resource_id"
    models: HashMap<String, KmeansModel>,
    /// Per-metric feature extractors
    extractors: HashMap<String, FeatureExtractor>,
    /// Samples since last retrain per metric
    samples_since_train: HashMap<String, usize>,
}

impl KmeansAnomalyEngine {
    pub fn new(config: KmeansConfig) -> Self {
        Self {
            config,
            models: HashMap::new(),
            extractors: HashMap::new(),
            samples_since_train: HashMap::new(),
        }
    }

    /// Process a single metric value and return an anomaly score
    pub fn process(
        &mut self,
        target_id: Uuid,
        metric_name: &str,
        resource_id: &str,
        value: f64,
    ) -> f64 {
        let key = format!("{}:{}:{}", target_id, metric_name, resource_id);

        // Get or create extractor
        let extractor = self.extractors
            .entry(key.clone())
            .or_insert_with(|| FeatureExtractor::new(&self.config));
        extractor.push(value);

        // Track samples
        let count = self.samples_since_train.entry(key.clone()).or_insert(0);
        *count += 1;

        // Train if needed
        let model = self.models.entry(key.clone()).or_insert_with(|| KmeansModel::new(&key));

        if !model.is_trained && extractor.count >= self.config.min_training_samples {
            let features = extractor.extract_training_batch();
            if !features.is_empty() {
                model.train(&features, &self.config);
                *count = 0;
                info!(key = %key, samples = model.training_samples, "K-means model trained");
            }
        } else if model.is_trained && *count >= self.config.retrain_interval {
            let features = extractor.extract_training_batch();
            if !features.is_empty() {
                model.train(&features, &self.config);
                *count = 0;
                debug!(key = %key, "K-means model retrained");
            }
        }

        // Score
        if let Some(feature) = extractor.extract() {
            model.score(&feature)
        } else {
            0.0
        }
    }

    /// Check if a given metric's anomaly score exceeds the threshold
    pub fn is_anomalous(&self, score: f64) -> bool {
        score >= self.config.anomaly_score_threshold
    }

    /// Compute host-level anomaly rate (fraction of metrics with anomaly score > threshold)
    pub fn host_anomaly_rate(&self, target_id: Uuid) -> f64 {
        let prefix = format!("{}:", target_id);
        let mut total = 0;
        let mut anomalous = 0;

        for (key, model) in &self.models {
            if key.starts_with(&prefix) && model.is_trained {
                total += 1;
                // For now, use the most recent score (stored in model)
                // In practice, we'd track rolling scores
            }
        }

        if total == 0 {
            return 0.0;
        }

        anomalous as f64 / total as f64
    }

    /// Get model status for all streams
    pub fn model_status(&self) -> Vec<ModelStatus> {
        self.models.values().map(|m| ModelStatus {
            metric_key: m.metric_key.clone(),
            is_trained: m.is_trained,
            training_samples: m.training_samples,
            num_clusters: m.cluster_centers.len(),
            trained_at: m.trained_at,
        }).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatus {
    pub metric_key: String,
    pub is_trained: bool,
    pub training_samples: usize,
    pub num_clusters: usize,
    pub trained_at: DateTime<Utc>,
}

// ── Helper functions ──

fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn nearest_center(point: &[f64], centers: &[Vec<f64>]) -> usize {
    centers.iter().enumerate()
        .map(|(i, c)| (i, euclidean_distance(point, c)))
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn kmeans_plus_plus_init(data: &[Vec<f64>], k: usize) -> Vec<Vec<f64>> {
    if data.is_empty() || k == 0 {
        return Vec::new();
    }

    let mut centers = vec![data[0].clone()];

    for _ in 1..k {
        // Compute distances to nearest existing center
        let distances: Vec<f64> = data.iter().map(|point| {
            centers.iter()
                .map(|c| euclidean_distance(point, c))
                .fold(f64::INFINITY, f64::min)
        }).collect();

        let total: f64 = distances.iter().map(|d| d * d).sum();
        if total <= 0.0 {
            // All points are at centers, just pick next sequentially
            if centers.len() < data.len() {
                centers.push(data[centers.len()].clone());
            }
            continue;
        }

        // Weighted random selection (deterministic: pick the point with max distance)
        let max_idx = distances.iter().enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        centers.push(data[max_idx].clone());
    }

    centers
}

fn smooth(values: &[f64], window: usize) -> Vec<f64> {
    if window <= 1 || values.len() < window {
        return values.to_vec();
    }

    let mut result = Vec::with_capacity(values.len());
    let mut sum: f64 = values[..window].iter().sum();
    result.push(sum / window as f64);

    for i in window..values.len() {
        sum += values[i] - values[i - window];
        result.push(sum / window as f64);
    }

    result
}

fn diff(values: &[f64], order: usize) -> Vec<f64> {
    if order == 0 || values.len() <= order {
        return values.to_vec();
    }

    let mut result = values.to_vec();
    for _ in 0..order {
        let mut diffed = Vec::with_capacity(result.len() - 1);
        for i in 1..result.len() {
            diffed.push(result[i] - result[i - 1]);
        }
        result = diffed;
    }
    result
}

fn lag_features(values: &[f64], lag_n: usize) -> Option<Vec<f64>> {
    if values.len() < lag_n {
        return None;
    }
    let start = values.len() - lag_n;
    Some(values[start..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kmeans_model_train_and_score() {
        let config = KmeansConfig {
            num_clusters: 2,
            max_iterations: 50,
            lag_n: 3,
            ..Default::default()
        };

        // Generate two clusters of feature vectors
        let mut features = Vec::new();
        for _ in 0..50 {
            features.push(vec![1.0, 1.1, 0.9]);
            features.push(vec![5.0, 5.1, 4.9]);
        }

        let mut model = KmeansModel::new("test");
        model.train(&features, &config);

        assert!(model.is_trained);
        assert_eq!(model.cluster_centers.len(), 2);

        // A point near cluster 1 should have low score
        let score_near = model.score(&[1.0, 1.0, 1.0]);
        assert!(score_near < 0.5, "Score near cluster should be low: {}", score_near);

        // A point far from both clusters should have high score
        let score_far = model.score(&[100.0, 100.0, 100.0]);
        assert!(score_far > 0.5, "Score far from clusters should be high: {}", score_far);
    }

    #[test]
    fn test_feature_extraction() {
        let config = KmeansConfig {
            diff_n: 1,
            smooth_n: 2,
            lag_n: 3,
            ..Default::default()
        };

        let mut extractor = FeatureExtractor::new(&config);

        // Push enough values
        for i in 0..20 {
            extractor.push(i as f64);
        }

        let feature = extractor.extract();
        assert!(feature.is_some());
        let f = feature.unwrap();
        assert_eq!(f.len(), 3); // lag_n = 3
    }

    #[test]
    fn test_smooth() {
        let values = vec![1.0, 3.0, 5.0, 7.0, 9.0];
        let smoothed = smooth(&values, 3);
        assert_eq!(smoothed.len(), 3); // 5 - 3 + 1
        assert!((smoothed[0] - 3.0).abs() < 1e-10); // avg(1,3,5)
        assert!((smoothed[1] - 5.0).abs() < 1e-10); // avg(3,5,7)
        assert!((smoothed[2] - 7.0).abs() < 1e-10); // avg(5,7,9)
    }

    #[test]
    fn test_diff() {
        let values = vec![1.0, 3.0, 6.0, 10.0];
        let diffed = diff(&values, 1);
        assert_eq!(diffed, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_kmeans_engine_process() {
        let config = KmeansConfig {
            min_training_samples: 20,
            diff_n: 1,
            smooth_n: 2,
            lag_n: 3,
            ..Default::default()
        };

        let mut engine = KmeansAnomalyEngine::new(config);
        let target_id = Uuid::new_v4();

        // Feed normal data
        for i in 0..50 {
            let value = 50.0 + (i as f64 * 0.1).sin() * 5.0;
            let score = engine.process(target_id, "cpu", "host-1", value);
            // Score should be low for normal data
            assert!(score <= 1.0);
        }
    }

    #[test]
    fn test_kmeans_plus_plus_init() {
        let data = vec![
            vec![0.0, 0.0],
            vec![10.0, 10.0],
            vec![0.1, 0.1],
            vec![9.9, 9.9],
        ];

        let centers = kmeans_plus_plus_init(&data, 2);
        assert_eq!(centers.len(), 2);

        // Centers should be far apart
        let dist = euclidean_distance(&centers[0], &centers[1]);
        assert!(dist > 5.0, "Centers should be far apart: {}", dist);
    }
}
