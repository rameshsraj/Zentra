//! Mute Timings — Recurring maintenance windows for alert suppression
//! + Alert Backtesting — evaluate rules against historical data

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc, Datelike, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::state::AppState;
use crate::tenant::OrgId;

use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// Mute Timings
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuteTiming {
    pub id: Uuid,
    pub name: String,
    pub time_intervals: Vec<TimeInterval>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeInterval {
    pub weekdays: Option<Vec<u8>>,
    pub months: Option<Vec<u8>>,
    pub days_of_month: Option<Vec<u8>>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub location: Option<String>,
}

impl MuteTiming {
    pub fn is_active(&self, at: &DateTime<Utc>) -> bool {
        if self.time_intervals.is_empty() { return false; }
        self.time_intervals.iter().any(|ti| {
            let weekday_match = ti.weekdays.as_ref().map_or(true, |days| {
                let wd = at.weekday().num_days_from_monday() as u8;
                days.contains(&wd)
            });
            let month_match = ti.months.as_ref().map_or(true, |months| {
                months.contains(&(at.month() as u8))
            });
            let time_match = match (&ti.start_time, &ti.end_time) {
                (Some(start), Some(end)) => {
                    let cur = format!("{:02}:{:02}", at.hour(), at.minute());
                    cur >= *start && cur <= *end
                }
                _ => true,
            };
            weekday_match && month_match && time_match
        })
    }
}

fn mute_store() -> &'static RwLock<Vec<MuteTiming>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<MuteTiming>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(Vec::new()))
}

/// GET /api/notifications/mute-timings
pub async fn list_mute_timings() -> Json<Value> {
    let store = mute_store().read();
    Json(json!({ "mute_timings": *store, "count": store.len() }))
}

#[derive(Debug, Deserialize)]
pub struct CreateMuteTimingRequest {
    pub name: String,
    pub time_intervals: Vec<TimeInterval>,
}

/// POST /api/notifications/mute-timings
pub async fn create_mute_timing(
    Json(req): Json<CreateMuteTimingRequest>,
) -> Json<Value> {
    let now = Utc::now();
    let mt = MuteTiming {
        id: Uuid::now_v7(),
        name: req.name.clone(),
        time_intervals: req.time_intervals,
        created_at: now,
        updated_at: now,
    };
    let id = mt.id;
    mute_store().write().push(mt);
    Json(json!({ "id": id, "name": req.name, "status": "created" }))
}

/// DELETE /api/notifications/mute-timings/:id
pub async fn delete_mute_timing(
    Path(id): Path<Uuid>,
) -> Json<Value> {
    mute_store().write().retain(|m| m.id != id);
    Json(json!({ "id": id, "status": "deleted" }))
}

// ═══════════════════════════════════════════════════════════════════════════
// Alert Backtesting
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub metric_name: String,
    pub operator: String,
    pub threshold: f64,
    pub for_duration_seconds: Option<i64>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub step_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct BacktestViolation {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub would_fire: bool,
}

#[derive(Debug, Serialize)]
pub struct BacktestResult {
    pub metric_name: String,
    pub threshold: f64,
    pub operator: String,
    pub total_points: usize,
    pub violations: Vec<BacktestViolation>,
    pub violation_count: usize,
    pub violation_pct: f64,
    pub evaluated_from: DateTime<Utc>,
    pub evaluated_to: DateTime<Utc>,
}

/// POST /api/alerts/backtest
pub async fn backtest_alert(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<BacktestRequest>,
) -> Result<Json<BacktestResult>, ApiError> {
    let step = req.step_seconds.unwrap_or(60);

    // Fetch aggregated metric data
    let rows = sqlx::query_as::<_, MetricRow>(
        r#"SELECT time_bucket(make_interval(secs => $4), timestamp) AS bucket,
                  avg(value) AS avg_value
           FROM metrics
           WHERE org_id = $1 AND metric_name = $2 AND timestamp >= $3 AND timestamp < $5
           GROUP BY 1 ORDER BY 1"#
    )
    .bind(org_id)
    .bind(&req.metric_name)
    .bind(req.start)
    .bind(step as f64)
    .bind(req.end)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut violations = Vec::new();
    for row in &rows {
        let val = row.avg_value.unwrap_or(0.0);
        let fires = match req.operator.as_str() {
            ">" | "gt" => val > req.threshold,
            ">=" | "gte" => val >= req.threshold,
            "<" | "lt" => val < req.threshold,
            "<=" | "lte" => val <= req.threshold,
            "==" | "eq" => (val - req.threshold).abs() < 0.001,
            "!=" | "ne" => (val - req.threshold).abs() >= 0.001,
            _ => val > req.threshold,
        };
        violations.push(BacktestViolation {
            timestamp: row.bucket,
            value: val,
            would_fire: fires,
        });
    }

    let total = violations.len();
    let fire_count = violations.iter().filter(|v| v.would_fire).count();

    Ok(Json(BacktestResult {
        metric_name: req.metric_name,
        threshold: req.threshold,
        operator: req.operator,
        total_points: total,
        violation_count: fire_count,
        violation_pct: if total > 0 { (fire_count as f64 / total as f64) * 100.0 } else { 0.0 },
        violations,
        evaluated_from: req.start,
        evaluated_to: req.end,
    }))
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct MetricRow {
    bucket: DateTime<Utc>,
    avg_value: Option<f64>,
}
