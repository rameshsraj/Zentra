use zentra_core::Metric;
use zentra_db::DbPool;
use zentra_db::repositories::metrics::{query_metrics, query_metrics_aggregated, AggregatedMetric};
use super::TimeRange;
use uuid::Uuid;

pub struct MetricQueryService {
    pool: DbPool,
}

impl MetricQueryService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get raw metrics for a given metric name and time range
    pub async fn get_metrics(
        &self,
        org_id: Uuid,
        metric_name: &str,
        time_range: &TimeRange,
        target_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<Metric>, zentra_core::Error> {
        query_metrics(
            &self.pool,
            org_id,
            metric_name,
            time_range.from,
            time_range.to,
            target_id,
            limit,
        )
        .await
        .map_err(zentra_core::Error::from)
    }

    /// Get aggregated (bucketed) metrics for chart display
    pub async fn get_metrics_aggregated(
        &self,
        org_id: Uuid,
        metric_name: &str,
        time_range: &TimeRange,
        bucket_seconds: Option<i64>,
    ) -> Result<Vec<AggregatedMetric>, zentra_core::Error> {
        let bucket = bucket_seconds.unwrap_or_else(|| time_range.auto_bucket_seconds());
        query_metrics_aggregated(
            &self.pool,
            org_id,
            metric_name,
            time_range.from,
            time_range.to,
            bucket,
        )
        .await
        .map_err(zentra_core::Error::from)
    }
}
