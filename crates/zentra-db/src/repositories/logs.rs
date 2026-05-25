use zentra_core::LogEntry;
use zentra_core::models::log_entry::LogSeverity;
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{info, error};

/// Insert a batch of log entries
pub async fn insert_logs(pool: &PgPool, entries: &[LogEntry]) -> Result<u64, sqlx::Error> {
    if entries.is_empty() {
        return Ok(0);
    }

    let mut count = 0u64;
    for chunk in entries.chunks(200) {
        let mut query = String::from(
            "INSERT INTO logs (log_id, timestamp, org_id, target_id, source, component, component_type, severity, message, raw_log, parsed_fields, tags) VALUES "
        );
        let mut idx = 1u32;
        for (i, _) in chunk.iter().enumerate() {
            if i > 0 { query.push_str(", "); }
            query.push_str(&format!(
                "(${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${}, ${})",
                idx, idx+1, idx+2, idx+3, idx+4, idx+5, idx+6, idx+7, idx+8, idx+9, idx+10, idx+11
            ));
            idx += 12;
        }

        // Pre-compute serialized fields so they outlive the bind loop
        let parsed_vec: Vec<serde_json::Value> = chunk.iter()
            .map(|e| serde_json::to_value(&e.parsed_fields).unwrap_or_default())
            .collect();
        let tags_vec: Vec<serde_json::Value> = chunk.iter()
            .map(|e| serde_json::to_value(&e.tags).unwrap_or_default())
            .collect();

        let mut q = sqlx::query(&query);
        for (i, e) in chunk.iter().enumerate() {
            q = q
                .bind(e.log_id)
                .bind(e.timestamp)
                .bind(e.org_id)
                .bind(e.target_id)
                .bind(&e.source)
                .bind(&e.component)
                .bind(&e.component_type)
                .bind(e.severity.as_str())
                .bind(&e.message)
                .bind(&e.raw_log)
                .bind(&parsed_vec[i])
                .bind(&tags_vec[i]);
        }

        match q.execute(pool).await {
            Ok(r) => count += r.rows_affected(),
            Err(e) => {
                error!(error = %e, "Failed to insert log batch");
                return Err(e);
            }
        }
    }

    info!(count, "Logs inserted");
    Ok(count)
}

/// Search logs with full-text search and filters
pub async fn search_logs(
    pool: &PgPool,
    org_id: Uuid,
    query_text: Option<&str>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    severity: Option<&str>,
    source: Option<&str>,
    target_id: Option<Uuid>,
    limit: i64,
    offset: i64,
) -> Result<Vec<LogEntry>, sqlx::Error> {
    // Build dynamic query
    let mut sql = String::from(
        "SELECT log_id, timestamp, org_id, target_id, source, component, component_type, severity, message, raw_log, parsed_fields, tags FROM logs WHERE org_id = $1 AND timestamp >= $2 AND timestamp < $3"
    );
    let mut param_idx = 4i32;

    if severity.is_some() {
        sql.push_str(&format!(" AND severity = ${param_idx}"));
        param_idx += 1;
    }
    if source.is_some() {
        sql.push_str(&format!(" AND source = ${param_idx}"));
        param_idx += 1;
    }
    if target_id.is_some() {
        sql.push_str(&format!(" AND target_id = ${param_idx}"));
        param_idx += 1;
    }
    if query_text.is_some() {
        sql.push_str(&format!(" AND message_tsv @@ plainto_tsquery('english', ${param_idx})"));
        param_idx += 1;
    }

    sql.push_str(&format!(" ORDER BY timestamp DESC LIMIT ${param_idx} OFFSET ${}", param_idx + 1));

    let mut q = sqlx::query_as::<_, LogRow>(&sql)
        .bind(org_id)
        .bind(from)
        .bind(to);

    if let Some(s) = severity { q = q.bind(s); }
    if let Some(s) = source { q = q.bind(s); }
    if let Some(t) = target_id { q = q.bind(t); }
    if let Some(t) = query_text { q = q.bind(t); }
    q = q.bind(limit).bind(offset);

    let rows = q.fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

#[derive(sqlx::FromRow)]
struct LogRow {
    log_id: Uuid,
    timestamp: DateTime<Utc>,
    org_id: Uuid,
    target_id: Uuid,
    source: String,
    component: String,
    component_type: String,
    severity: String,
    message: String,
    raw_log: Option<String>,
    parsed_fields: serde_json::Value,
    tags: serde_json::Value,
}

impl From<LogRow> for LogEntry {
    fn from(r: LogRow) -> Self {
        LogEntry {
            log_id: r.log_id,
            timestamp: r.timestamp,
            org_id: r.org_id,
            target_id: r.target_id,
            source: r.source,
            component: r.component,
            component_type: r.component_type,
            severity: LogSeverity::from_str_loose(&r.severity),
            message: r.message,
            raw_log: r.raw_log,
            parsed_fields: serde_json::from_value(r.parsed_fields).unwrap_or_default(),
            tags: serde_json::from_value(r.tags).unwrap_or_default(),
        }
    }
}
