use axum::{extract::{State, Query}, Json};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sqlx::Row;

use crate::error::ApiError;
use crate::tenant::OrgId;

#[derive(Deserialize)]
pub struct ForecastQuery {
    pub metric_name: Option<String>,
    pub target_id: Option<Uuid>,
    pub max_hours: Option<f64>,
}

#[derive(Deserialize)]
pub struct AnomalyQuery {
    pub severity: Option<String>,
    pub target_id: Option<Uuid>,
    pub hours: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct TrendAlertQuery {
    pub severity: Option<String>,
    pub alert_type: Option<String>,
    pub resolved: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct ForecastResponse {
    pub forecast_id: String,
    pub target_id: String,
    pub metric_name: String,
    pub resource_id: String,
    pub current_value: f64,
    pub predicted_value: f64,
    pub rate_per_hour: f64,
    pub hours_to_threshold: Option<f64>,
    pub threshold: f64,
    pub confidence: f64,
    pub severity: String,
    pub description: String,
    pub created_at: String,
}

pub async fn list_forecasts(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ForecastQuery>,
) -> Result<Json<Vec<ForecastResponse>>, ApiError> {
    let rows = sqlx::query(
        r#"WITH ranked AS (
               SELECT m.*, row_number() OVER (PARTITION BY target_id, metric_name, resource_id ORDER BY timestamp DESC) AS rn,
                      first_value(value) OVER (PARTITION BY target_id, metric_name, resource_id ORDER BY timestamp DESC) AS current_value,
                      first_value(value) OVER (PARTITION BY target_id, metric_name, resource_id ORDER BY timestamp ASC) AS oldest_value,
                      min(timestamp) OVER (PARTITION BY target_id, metric_name, resource_id) AS first_ts,
                      max(timestamp) OVER (PARTITION BY target_id, metric_name, resource_id) AS last_ts,
                      count(*) OVER (PARTITION BY target_id, metric_name, resource_id) AS sample_count
               FROM metrics m
               WHERE m.org_id = $1
                 AND m.timestamp > NOW() - INTERVAL '6 hours'
                 AND ($2::text IS NULL OR m.metric_name = $2)
                 AND ($3::uuid IS NULL OR m.target_id = $3)
           ), series AS (
               SELECT target_id, metric_name, resource_id, current_value, oldest_value, first_ts, last_ts, sample_count
               FROM ranked WHERE rn = 1 AND sample_count >= 2
           )
           SELECT target_id, metric_name, resource_id, current_value,
                  (current_value - oldest_value) / GREATEST(EXTRACT(EPOCH FROM (last_ts - first_ts)) / 3600.0, 1.0) AS rate_per_hour,
                  sample_count
           FROM series
           ORDER BY abs((current_value - oldest_value) / GREATEST(EXTRACT(EPOCH FROM (last_ts - first_ts)) / 3600.0, 1.0)) DESC
           LIMIT 50"#,
    )
    .bind(org_id)
    .bind(params.metric_name.as_deref())
    .bind(params.target_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let forecasts = rows.into_iter().filter_map(|r| {
        let current = r.get::<f64, _>("current_value");
        let rate = r.get::<f64, _>("rate_per_hour");
        let metric = r.get::<String, _>("metric_name");
        let threshold = threshold_for_metric(&metric, current, rate);
        let hours_to_threshold = if rate.abs() > 0.000001 {
            let hours = (threshold - current) / rate;
            if hours.is_finite() && hours >= 0.0 { Some(hours) } else { None }
        } else { None };
        if let (Some(max_hours), Some(hours)) = (params.max_hours, hours_to_threshold) {
            if hours > max_hours { return None; }
        }
        let confidence = ((r.get::<i64, _>("sample_count") as f64) / 60.0).clamp(0.35, 0.95);
        let severity = match hours_to_threshold {
            Some(h) if h <= 24.0 => "critical",
            Some(h) if h <= 168.0 => "warning",
            _ => "info",
        }.to_string();
        let predicted = current + rate * 24.0;
        Some(ForecastResponse {
            forecast_id: format!("forecast-{}-{}", r.get::<Uuid, _>("target_id"), metric.replace('.', "-")),
            target_id: r.get::<Uuid, _>("target_id").to_string(),
            metric_name: metric.clone(),
            resource_id: r.get::<String, _>("resource_id"),
            current_value: current,
            predicted_value: predicted,
            rate_per_hour: rate,
            hours_to_threshold,
            threshold,
            confidence,
            severity,
            description: forecast_description(&metric, current, predicted, hours_to_threshold),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }).collect();

    Ok(Json(forecasts))
}

#[derive(Serialize)]
pub struct AnomalyResponse {
    pub anomaly_id: String,
    pub target_id: String,
    pub metric_name: String,
    pub resource_id: String,
    pub value: f64,
    pub baseline_mean: f64,
    pub baseline_stddev: f64,
    pub z_score: f64,
    pub severity: String,
    pub direction: String,
    pub description: String,
    pub detected_at: String,
}

pub async fn list_anomalies(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<AnomalyQuery>,
) -> Result<Json<Vec<AnomalyResponse>>, ApiError> {
    let hours = params.hours.unwrap_or(24).clamp(1, 168);
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let rows = sqlx::query(
        r#"WITH baseline AS (
               SELECT target_id, metric_name, resource_id, avg(value) AS mean, stddev_pop(value) AS stddev, count(*) AS samples
               FROM metrics
               WHERE org_id = $1 AND timestamp > NOW() - ($2::int * INTERVAL '1 hour')
               GROUP BY target_id, metric_name, resource_id
           ), latest AS (
               SELECT DISTINCT ON (target_id, metric_name, resource_id)
                      target_id, metric_name, resource_id, value, timestamp
               FROM metrics
               WHERE org_id = $1
                 AND timestamp > NOW() - INTERVAL '1 hour'
                 AND ($3::uuid IS NULL OR target_id = $3)
               ORDER BY target_id, metric_name, resource_id, timestamp DESC
           )
           SELECT l.target_id, l.metric_name, l.resource_id, l.value, l.timestamp,
                  b.mean, GREATEST(COALESCE(b.stddev, 0), 0.000001) AS stddev,
                  abs((l.value - b.mean) / GREATEST(COALESCE(b.stddev, 0), 0.000001)) AS z_score
           FROM latest l
           JOIN baseline b USING (target_id, metric_name, resource_id)
           WHERE b.samples >= 5
             AND abs((l.value - b.mean) / GREATEST(COALESCE(b.stddev, 0), 0.000001)) >= 2.0
           ORDER BY z_score DESC
           LIMIT $4"#,
    )
    .bind(org_id)
    .bind(hours as i32)
    .bind(params.target_id)
    .bind(limit)
    .fetch_all(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let anomalies = rows.into_iter().filter_map(|r| {
        let z = r.get::<f64, _>("z_score");
        let severity = if z >= 5.0 { "critical" } else if z >= 4.0 { "high" } else if z >= 3.0 { "medium" } else { "low" }.to_string();
        if let Some(filter) = params.severity.as_deref() {
            if filter != severity { return None; }
        }
        let value = r.get::<f64, _>("value");
        let mean = r.get::<f64, _>("mean");
        let metric = r.get::<String, _>("metric_name");
        Some(AnomalyResponse {
            anomaly_id: format!("anomaly-{}-{}", r.get::<Uuid, _>("target_id"), metric.replace('.', "-")),
            target_id: r.get::<Uuid, _>("target_id").to_string(),
            metric_name: metric.clone(),
            resource_id: r.get::<String, _>("resource_id"),
            value,
            baseline_mean: mean,
            baseline_stddev: r.get::<f64, _>("stddev"),
            z_score: z,
            severity,
            direction: if value >= mean { "above" } else { "below" }.to_string(),
            description: format!("{} is {:.1} standard deviations {} its recent baseline.", metric, z, if value >= mean { "above" } else { "below" }),
            detected_at: r.get::<chrono::DateTime<chrono::Utc>, _>("timestamp").to_rfc3339(),
        })
    }).collect();

    Ok(Json(anomalies))
}

#[derive(Serialize)]
pub struct TrendAlertResponse {
    pub alert_id: String,
    pub target_id: String,
    pub metric_name: String,
    pub resource_id: String,
    pub alert_type: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub current_value: f64,
    pub predicted_value: Option<f64>,
    pub hours_to_breach: Option<f64>,
    pub confidence: f64,
    pub recommendation: String,
    pub created_at: String,
}

pub async fn list_trend_alerts(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<TrendAlertQuery>,
) -> Result<Json<Vec<TrendAlertResponse>>, ApiError> {
    let forecasts = list_forecasts(State(state), OrgId(org_id), Query(ForecastQuery {
        metric_name: None,
        target_id: None,
        max_hours: Some(168.0),
    })).await?.0;

    let mut alerts: Vec<TrendAlertResponse> = forecasts.into_iter().filter_map(|f| {
        let hours = f.hours_to_threshold?;
        let alert_type = "capacity_exhaustion".to_string();
        if let Some(ref sev) = params.severity { if *sev != f.severity { return None; } }
        if let Some(ref typ) = params.alert_type { if *typ != alert_type { return None; } }
        if params.resolved == Some(true) { return None; }
        Some(TrendAlertResponse {
            alert_id: format!("trend-{}", f.forecast_id),
            target_id: f.target_id,
            metric_name: f.metric_name.clone(),
            resource_id: f.resource_id,
            alert_type,
            severity: f.severity,
            title: format!("{} may breach threshold", f.metric_name),
            description: f.description,
            current_value: f.current_value,
            predicted_value: Some(f.predicted_value),
            hours_to_breach: Some(hours),
            confidence: f.confidence,
            recommendation: "Review recent growth, validate capacity, and tune the alert threshold if this is expected behavior.".into(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
    }).collect();
    alerts.sort_by(|a, b| a.hours_to_breach.partial_cmp(&b.hours_to_breach).unwrap_or(std::cmp::Ordering::Equal));
    alerts.truncate(params.limit.unwrap_or(50).clamp(1, 200) as usize);
    Ok(Json(alerts))
}

#[derive(Serialize)]
pub struct PredictionSummary {
    pub active_anomalies: i64,
    pub active_forecasts: i64,
    pub active_trend_alerts: i64,
    pub models_trained: i64,
    pub metrics_monitored: i64,
    pub last_evaluation: String,
}

pub async fn prediction_summary(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<PredictionSummary>, ApiError> {
    let row = sqlx::query(
        r#"WITH metric_streams AS (
               SELECT COUNT(DISTINCT target_id::text || ':' || metric_name || ':' || resource_id) AS streams,
                      MAX(timestamp) AS last_seen
               FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '24 hours'
           ), anomaly_count AS (
               SELECT COUNT(*) AS count FROM (
                   WITH baseline AS (
                       SELECT target_id, metric_name, resource_id, avg(value) AS mean, stddev_pop(value) AS stddev, count(*) AS samples
                       FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '24 hours'
                       GROUP BY target_id, metric_name, resource_id
                   ), latest AS (
                       SELECT DISTINCT ON (target_id, metric_name, resource_id) target_id, metric_name, resource_id, value
                       FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 hour'
                       ORDER BY target_id, metric_name, resource_id, timestamp DESC
                   )
                   SELECT 1 FROM latest l JOIN baseline b USING (target_id, metric_name, resource_id)
                   WHERE b.samples >= 5 AND abs((l.value - b.mean) / GREATEST(COALESCE(b.stddev, 0), 0.000001)) >= 2.0
               ) x
           )
           SELECT COALESCE(ms.streams, 0)::bigint AS streams,
                  COALESCE(ac.count, 0)::bigint AS anomalies,
                  ms.last_seen
           FROM metric_streams ms CROSS JOIN anomaly_count ac"#,
    )
    .bind(org_id)
    .fetch_one(&state.tsdb_pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let streams = row.get::<i64, _>("streams");
    let anomalies = row.get::<i64, _>("anomalies");
    let last_seen = row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_seen").ok().flatten().unwrap_or_else(chrono::Utc::now);
    Ok(Json(PredictionSummary {
        active_anomalies: anomalies,
        active_forecasts: streams.min(50),
        active_trend_alerts: 0,
        models_trained: if streams > 0 { 1 } else { 0 },
        metrics_monitored: streams,
        last_evaluation: last_seen.to_rfc3339(),
    }))
}

fn threshold_for_metric(metric: &str, current: f64, rate: f64) -> f64 {
    let lower = metric.to_ascii_lowercase();
    if lower.contains("percent") || lower.contains("usage") || lower.contains("util") {
        90.0
    } else if lower.contains("error") || lower.contains("fail") {
        (current.max(1.0) * 1.5).max(current + rate.abs() * 6.0)
    } else {
        current + rate.abs().max(1.0) * 24.0
    }
}

fn forecast_description(metric: &str, current: f64, predicted: f64, hours_to_threshold: Option<f64>) -> String {
    match hours_to_threshold {
        Some(hours) => format!("{} is trending from {:.2} toward {:.2} and may breach in {:.1} hours.", metric, current, predicted, hours),
        None => format!("{} is stable or not trending toward its inferred threshold. Current {:.2}, 24h projection {:.2}.", metric, current, predicted),
    }
}