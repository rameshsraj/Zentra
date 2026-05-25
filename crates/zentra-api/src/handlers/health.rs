use axum::{extract::State, Json};
use crate::state::AppState;
use serde_json::json;

pub async fn health_check(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let db_ok = zentra_db::pool::health_check(&state.pool).await;
    Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "database": db_ok,
        "subscribers": state.event_bus.subscriber_count(),
    }))
}
