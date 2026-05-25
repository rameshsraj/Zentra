use zentra_core::Metric;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{info, error};

/// Insert a batch of metrics into TimescaleDB
pub async fn insert_metrics(pool: &PgPool, metrics: &[Metric]) -> Result<u64, sqlx::Error> {
    if metrics.is_empty() {
        return Ok(0);
    }

    let mut count = 0u64;
    // Use batched inserts for performance
    for chunk in metrics.chunks(500) {
        let mut query = String::from(
            "INSERT INTO metrics (timestamp, org_id, target_id, plugin_id, resource_type, resource_id, metric_name, value, unit, tags, metadata) VALUES "
        );
        let mut params_idx = 1u32;

        for (i, _m) in chunk.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                params_idx, params_idx + 1, params_idx + 2, params_idx + 3,
                params_idx + 4, params_idx + 5, params_idx + 6, params_idx + 7,
                params_idx + 8, params_idx + 9, params_idx + 10
            ));
            params_idx += 11;
        }

        // Pre-compute serialized tags so they live long enough for bind
        let tags_vec: Vec<serde_json::Value> = chunk.iter()
            .map(|m| serde_json::to_value(&m.tags).unwrap_or_default())
            .collect();

        let mut q = sqlx::query(&query);
        for (idx, m) in chunk.iter().enumerate() {
            q = q
                .bind(m.timestamp)
                .bind(m.org_id)
                .bind(m.target_id)
                .bind(&m.plugin_id)
                .bind(&m.resource_type)
                .bind(&m.resource_id)
                .bind(&m.metric_name)
                .bind(m.value)
                .bind(&m.unit)
                .bind(&tags_vec[idx])
                .bind(&m.metadata);
        }

        match q.execute(pool).await {
            Ok(result) => count += result.rows_affected(),
            Err(e) => {
                error!(error = %e, "Failed to insert metric batch");
                return Err(e);
            }
        }
    }

    info!(count, "Metrics inserted");
    Ok(count)
}

/// Query metrics with time range and optional filters
pub async fn query_metrics(
    pool: &PgPool,
    org_id: Uuid,
    metric_name: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    target_id: Option<Uuid>,
    limit: i64,
) -> Result<Vec<Metric>, sqlx::Error> {
    let rows = if let Some(tid) = target_id {
        sqlx::query_as::<_, MetricRow>(
            r#"SELECT timestamp, org_id, target_id, plugin_id, resource_type, resource_id,
                      metric_name, value, unit, tags, metadata
               FROM metrics
               WHERE org_id = $1 AND metric_name = $2 AND timestamp >= $3 AND timestamp < $4 AND target_id = $5
               ORDER BY timestamp DESC
               LIMIT $6"#
        )
        .bind(org_id).bind(metric_name).bind(from).bind(to).bind(tid).bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, MetricRow>(
            r#"SELECT timestamp, org_id, target_id, plugin_id, resource_type, resource_id,
                      metric_name, value, unit, tags, metadata
               FROM metrics
               WHERE org_id = $1 AND metric_name = $2 AND timestamp >= $3 AND timestamp < $4
               ORDER BY timestamp DESC
               LIMIT $5"#
        )
        .bind(org_id).bind(metric_name).bind(from).bind(to).bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Time-bucketed aggregation for dashboard charts.
/// Automatically selects the best continuous aggregate based on bucket size:
///   - bucket >= 86400s (1d) → metrics_1d
///   - bucket >= 3600s  (1h) → metrics_1h
///   - bucket >= 300s   (5m) → metrics_5m
///   - bucket >= 60s    (1m) → metrics_1m
///   - bucket < 60s          → raw hypertable with time_bucket
pub async fn query_metrics_aggregated(
    pool: &PgPool,
    org_id: Uuid,
    metric_name: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    bucket_seconds: i64,
) -> Result<Vec<AggregatedMetric>, sqlx::Error> {
    // Choose the most efficient source view
    let (source, re_bucket) = match bucket_seconds {
        s if s >= 86400 => ("metrics_1d", s != 86400),
        s if s >= 3600  => ("metrics_1h", s != 3600),
        s if s >= 300   => ("metrics_5m", s != 300),
        s if s >= 60    => ("metrics_1m", s != 60),
        _               => ("metrics", true),
    };

    let query = if source == "metrics" {
        // Raw hypertable — use time_bucket directly
        format!(
            r#"SELECT
                 time_bucket(make_interval(secs => $5), timestamp) AS bucket,
                 avg(value) AS avg_value,
                 max(value) AS max_value,
                 min(value) AS min_value,
                 count(*) AS count
               FROM metrics
               WHERE org_id = $1 AND metric_name = $2 AND timestamp >= $3 AND timestamp < $4
               GROUP BY 1
               ORDER BY 1"#
        )
    } else if re_bucket {
        // Continuous aggregate with re-bucketing for non-native intervals
        format!(
            r#"SELECT
                 time_bucket(make_interval(secs => $5), bucket) AS bucket,
                 avg(avg_value) AS avg_value,
                 max(max_value) AS max_value,
                 min(min_value) AS min_value,
                 sum(sample_count) AS count
               FROM {source}
               WHERE org_id = $1 AND metric_name = $2 AND bucket >= $3 AND bucket < $4
               GROUP BY 1
               ORDER BY 1"#
        )
    } else {
        // Continuous aggregate at native resolution — no re-bucketing needed
        format!(
            r#"SELECT
                 bucket,
                 avg_value,
                 max_value,
                 min_value,
                 sample_count AS count
               FROM {source}
               WHERE org_id = $1 AND metric_name = $2 AND bucket >= $3 AND bucket < $4
               ORDER BY 1"#
        )
    };

    let rows = if re_bucket || source == "metrics" {
        sqlx::query_as::<_, AggregatedMetricRow>(&query)
            .bind(org_id).bind(metric_name).bind(from).bind(to).bind(bucket_seconds as f64)
            .fetch_all(pool)
            .await?
    } else {
        // Native resolution — no $5 param needed, but we bind it anyway for consistency
        sqlx::query_as::<_, AggregatedMetricRow>(&query)
            .bind(org_id).bind(metric_name).bind(from).bind(to)
            .fetch_all(pool)
            .await?
    };

    Ok(rows.into_iter().map(|r| AggregatedMetric {
        bucket: r.bucket,
        avg_value: r.avg_value.unwrap_or(0.0),
        max_value: r.max_value.unwrap_or(0.0),
        min_value: r.min_value.unwrap_or(0.0),
        count: r.count.unwrap_or(0),
    }).collect())
}

/// Get TimescaleDB compression stats for monitoring
pub async fn get_compression_stats(pool: &PgPool) -> Result<Vec<CompressionStats>, sqlx::Error> {
    let rows = sqlx::query_as::<_, CompressionStatsRow>(
        r#"SELECT
             hypertable_name::text AS table_name,
             total_chunks,
             compressed_chunks,
             COALESCE(before_compression_total_bytes, 0) AS uncompressed_bytes,
             COALESCE(after_compression_total_bytes, 0) AS compressed_bytes
           FROM hypertable_compression_stats()
           ORDER BY hypertable_name"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| {
        let ratio = if r.compressed_bytes > 0 {
            r.uncompressed_bytes as f64 / r.compressed_bytes as f64
        } else {
            0.0
        };
        CompressionStats {
            table_name: r.table_name,
            total_chunks: r.total_chunks,
            compressed_chunks: r.compressed_chunks,
            uncompressed_bytes: r.uncompressed_bytes,
            compressed_bytes: r.compressed_bytes,
            compression_ratio: ratio,
        }
    }).collect())
}

/// Get hypertable size information
pub async fn get_hypertable_sizes(pool: &PgPool) -> Result<Vec<HypertableSize>, sqlx::Error> {
    let rows = sqlx::query_as::<_, HypertableSizeRow>(
        r#"SELECT
             hypertable_name::text AS table_name,
             total_bytes,
             num_chunks
           FROM hypertable_detailed_size()
           ORDER BY total_bytes DESC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(rows.into_iter().map(|r| HypertableSize {
        table_name: r.table_name,
        total_bytes: r.total_bytes,
        num_chunks: r.num_chunks,
    }).collect())
}

// Internal row type for sqlx mapping
#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct MetricRow {
    timestamp: DateTime<Utc>,
    org_id: Uuid,
    target_id: Uuid,
    plugin_id: String,
    resource_type: String,
    resource_id: String,
    metric_name: String,
    value: f64,
    unit: Option<String>,
    tags: serde_json::Value,
    metadata: Option<serde_json::Value>,
}

impl From<MetricRow> for Metric {
    fn from(r: MetricRow) -> Self {
        let tags: std::collections::BTreeMap<String, String> =
            serde_json::from_value(r.tags).unwrap_or_default();
        Metric {
            timestamp: r.timestamp,
            org_id: r.org_id,
            target_id: r.target_id,
            plugin_id: r.plugin_id,
            resource_type: r.resource_type,
            resource_id: r.resource_id,
            metric_name: r.metric_name,
            value: r.value,
            unit: r.unit.unwrap_or_default(),
            tags,
            metadata: r.metadata,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AggregatedMetric {
    pub bucket: DateTime<Utc>,
    pub avg_value: f64,
    pub max_value: f64,
    pub min_value: f64,
    pub count: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct AggregatedMetricRow {
    bucket: DateTime<Utc>,
    avg_value: Option<f64>,
    max_value: Option<f64>,
    min_value: Option<f64>,
    count: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CompressionStats {
    pub table_name: String,
    pub total_chunks: i64,
    pub compressed_chunks: i64,
    pub uncompressed_bytes: i64,
    pub compressed_bytes: i64,
    pub compression_ratio: f64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct CompressionStatsRow {
    table_name: String,
    total_chunks: i64,
    compressed_chunks: i64,
    uncompressed_bytes: i64,
    compressed_bytes: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HypertableSize {
    pub table_name: String,
    pub total_bytes: i64,
    pub num_chunks: i64,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct HypertableSizeRow {
    table_name: String,
    total_bytes: i64,
    num_chunks: i64,
}
