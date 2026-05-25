use axum::{
    extract::{Query, State, ws::{WebSocket, WebSocketUpgrade, Message}},
    http::StatusCode,
    response::IntoResponse,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use crate::auth_middleware::JwtClaims;
use crate::state::AppState;
use tracing::{info, debug, warn};

#[derive(Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

/// Main WebSocket handler for real-time event streaming to the frontend.
/// Validates the Bearer token supplied as `?token=<jwt>` (browsers cannot set
/// Authorization headers on WebSocket connections) before accepting the upgrade.
pub async fn ws_handler(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let token = match query.token.as_deref().filter(|t| !t.is_empty()) {
        Some(t) => t.to_owned(),
        None => {
            warn!("WebSocket upgrade rejected: missing token");
            return (StatusCode::UNAUTHORIZED, "missing token").into_response();
        }
    };

    let secret = std::env::var("ZENTRA_JWT_SECRET").unwrap_or_default();
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 30;

    match decode::<JwtClaims>(&token, &DecodingKey::from_secret(secret.as_bytes()), &validation) {
        Ok(_) => ws.on_upgrade(move |socket| handle_ws(socket, state)),
        Err(e) => {
            warn!("WebSocket upgrade rejected: invalid token: {}", e);
            (StatusCode::UNAUTHORIZED, "invalid token").into_response()
        }
    }
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    info!("WebSocket client connected");
    let mut rx = state.event_bus.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        let json = serde_json::to_string(&event).unwrap_or_default();
                        if socket.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!(skipped = n, "WebSocket client lagged");
                        continue;
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Text(text))) => {
                        // Client can send filter commands
                        debug!(message = %text, "WebSocket message from client");
                    }
                    _ => {}
                }
            }
        }
    }

    info!("WebSocket client disconnected");
}
