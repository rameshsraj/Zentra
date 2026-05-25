pub mod metric_query;
pub mod log_query;
pub mod global_search;
pub mod promql;
pub mod mqe;
pub mod traceql;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Unified time range for all queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        Self { from, to }
    }

    /// Validate that the time range is within acceptable bounds.
    /// Returns Err if range exceeds 90 days or if `from` > `to`.
    pub fn validate(&self) -> Result<(), String> {
        if self.from > self.to {
            return Err("Invalid time range: 'from' must be before 'to'".into());
        }
        let max_range = chrono::Duration::days(90);
        if self.to.signed_duration_since(self.from) > max_range {
            return Err("Time range exceeds maximum of 90 days".into());
        }
        Ok(())
    }

    pub fn last_minutes(minutes: i64) -> Self {
        let to = Utc::now();
        let from = to - chrono::Duration::minutes(minutes);
        Self { from, to }
    }

    pub fn last_hours(hours: i64) -> Self {
        let to = Utc::now();
        let from = to - chrono::Duration::hours(hours);
        Self { from, to }
    }

    pub fn last_days(days: i64) -> Self {
        let to = Utc::now();
        let from = to - chrono::Duration::days(days);
        Self { from, to }
    }

    /// Determine the optimal bucket size for this time range
    pub fn auto_bucket_seconds(&self) -> i64 {
        let duration = self.to.signed_duration_since(self.from);
        let secs = duration.num_seconds();
        match secs {
            0..=3600 => 10,             // ≤1h → 10s buckets
            3601..=21600 => 60,         // ≤6h → 1m buckets
            21601..=86400 => 300,       // ≤1d → 5m buckets
            86401..=604800 => 3600,     // ≤7d → 1h buckets
            _ => 86400,                 // >7d → 1d buckets
        }
    }
}

/// Pagination parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: i64,
    pub offset: i64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

/// Global search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub result_type: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub score: f64,
    pub metadata: serde_json::Value,
}
