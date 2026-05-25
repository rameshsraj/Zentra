//! EWMA (Exponentially Weighted Moving Average) anomaly detector.
//!
//! Tracks a smoothed mean and variance using exponential decay.
//! Suitable for non-stationary time series where recent values matter more.
//!
//! Formula: mean_t = α * x_t + (1-α) * mean_{t-1}
//!          var_t  = (1-α) * (var_{t-1} + α * (x_t - mean_{t-1})^2)

pub struct EwmaDetector {
    alpha: f64,              // smoothing factor ∈ (0, 1]; higher = more reactive
    threshold_sigma: f64,
    mean: f64,
    variance: f64,
    initialized: bool,
    sample_count: u64,
    warmup: u64,
}

impl EwmaDetector {
    /// `alpha`: 0.1 = slow/smooth, 0.3 = moderate, 0.5 = fast/reactive
    /// `threshold_sigma`: anomaly if deviation > N standard deviations
    /// `warmup`: number of samples before anomalies are emitted
    pub fn new(alpha: f64, threshold_sigma: f64, warmup: u64) -> Self {
        Self {
            alpha: alpha.clamp(0.001, 1.0),
            threshold_sigma,
            mean: 0.0,
            variance: 0.0,
            initialized: false,
            sample_count: 0,
            warmup,
        }
    }

    /// Returns (is_anomaly, deviation_sigma, ewma_mean, ewma_stddev)
    pub fn update(&mut self, value: f64) -> (bool, f64, f64, f64) {
        self.sample_count += 1;

        if !self.initialized {
            self.mean = value;
            self.variance = 0.0;
            self.initialized = true;
            return (false, 0.0, self.mean, 0.0);
        }

        let prev_mean = self.mean;
        self.mean = self.alpha * value + (1.0 - self.alpha) * self.mean;
        self.variance = (1.0 - self.alpha) * (self.variance + self.alpha * (value - prev_mean).powi(2));

        let stddev = self.variance.sqrt();

        if self.sample_count < self.warmup || stddev < 1e-10 {
            return (false, 0.0, self.mean, stddev);
        }

        let deviation_sigma = (value - self.mean).abs() / stddev;
        let is_anomaly = deviation_sigma > self.threshold_sigma;

        (is_anomaly, deviation_sigma, self.mean, stddev)
    }

    pub fn mean(&self) -> f64 { self.mean }
    pub fn stddev(&self) -> f64 { self.variance.sqrt() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ewma_detects_spike() {
        let mut det = EwmaDetector::new(0.1, 3.0, 20);
        for _ in 0..50 {
            det.update(10.0);
        }
        let (anomaly, sigma, _, _) = det.update(500.0);
        assert!(anomaly, "Should detect spike");
        assert!(sigma > 3.0);
    }

    #[test]
    fn ewma_adapts_to_gradual_change() {
        let mut det = EwmaDetector::new(0.3, 3.0, 10);
        let mut anomalies = 0;
        for i in 0..100 {
            // Gradual drift from 10 to 20
            let v = 10.0 + i as f64 * 0.1;
            let (a, _, _, _) = det.update(v);
            if a && i > 20 { anomalies += 1; }
        }
        // Gradual change should not trigger many anomalies
        assert!(anomalies < 5, "Gradual drift should not flood anomalies: got {}", anomalies);
    }
}
