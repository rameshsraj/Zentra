use axum::{extract::{State, Query}, Json};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_drilldown::{DrilldownContext, DrilldownLevel, DrilldownResult, TimeLevel};
use serde::Deserialize;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct DrilldownQuery {
    pub level: String,
    pub entity_id: Option<String>,
    pub entity_label: Option<String>,
    pub time_level: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub last_minutes: Option<i64>,
}

pub async fn query_drilldown(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<DrilldownQuery>,
) -> Result<Json<DrilldownResult>, ApiError> {
    let level = parse_level(&params.level)
        .ok_or_else(|| ApiError::BadRequest(format!("Invalid level: {}", params.level)))?;

    let time_level = params.time_level.as_deref()
        .and_then(parse_time_level)
        .unwrap_or(TimeLevel::Hour);

    let (from, to) = if let (Some(f), Some(t)) = (params.from, params.to) {
        (f, t)
    } else if let Some(mins) = params.last_minutes {
        let to = Utc::now();
        let from = to - chrono::Duration::minutes(mins);
        (from, to)
    } else {
        let to = Utc::now();
        let from = to - chrono::Duration::hours(1);
        (from, to)
    };

    let context = DrilldownContext {
        level,
        entity_id: params.entity_id,
        entity_label: params.entity_label,
        time_level,
        time_range: (from, to),
        filters: BTreeMap::new(),
    };

    let result = state.drilldown
        .query(org_id, &context)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(result))
}

fn parse_level(s: &str) -> Option<DrilldownLevel> {
    match s {
        "global" => Some(DrilldownLevel::Global),
        "network" => Some(DrilldownLevel::Network),
        "host" => Some(DrilldownLevel::Host),
        "server" => Some(DrilldownLevel::Server),
        "database" => Some(DrilldownLevel::Database),
        "storage" => Some(DrilldownLevel::Storage),
        "service" => Some(DrilldownLevel::Service),
        "process" => Some(DrilldownLevel::Process),
        "query" => Some(DrilldownLevel::Query),
        "metric" => Some(DrilldownLevel::Metric),
        _ => None,
    }
}

fn parse_time_level(s: &str) -> Option<TimeLevel> {
    match s {
        "year" => Some(TimeLevel::Year),
        "month" => Some(TimeLevel::Month),
        "day" => Some(TimeLevel::Day),
        "hour" => Some(TimeLevel::Hour),
        "minute" => Some(TimeLevel::Minute),
        "second" => Some(TimeLevel::Second),
        _ => None,
    }
}
