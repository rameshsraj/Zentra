use zentra_core::models::target::{Target, TargetStatus, TargetSubtype, TargetCredentials};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::info;

#[tracing::instrument(skip(pool, target), fields(db.system = "postgresql", db.operation = "INSERT", db.sql.table = "targets", otel.kind = "client"))]
pub async fn insert_target(pool: &PgPool, target: &Target) -> Result<(), sqlx::Error> {
    let creds_json = target.credentials.as_ref().map(|c| serde_json::to_value(c).unwrap_or_default());
    let conn_opts = serde_json::to_value(&target.connection_options).unwrap_or_default();
    let tags = serde_json::to_value(&target.tags).unwrap_or_default();

    sqlx::query(
        r#"INSERT INTO targets
           (target_id, org_id, name, description, target_type, target_subtype, host, port,
            credentials, connection_options, status, environment, tags, check_interval_seconds,
            consecutive_failures, created_at, updated_at, is_active)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)"#
    )
    .bind(target.target_id)
    .bind(target.org_id)
    .bind(&target.name)
    .bind(&target.description)
    .bind(target.target_type.as_str())
    .bind(serde_json::to_string(&target.target_subtype).unwrap_or_default().trim_matches('"'))
    .bind(&target.host)
    .bind(target.port as i32)
    .bind(&creds_json)
    .bind(&conn_opts)
    .bind(format!("{:?}", target.status).to_lowercase())
    .bind(&target.environment)
    .bind(&tags)
    .bind(target.check_interval_seconds as i32)
    .bind(target.consecutive_failures as i32)
    .bind(target.created_at)
    .bind(target.updated_at)
    .bind(target.is_active)
    .execute(pool)
    .await?;

    info!(target_id = %target.target_id, name = %target.name, "Target inserted");
    Ok(())
}

#[tracing::instrument(skip(pool), fields(db.system = "postgresql", db.operation = "SELECT", db.sql.table = "targets", otel.kind = "client"))]
pub async fn list_targets(
    pool: &PgPool,
    org_id: Uuid,
    target_type: Option<&str>,
    active_only: bool,
) -> Result<Vec<Target>, sqlx::Error> {
    let mut sql = String::from(
        "SELECT target_id, org_id, name, description, target_type, target_subtype, host, port, credentials, connection_options, status, environment, tags, check_interval_seconds, last_check_at, last_error, consecutive_failures, created_at, updated_at, is_active FROM targets WHERE org_id = $1"
    );
    if active_only {
        sql.push_str(" AND is_active = true");
    }
    if target_type.is_some() {
        sql.push_str(" AND target_type = $2");
    }
    sql.push_str(" ORDER BY name");

    let rows = if let Some(tt) = target_type {
        sqlx::query_as::<_, TargetRow>(&sql).bind(org_id).bind(tt).fetch_all(pool).await?
    } else {
        sqlx::query_as::<_, TargetRow>(&sql).bind(org_id).fetch_all(pool).await?
    };

    Ok(rows.into_iter().filter_map(|r| r.try_into().ok()).collect())
}

#[tracing::instrument(skip(pool), fields(db.system = "postgresql", db.operation = "SELECT", db.sql.table = "targets", otel.kind = "client"))]
pub async fn get_target(pool: &PgPool, target_id: Uuid) -> Result<Option<Target>, sqlx::Error> {
    let row = sqlx::query_as::<_, TargetRow>(
        "SELECT target_id, org_id, name, description, target_type, target_subtype, host, port, credentials, connection_options, status, environment, tags, check_interval_seconds, last_check_at, last_error, consecutive_failures, created_at, updated_at, is_active FROM targets WHERE target_id = $1"
    )
    .bind(target_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|r| r.try_into().ok()))
}

#[tracing::instrument(skip(pool), fields(db.system = "postgresql", db.operation = "SELECT", db.sql.table = "targets", otel.kind = "client"))]
pub async fn get_target_scoped(pool: &PgPool, target_id: Uuid, org_id: Uuid) -> Result<Option<Target>, sqlx::Error> {
    let row = sqlx::query_as::<_, TargetRow>(
        "SELECT target_id, org_id, name, description, target_type, target_subtype, host, port, credentials, connection_options, status, environment, tags, check_interval_seconds, last_check_at, last_error, consecutive_failures, created_at, updated_at, is_active FROM targets WHERE target_id = $1 AND org_id = $2"
    )
    .bind(target_id)
    .bind(org_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|r| r.try_into().ok()))
}

#[tracing::instrument(skip(pool), fields(db.system = "postgresql", db.operation = "UPDATE", db.sql.table = "targets", otel.kind = "client"))]
pub async fn update_target_status(
    pool: &PgPool,
    target_id: Uuid,
    status: &str,
    error: Option<&str>,
    failures: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE targets SET status = $2, last_check_at = now(), last_error = $3, consecutive_failures = $4, updated_at = now() WHERE target_id = $1"
    )
    .bind(target_id)
    .bind(status)
    .bind(error)
    .bind(failures)
    .execute(pool)
    .await?;
    Ok(())
}

#[tracing::instrument(skip(pool), fields(db.system = "postgresql", db.operation = "UPDATE", db.sql.table = "targets", otel.kind = "client"))]
pub async fn delete_target(pool: &PgPool, target_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE targets SET is_active = false, updated_at = now() WHERE target_id = $1")
        .bind(target_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct TargetRow {
    target_id: Uuid,
    org_id: Uuid,
    name: String,
    description: Option<String>,
    target_type: String,
    target_subtype: String,
    host: String,
    port: i32,
    credentials: Option<serde_json::Value>,
    connection_options: serde_json::Value,
    status: String,
    environment: String,
    tags: serde_json::Value,
    check_interval_seconds: i32,
    last_check_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
    consecutive_failures: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    is_active: bool,
}

impl TryFrom<TargetRow> for Target {
    type Error = String;

    fn try_from(r: TargetRow) -> Result<Self, String> {
        let target_subtype: TargetSubtype = serde_json::from_value(
            serde_json::Value::String(r.target_subtype.clone())
        ).map_err(|e| format!("Invalid target_subtype '{}': {}", r.target_subtype, e))?;

        let status = match r.status.as_str() {
            "pending" => TargetStatus::Pending,
            "connected" => TargetStatus::Connected,
            "healthy" => TargetStatus::Healthy,
            "warning" => TargetStatus::Warning,
            "critical" => TargetStatus::Critical,
            "unreachable" => TargetStatus::Unreachable,
            "disabled" => TargetStatus::Disabled,
            _ => TargetStatus::Pending,
        };

        let credentials: Option<TargetCredentials> = r.credentials
            .and_then(|v| serde_json::from_value(v).ok());

        Ok(Target {
            target_id: r.target_id,
            org_id: r.org_id,
            name: r.name,
            description: r.description,
            target_type: target_subtype.parent_type(),
            target_subtype,
            host: r.host,
            port: r.port as u16,
            credentials,
            connection_options: serde_json::from_value(r.connection_options).unwrap_or_default(),
            status,
            environment: r.environment,
            tags: serde_json::from_value(r.tags).unwrap_or_default(),
            check_interval_seconds: r.check_interval_seconds as u32,
            last_check_at: r.last_check_at,
            last_error: r.last_error,
            consecutive_failures: r.consecutive_failures as u32,
            created_at: r.created_at,
            updated_at: r.updated_at,
            is_active: r.is_active,
        })
    }
}
