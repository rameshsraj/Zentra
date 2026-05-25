//! Z-score (sigma) anomaly detector with sliding window statistics.
//!
//! Maintains a rolling window of N samples. A new value is anomalous when:
//!   |value - mean| / stddev > threshold_sigma
//!
//! Uses Welford's online algorithm for numerically stable mean/variance.

use std::collections::VecDeque;

pub struct ZScoreDetector {
    window_size: usize,
    threshold_sigma: f64,
    window: VecDeque<f64>,
    // Welford running stats
    n: u64,
    mean: f64,
    m2: f64,
}

impl ZScoreDetector {
    pub fn new(window_size: usize, threshold_sigma: f64) -> Self {
        Self {
            window_size: window_size.max(10),
            threshold_sigma,
            window: VecDeque::with_capacity(window_size),
            n: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    /// Returns (is_anomaly, z_score, current_mean, current_stddev)
    pub fn update(&mut self, value: f64) -> (bool, f64, f64, f64) {
        // Evict oldest if at capacity
        if self.window.len() == self.window_size {
            if let Some(old) = self.window.pop_front() {
                // Welford removal (requires n > 1)
                if self.n > 1 {
                    let old_mean = self.mean;
                    self.mean = (self.mean * self.n as f64 - old) / (self.n as f64 - 1.0);
                    self.m2 -= (old - old_mean) * (old - self.mean);
                    self.m2 = self.m2.max(0.0);
                    self.n -= 1;
                }
            }
        }

        // Welford update
        self.n += 1;
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        self.window.push_back(value);

        let stddev = if self.n > 1 { (self.m2 / (self.n - 1) as f64).sqrt() } else { 0.0 };

        if self.n < 10 || stddev < 1e-10 {
            // Not enough data or zero variance — not anomalous
            return (false, 0.0, self.mean, stddev);
        }

        let z = (value - self.mean).abs() / stddev;
        let is_anomaly = z > self.threshold_sigma;
        (is_anomaly, z, self.mean, stddev)
    }

    pub fn mean(&self) -> f64 { self.mean }
    pub fn stddev(&self) -> f64 {
        if self.n > 1 { (self.m2 / (self.n - 1) as f64).sqrt() } else { 0.0 }
    }
    pub fn sample_count(&self) -> u64 { self.n }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_spike() {
        let mut det = ZScoreDetector::new(100, 3.0);
        // Feed 50 normal values around 50.0
        for i in 0..50 {
            det.update(50.0 + (i % 3) as f64 * 0.1);
        }
        let (anomaly, z, _, _) = det.update(200.0); // Clear spike
        assert!(anomaly, "Should detect spike of 200 in distribution around 50");
        assert!(z > 3.0);
    }

    #[test]
    fn no_false_positive_on_normal() {
        let mut det = ZScoreDetector::new(100, 3.0);
        let mut anomalies = 0;
        for i in 0..200 {
            let v = 50.0 + ((i % 5) as f64 - 2.0) * 0.5;
            let (a, _, _, _) = det.update(v);
            if a { anomalies += 1; }
        }
        assert_eq!(anomalies, 0, "No anomalies in smooth data");
    }
}
