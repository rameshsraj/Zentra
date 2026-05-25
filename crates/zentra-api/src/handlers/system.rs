use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use crate::state::AppState;
use crate::tenant::OrgId;
use serde::Serialize;
use chrono::Utc;

#[derive(Serialize)]
pub struct SystemStatus {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: f64,
    pub org_id: uuid::Uuid,
    pub event_bus_subscribers: usize,
    pub timestamp: chrono::DateTime<Utc>,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub async fn system_status(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> impl IntoResponse {
    let start = START_TIME.get_or_init(std::time::Instant::now);
    let uptime = start.elapsed().as_secs_f64();

    Json(SystemStatus {
        status: "operational",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        org_id: org_id,
        event_bus_subscribers: state.event_bus.subscriber_count(),
        timestamp: Utc::now(),
    })
}

pub async fn db_health(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let healthy = zentra_db::pool::health_check(&state.pool).await;
    Json(serde_json::json!({
        "database": if healthy { "healthy" } else { "unhealthy" },
        "timestamp": Utc::now(),
    }))
}
