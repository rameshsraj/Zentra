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
    Json(state.event_history.stats())
}
