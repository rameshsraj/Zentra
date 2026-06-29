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

/// GET /api/logs/:log_id/traces — return OTEL spans correlated with a log entry's trace_id
pub async fn log_traces(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    axum::extract::Path(log_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let trace_id: Option<String> = sqlx::query_scalar(
        "SELECT trace_id FROM log_entries WHERE id = $1 AND org_id = $2 LIMIT 1"
    )
    .bind(&log_id)
    .bind(org_id)
    .fetch_optional(&state.tsdb_pool)
    .await
    .unwrap_or(None)
    .flatten();

    let trace_id = match trace_id {
        Some(t) if !t.is_empty() => t,
        _ => return Err(ApiError::NotFound("No trace_id associated with this log entry".into())),
    };

    let spans: Vec<serde_json::Value> = sqlx::query(
        "SELECT span_id, trace_id, parent_span_id, operation_name, service_name, duration_us, started_at, status
         FROM traces WHERE trace_id = $1 AND org_id = $2 ORDER BY started_at ASC"
    )
    .bind(&trace_id)
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| {
        use sqlx::Row;
        serde_json::json!({
            "span_id": r.get::<String, _>("span_id"),
            "trace_id": r.get::<String, _>("trace_id"),
            "parent_span_id": r.get::<Option<String>, _>("parent_span_id"),
            "operation_name": r.get::<String, _>("operation_name"),
            "service_name": r.get::<String, _>("service_name"),
            "duration_us": r.get::<i64, _>("duration_us"),
            "started_at": r.get::<chrono::DateTime<Utc>, _>("started_at"),
            "status": r.get::<String, _>("status"),
        })
    })
    .collect();

    Ok(Json(serde_json::json!({
        "trace_id": trace_id,
        "span_count": spans.len(),
        "spans": spans,
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
