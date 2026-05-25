use crate::{
    DrilldownContext, DrilldownItem, DrilldownLevel, DrilldownResult,
    breadcrumb::Breadcrumb,
};
use zentra_db::DbPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::debug;

/// The drill-down engine resolves navigation requests.
///
/// Given a context (level, entity, time range, filters), it queries the DB
/// and returns items at the current level + breadcrumb + available children.
pub struct DrilldownEngine {
    pool: DbPool,
}

impl DrilldownEngine {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Execute a drill-down query at the given context
    pub async fn query(
        &self,
        org_id: Uuid,
        context: &DrilldownContext,
    ) -> Result<DrilldownResult, zentra_core::Error> {
        let items = match context.level {
            DrilldownLevel::Global => self.query_global(org_id).await?,
            DrilldownLevel::Host => self.query_hosts(org_id).await?,
            DrilldownLevel::Server => self.query_by_type(org_id, "server").await?,
            DrilldownLevel::Database => self.query_by_type(org_id, "database").await?,
            DrilldownLevel::Storage => self.query_by_type(org_id, "storage").await?,
            DrilldownLevel::Network => self.query_by_type(org_id, "network").await?,
            DrilldownLevel::Metric => {
                if let Some(ref entity_id) = context.entity_id {
                    self.query_metrics_for_entity(org_id, entity_id, &context.time_range).await?
                } else {
                    vec![]
                }
            }
            _ => vec![],
        };

        let breadcrumb = Breadcrumb::build(context);
        let available_children = context.level.children();

        debug!(level = context.level.as_str(), items = items.len(), "Drill-down query executed");

        Ok(DrilldownResult {
            context: context.clone(),
            items,
            breadcrumb,
            available_children,
        })
    }

    async fn query_global(&self, org_id: Uuid) -> Result<Vec<DrilldownItem>, zentra_core::Error> {
        // Count targets by type
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT target_type, count(*) FROM targets WHERE org_id = $1 AND is_active = true GROUP BY target_type ORDER BY target_type"
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(zentra_core::Error::from)?;

        Ok(rows.into_iter().map(|(ttype, count)| DrilldownItem {
            id: ttype.clone(),
            label: format!("{} ({})", capitalize(&ttype), count),
            item_type: ttype,
            status: "active".into(),
            summary: serde_json::json!({ "count": count }),
            has_children: true,
        }).collect())
    }

    async fn query_hosts(&self, org_id: Uuid) -> Result<Vec<DrilldownItem>, zentra_core::Error> {
        let rows = sqlx::query_as::<_, (String, i64, String)>(
            "SELECT host, count(*), max(status) FROM targets WHERE org_id = $1 AND is_active = true GROUP BY host ORDER BY host"
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(zentra_core::Error::from)?;

        Ok(rows.into_iter().map(|(host, count, status)| DrilldownItem {
            id: host.clone(),
            label: host,
            item_type: "host".into(),
            status,
            summary: serde_json::json!({ "targets": count }),
            has_children: true,
        }).collect())
    }

    async fn query_by_type(&self, org_id: Uuid, target_type: &str) -> Result<Vec<DrilldownItem>, zentra_core::Error> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, String)>(
            "SELECT target_id, name, status, host FROM targets WHERE org_id = $1 AND target_type = $2 AND is_active = true ORDER BY name"
        )
        .bind(org_id)
        .bind(target_type)
        .fetch_all(&self.pool)
        .await
        .map_err(zentra_core::Error::from)?;

        Ok(rows.into_iter().map(|(id, name, status, host)| DrilldownItem {
            id: id.to_string(),
            label: name,
            item_type: target_type.to_string(),
            status,
            summary: serde_json::json!({ "host": host }),
            has_children: true,
        }).collect())
    }

    async fn query_metrics_for_entity(
        &self,
        org_id: Uuid,
        entity_id: &str,
        time_range: &(DateTime<Utc>, DateTime<Utc>),
    ) -> Result<Vec<DrilldownItem>, zentra_core::Error> {
        // Get distinct metric names for this entity
        let target_id: Uuid = entity_id.parse().unwrap_or_default();
        let rows = sqlx::query_as::<_, (String, f64)>(
            "SELECT metric_name, avg(value) FROM metrics WHERE org_id = $1 AND target_id = $2 AND timestamp >= $3 AND timestamp < $4 GROUP BY metric_name ORDER BY metric_name"
        )
        .bind(org_id)
        .bind(target_id)
        .bind(time_range.0)
        .bind(time_range.1)
        .fetch_all(&self.pool)
        .await
        .map_err(zentra_core::Error::from)?;

        Ok(rows.into_iter().map(|(name, avg)| DrilldownItem {
            id: name.clone(),
            label: name,
            item_type: "metric".into(),
            status: "active".into(),
            summary: serde_json::json!({ "avg": format!("{:.2}", avg) }),
            has_children: false,
        }).collect())
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
