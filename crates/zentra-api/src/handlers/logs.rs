use axum::{
    extract::{State, Query, ws::{WebSocket, WebSocketUpgrade, Message}},
    response::IntoResponse,
    Json,
};
use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;
use zentra_query::{TimeRange, Pagination};
use serde::Deserialize;
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Deserialize)]
pub struct LogSearchQuery {
    pub query: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub last_minutes: Option<i64>,
    pub severity: Option<String>,
    pub source: Option<String>,
    pub target_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn search_logs(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<LogSearchQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let time_range = if let (Some(f), Some(t)) = (params.from, params.to) {
        TimeRange::new(f, t)
    } else if let Some(mins) = params.last_minutes {
        TimeRange::last_minutes(mins)
    } else {
        TimeRange::last_hours(1)
    };

    if let Err(msg) = time_range.validate() {
        return Err(ApiError::BadRequest(msg));
    }

    let pagination = Pagination {
        limit: params.limit.unwrap_or(50),
        offset: params.offset.unwrap_or(0),
    };

    let logs = state.log_query
        .search(
            org_id,
            params.query.as_deref(),
            &time_range,
            params.severity.as_deref(),
            params.source.as_deref(),
            params.target_id,
            &pagination,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "count": logs.len(),
        "logs": logs,
    })))
}

/// WebSocket endpoint for live log streaming
pub async fn live_logs_ws(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_live_logs(socket, state))
}

async fn handle_live_logs(mut socket: WebSocket, state: AppState) {
    let mut rx = state.event_bus.subscribe();

    loop {
        tokio::select! {
            // Forward log events to client
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        if event.event_type() == "logs_ingested" {
                            let json = serde_json::to_string(&event).unwrap_or_default();
                            if socket.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            // Check for client disconnect
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}
