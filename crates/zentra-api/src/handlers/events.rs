use axum::{
    extract::{State, Query},
    response::IntoResponse,
    Json,
};
use crate::state::AppState;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct EventQuery {
    pub event_type: Option<String>,
    pub limit: Option<usize>,
}

pub async fn recent_events(
    State(state): State<AppState>,
    Query(params): Query<EventQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);

    let events = if let Some(ref event_type) = params.event_type {
        state.event_history.filter_by_type(event_type)
    } else {
        state.event_history.recent(limit)
    };

    Json(serde_json::json!({
        "events": events,
        "count": events.len(),
        "total_published": state.event_history.total_published(),
    }))
}

pub async fn event_stats(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let stats = state.event_history.stats();
    let total_published = state.event_history.total_published();
    Json(serde_json::json!({
        "total_published": total_published,
        "subscribers": state.event_bus.subscriber_count(),
        "events_per_second": 0.0,
        "metrics_collected": stats.metrics_collected,
        "logs_ingested": stats.logs_ingested,
        "alerts_triggered": stats.alerts_triggered,
        "alerts_resolved": stats.alerts_resolved,
        "target_status_changed": stats.target_status_changed,
        "plugin_started": stats.plugin_started,
        "plugin_stopped": stats.plugin_stopped,
        "health_checks": stats.health_checks,
        "threats_detected": stats.threats_detected,
        "capture_stats": stats.capture_stats,
        "anomalies_detected": stats.anomalies_detected,
        "trend_alerts_generated": stats.trend_alerts_generated,
        "last_event_at": stats.last_event_at,
        "by_type": {
            "metrics_collected": stats.metrics_collected,
            "logs_ingested": stats.logs_ingested,
            "alerts_triggered": stats.alerts_triggered,
            "alerts_resolved": stats.alerts_resolved,
            "target_status_changed": stats.target_status_changed,
            "plugin_started": stats.plugin_started,
            "plugin_stopped": stats.plugin_stopped,
            "health_checks": stats.health_checks,
            "threats_detected": stats.threats_detected,
            "capture_stats": stats.capture_stats,
            "anomalies_detected": stats.anomalies_detected,
            "trend_alerts_generated": stats.trend_alerts_generated
        },
        "by_severity": {
            "info": stats.metrics_collected + stats.logs_ingested + stats.plugin_started + stats.plugin_stopped + stats.health_checks,
            "warning": stats.target_status_changed + stats.anomalies_detected + stats.trend_alerts_generated,
            "critical": stats.alerts_triggered + stats.threats_detected
        }
    }))
}
