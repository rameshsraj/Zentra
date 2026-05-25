use zentra_db::DbPool;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use serde::{Serialize, Deserialize};
use tracing::debug;

/// Correlation engine: links metrics ↔ logs by time proximity and target
pub struct CorrelationEngine {
    pool: DbPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelatedEvent {
    pub event_type: String, // "metric" or "log"
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub summary: String,
    pub severity: Option<String>,
    pub target_id: Uuid,
}

impl CorrelationEngine {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Given a metric spike, find correlated log entries within a time window
    pub async fn logs_for_metric(
        &self,
        org_id: Uuid,
        target_id: Uuid,
        metric_timestamp: DateTime<Utc>,
        window_minutes: i64,
    ) -> Result<Vec<CorrelatedEvent>, zentra_core::Error> {
        let from = metric_timestamp - Duration::minutes(window_minutes);
        let to = metric_timestamp + Duration::minutes(window_minutes);

        let rows = sqlx::query_as::<_, (Uuid, DateTime<Utc>, String, String, String)>(
            "SELECT log_id, timestamp, source, severity, message FROM logs WHERE org_id = $1 AND target_id = $2 AND timestamp >= $3 AND timestamp <= $4 ORDER BY timestamp"
        )
        .bind(org_id)
        .bind(target_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(zentra_core::Error::from)?;

        debug!(count = rows.len(), "Found correlated logs for metric");

        Ok(rows.into_iter().map(|(id, ts, _source, severity, msg)| CorrelatedEvent {
            event_type: "log".into(),
            id: id.to_string(),
            timestamp: ts,
            summary: if msg.len() > 200 { format!("{}...", &msg[..200]) } else { msg },
            severity: Some(severity),
            target_id,
        }).collect())
    }

    /// Given a log entry, find correlated metric spikes within a time window
    pub async fn metrics_for_log(
        &self,
        org_id: Uuid,
        target_id: Uuid,
        log_timestamp: DateTime<Utc>,
        window_minutes: i64,
    ) -> Result<Vec<CorrelatedEvent>, zentra_core::Error> {
        let from = log_timestamp - Duration::minutes(window_minutes);
        let to = log_timestamp + Duration::minutes(window_minutes);

        let rows = sqlx::query_as::<_, (String, DateTime<Utc>, f64, String)>(
            "SELECT metric_name, timestamp, value, unit FROM metrics WHERE org_id = $1 AND target_id = $2 AND timestamp >= $3 AND timestamp <= $4 ORDER BY timestamp"
        )
        .bind(org_id)
        .bind(target_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(zentra_core::Error::from)?;

        debug!(count = rows.len(), "Found correlated metrics for log");

        Ok(rows.into_iter().map(|(name, ts, value, unit)| CorrelatedEvent {
            event_type: "metric".into(),
            id: format!("{name}@{ts}"),
            timestamp: ts,
            summary: format!("{name} = {value:.2} {unit}"),
            severity: None,
            target_id,
        }).collect())
    }
}
