//! Seasonal anomaly detector — accounts for time-of-day and day-of-week patterns.
//!
//! Bins values by (hour_of_day × 7 + day_of_week) = 168 buckets (full week).
//! Each bucket maintains its own Welford statistics.
//! A value is anomalous if it deviates from the historical norm for that time slot.

use std::collections::HashMap;
use chrono::{Utc, Datelike, Timelike};

#[derive(Default, Clone, Debug)]
struct SeasonalBucket {
    n: u64,
    mean: f64,
    m2: f64,
    last_update: i64,
}

impl SeasonalBucket {
    fn update(&mut self, value: f64, timestamp_secs: i64) {
        self.n += 1;
        let delta = value - self.mean;
        self.mean += delta / self.n as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;
        self.last_update = timestamp_secs;
    }

    fn stddev(&self) -> f64 {
        if self.n > 1 { (self.m2 / (self.n - 1) as f64).sqrt() } else { 0.0 }
    }
}

pub struct SeasonalDetector {
    buckets: HashMap<u32, SeasonalBucket>, // key = hour * 7 + weekday
    threshold_sigma: f64,
    min_samples: u64,
    bucket_ttl_secs: i64,
}

impl SeasonalDetector {
    /// `threshold_sigma`: anomaly if |value - seasonal_mean| / seasonal_stddev > threshold
    /// `min_samples`: minimum bucket samples before anomaly detection activates
    pub fn new(threshold_sigma: f64, min_samples: u64) -> Self {
        Self {
            buckets: HashMap::with_capacity(168),
            threshold_sigma,
            min_samples: min_samples.max(5),
            bucket_ttl_secs: 30 * 86400, // 30 days default TTL
        }
    }

    /// Returns (is_anomaly, z_score, bucket_mean, bucket_stddev, bucket_key)
    pub fn update(&mut self, value: f64, timestamp_secs: i64) -> (bool, f64, f64, f64, u32) {
        let dt = chrono::DateTime::from_timestamp(timestamp_secs, 0)
            .unwrap_or_else(|| Utc::now());
        let hour = dt.hour();
        let weekday = dt.weekday().num_days_from_monday(); // 0=Mon, 6=Sun
        let key = hour * 7 + weekday;

        let bucket = self.buckets.entry(key).or_default();
        let prev_mean = bucket.mean;
        let prev_stddev = bucket.stddev();
        let prev_n = bucket.n;

        bucket.update(value, timestamp_secs);

        if prev_n < self.min_samples || prev_stddev < 1e-10 {
            return (false, 0.0, bucket.mean, bucket.stddev(), key);
        }

        let z = (value - prev_mean).abs() / prev_stddev;
        let is_anomaly = z > self.threshold_sigma;

        (is_anomaly, z, prev_mean, prev_stddev, key)
    }

    /// Load historical baseline from pre-computed bucket stats
    pub fn load_baseline(&mut self, key: u32, mean: f64, stddev: f64, n: u64) {
        let m2 = if n > 1 { stddev.powi(2) * (n - 1) as f64 } else { 0.0 };
        let last_update = Utc::now().timestamp();
        self.buckets.insert(key, SeasonalBucket { n, mean, m2, last_update });
    }

    pub fn bucket_count(&self) -> usize { self.buckets.len() }

    /// Remove buckets that haven't been updated within the TTL.
    /// Call periodically (e.g., during each recompute cycle).
    pub fn prune_stale(&mut self, now_secs: i64) {
        let cutoff = now_secs - self.bucket_ttl_secs;
        self.buckets.retain(|_, b| b.last_update >= cutoff);
    }

    /// Set the bucket TTL in seconds (default: 30 days).
    pub fn set_bucket_ttl(&mut self, ttl_secs: i64) {
        self.bucket_ttl_secs = ttl_secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn seasonal_detects_outside_pattern() {
        let mut det = SeasonalDetector::new(3.0, 10);
        let base_ts = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap().timestamp();

        // Train on Monday 9am — always ~100
        for week in 0..15i64 {
            let ts = base_ts + week * 7 * 86400;
            det.update(100.0, ts);
        }

        // Now Monday 9am with value 500 — should be anomalous
        let (anomaly, z, _, _, _) = det.update(500.0, base_ts + 15 * 7 * 86400);
        assert!(anomaly, "Monday 9am spike should be anomalous: z={}", z);
    }
}
