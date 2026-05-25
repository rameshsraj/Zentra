use zentra_db::DbPool;
use super::SearchResult;
use uuid::Uuid;
use tracing::debug;

/// Searches across metrics, logs, targets, and alerts
pub struct GlobalSearchService {
    pool: DbPool,
}

impl GlobalSearchService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn search(
        &self,
        org_id: Uuid,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SearchResult>, zentra_core::Error> {
        let mut results = Vec::new();

        // Search targets by name
        let target_results = sqlx::query_as::<_, (Uuid, String, String, String)>(
            "SELECT target_id, name, target_type, host FROM targets WHERE org_id = $1 AND (name ILIKE $2 OR host ILIKE $2) AND is_active = true LIMIT $3"
        )
        .bind(org_id)
        .bind(format!("%{query}%"))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for (id, name, ttype, host) in target_results {
            results.push(SearchResult {
                result_type: "target".into(),
                id: id.to_string(),
                title: name,
                subtitle: format!("{ttype} — {host}"),
                timestamp: None,
                score: 1.0,
                metadata: serde_json::json!({}),
            });
        }

        // Search logs by message full-text
        let log_results = sqlx::query_as::<_, (Uuid, String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT log_id, source, message, timestamp FROM logs WHERE org_id = $1 AND message_tsv @@ plainto_tsquery('english', $2) ORDER BY timestamp DESC LIMIT $3"
        )
        .bind(org_id)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for (id, source, message, ts) in log_results {
            let preview = if message.len() > 120 {
                format!("{}...", &message[..120])
            } else {
                message
            };
            results.push(SearchResult {
                result_type: "log".into(),
                id: id.to_string(),
                title: source,
                subtitle: preview,
                timestamp: Some(ts),
                score: 0.8,
                metadata: serde_json::json!({}),
            });
        }

        // Search alerts by message
        let alert_results = sqlx::query_as::<_, (Uuid, String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT alert_id, severity, message, triggered_at FROM alerts WHERE org_id = $1 AND message ILIKE $2 ORDER BY triggered_at DESC LIMIT $3"
        )
        .bind(org_id)
        .bind(format!("%{query}%"))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for (id, severity, message, ts) in alert_results {
            results.push(SearchResult {
                result_type: "alert".into(),
                id: id.to_string(),
                title: format!("[{severity}] Alert"),
                subtitle: message,
                timestamp: Some(ts),
                score: 0.9,
                metadata: serde_json::json!({}),
            });
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        debug!(query, count = results.len(), "Global search completed");
        Ok(results)
    }
}
