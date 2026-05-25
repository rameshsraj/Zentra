use zentra_core::models::alert::{Alert, AlertRule, AlertSeverity, AlertStatus};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::info;

pub async fn insert_alert(pool: &PgPool, alert: &Alert) -> Result<(), sqlx::Error> {
    let metadata = alert.metadata.as_ref();
    sqlx::query(
        r#"INSERT INTO alerts
           (alert_id, rule_id, org_id, target_id, resource_id, severity, status, message,
            metric_value, threshold, triggered_at, metadata)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#
    )
    .bind(alert.alert_id)
    .bind(alert.rule_id)
    .bind(alert.org_id)
    .bind(alert.target_id)
    .bind(&alert.resource_id)
    .bind(alert.severity.as_str())
    .bind(format!("{:?}", alert.status).to_lowercase())
    .bind(&alert.message)
    .bind(alert.metric_value)
    .bind(alert.threshold)
    .bind(alert.triggered_at)
    .bind(metadata)
    .execute(pool)
    .await?;

    info!(alert_id = %alert.alert_id, "Alert inserted");
    Ok(())
}

pub async fn list_alerts(
    pool: &PgPool,
    org_id: Uuid,
    status: Option<&str>,
    severity: Option<&str>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Alert>, sqlx::Error> {
    let mut sql = String::from(
        "SELECT alert_id, rule_id, org_id, target_id, resource_id, severity, status, message, metric_value, threshold, triggered_at, acknowledged_at, acknowledged_by, resolved_at, metadata FROM alerts WHERE org_id = $1"
    );
    let mut idx = 2i32;

    if status.is_some() {
        sql.push_str(&format!(" AND status = ${idx}"));
        idx += 1;
    }
    if severity.is_some() {
        sql.push_str(&format!(" AND severity = ${idx}"));
        idx += 1;
    }
    if from.is_some() {
        sql.push_str(&format!(" AND triggered_at >= ${idx}"));
        idx += 1;
    }
    if to.is_some() {
        sql.push_str(&format!(" AND triggered_at < ${idx}"));
        idx += 1;
    }
    sql.push_str(&format!(" ORDER BY triggered_at DESC LIMIT ${idx} OFFSET ${}", idx + 1));

    let mut q = sqlx::query_as::<_, AlertRow>(&sql).bind(org_id);
    if let Some(s) = status { q = q.bind(s); }
    if let Some(s) = severity { q = q.bind(s); }
    if let Some(f) = from { q = q.bind(f); }
    if let Some(t) = to { q = q.bind(t); }
    q = q.bind(limit).bind(offset);

    let rows = q.fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn update_alert_status(
    pool: &PgPool,
    alert_id: Uuid,
    status: &str,
    acknowledged_by: Option<&str>,
) -> Result<(), sqlx::Error> {
    match status {
        "acknowledged" => {
            sqlx::query(
                "UPDATE alerts SET status = $2, acknowledged_at = now(), acknowledged_by = $3 WHERE alert_id = $1"
            )
            .bind(alert_id)
            .bind(status)
            .bind(acknowledged_by)
            .execute(pool)
            .await?;
        }
        "resolved" => {
            sqlx::query(
                "UPDATE alerts SET status = $2, resolved_at = now() WHERE alert_id = $1"
            )
            .bind(alert_id)
            .bind(status)
            .execute(pool)
            .await?;
        }
        _ => {
            sqlx::query("UPDATE alerts SET status = $2 WHERE alert_id = $1")
                .bind(alert_id)
                .bind(status)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

// Alert rules
pub async fn insert_alert_rule(pool: &PgPool, rule: &AlertRule) -> Result<(), sqlx::Error> {
    let condition = serde_json::to_value(&rule.condition).unwrap_or_default();
    let tags = serde_json::to_value(&rule.tags).unwrap_or_default();

    sqlx::query(
        r#"INSERT INTO alert_rules
           (rule_id, org_id, name, description, metric_name, condition, window_seconds, severity, enabled, tags, created_at, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"#
    )
    .bind(rule.rule_id)
    .bind(rule.org_id)
    .bind(&rule.name)
    .bind(&rule.description)
    .bind(&rule.metric_name)
    .bind(&condition)
    .bind(rule.window_seconds as i32)
    .bind(rule.severity.as_str())
    .bind(rule.enabled)
    .bind(&tags)
    .bind(rule.created_at)
    .bind(rule.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_alert_rules(pool: &PgPool, org_id: Uuid) -> Result<Vec<AlertRule>, sqlx::Error> {
    let rows = sqlx::query_as::<_, AlertRuleRow>(
        "SELECT rule_id, org_id, name, description, metric_name, condition, window_seconds, severity, enabled, tags, created_at, updated_at FROM alert_rules WHERE org_id = $1 ORDER BY name"
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(|r| r.try_into().ok()).collect())
}

#[derive(sqlx::FromRow)]
struct AlertRow {
    alert_id: Uuid,
    rule_id: Uuid,
    org_id: Uuid,
    target_id: Uuid,
    resource_id: String,
    severity: String,
    status: String,
    message: String,
    metric_value: f64,
    threshold: f64,
    triggered_at: DateTime<Utc>,
    acknowledged_at: Option<DateTime<Utc>>,
    acknowledged_by: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
    metadata: Option<serde_json::Value>,
}

impl From<AlertRow> for Alert {
    fn from(r: AlertRow) -> Self {
        Alert {
            alert_id: r.alert_id,
            rule_id: r.rule_id,
            org_id: r.org_id,
            target_id: r.target_id,
            resource_id: r.resource_id,
            severity: match r.severity.as_str() {
                "critical" => AlertSeverity::Critical,
                "warning" => AlertSeverity::Warning,
                _ => AlertSeverity::Info,
            },
            status: match r.status.as_str() {
                "pending" => AlertStatus::Pending,
                "triggered" => AlertStatus::Triggered,
                "acknowledged" => AlertStatus::Acknowledged,
                "resolved" => AlertStatus::Resolved,
                "suppressed" => AlertStatus::Suppressed,
                _ => AlertStatus::Triggered,
            },
            message: r.message,
            metric_value: r.metric_value,
            threshold: r.threshold,
            triggered_at: r.triggered_at,
            acknowledged_at: r.acknowledged_at,
            acknowledged_by: r.acknowledged_by,
            resolved_at: r.resolved_at,
            metadata: r.metadata,
            pending_since: None,
            last_notified_at: None,
            notification_count: 0,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AlertRuleRow {
    rule_id: Uuid,
    org_id: Uuid,
    name: String,
    description: Option<String>,
    metric_name: String,
    condition: serde_json::Value,
    window_seconds: i32,
    severity: String,
    enabled: bool,
    tags: serde_json::Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<AlertRuleRow> for AlertRule {
    type Error = String;
    fn try_from(r: AlertRuleRow) -> Result<Self, String> {
        let condition = serde_json::from_value(r.condition)
            .map_err(|e| format!("Invalid condition: {e}"))?;
        let tags: Vec<String> = serde_json::from_value(r.tags).unwrap_or_default();
        Ok(AlertRule {
            rule_id: r.rule_id,
            org_id: r.org_id,
            name: r.name,
            description: r.description,
            metric_name: r.metric_name,
            condition,
            window_seconds: r.window_seconds as u32,
            severity: match r.severity.as_str() {
                "critical" => AlertSeverity::Critical,
                "warning" => AlertSeverity::Warning,
                _ => AlertSeverity::Info,
            },
            enabled: r.enabled,
            tags,
            created_at: r.created_at,
            updated_at: r.updated_at,
            for_duration_seconds: 0,
            hysteresis_threshold: None,
            repeat_interval_seconds: 3600,
            notification_policy_id: None,
        })
    }
}
