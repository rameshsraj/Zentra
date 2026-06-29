//! AI / Chatbot and Insights API handlers for Zentra.

use axum::{extract::{State, Query}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;
use crate::tenant::OrgId;
use crate::error::ApiError;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponsePayload {
    pub text: String,
    pub intent: String,
    pub entities: std::collections::HashMap<String, String>,
    pub tool_used: Option<String>,
    pub tool_result: Option<serde_json::Value>,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct ChatMessageEntry {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub tool_used: Option<String>,
    pub tool_result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ChatHistoryQuery {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct AIInsightEntry {
    pub id: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub confidence: f64,
    pub metric_data: Option<std::collections::HashMap<String, f64>>,
    pub recommendation: String,
    pub created_at: String,
}

/// POST /api/ai/chat — send chat message
pub async fn chat(
    State(_state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatResponsePayload>, ApiError> {
    let now = Utc::now();
    return Ok(Json(ChatResponsePayload {
        text: format!("Live mode: AI chat processing '{}'. Use the MCP chat endpoint (/api/mcp/chat) for full tool-integrated responses.", req.message),
        intent: "live_redirect".into(),
        entities: std::collections::HashMap::new(),
        tool_used: None,
        tool_result: None,
        timestamp: now.to_rfc3339(),
    }));
}

/// GET /api/ai/chat/history — get chat history for session
pub async fn chat_history(
    State(_state): State<AppState>,
    Query(params): Query<ChatHistoryQuery>,
) -> Result<Json<Vec<ChatMessageEntry>>, ApiError> {
    let now = Utc::now();
    return Ok(Json(vec![]));
}

/// GET /api/ai/insights — get AI-generated insights derived from OTEL trace errors
pub async fn insights(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Vec<AIInsightEntry>>, ApiError> {
    let now = Utc::now();

    #[derive(sqlx::FromRow)]
    struct ErrorRow {
        service_name: String,
        operation_name: String,
        error_count: i64,
    }

    let rows = sqlx::query_as::<_, ErrorRow>(
        "SELECT service_name, operation_name, COUNT(*) AS error_count \
         FROM traces \
         WHERE status = 'error' AND org_id = $1 \
           AND started_at > NOW() - INTERVAL '1 hour' \
         GROUP BY service_name, operation_name \
         ORDER BY error_count DESC \
         LIMIT 10"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();

    let insights: Vec<AIInsightEntry> = rows.into_iter().map(|r| {
        let severity = if r.error_count > 50 { "critical" } else if r.error_count > 10 { "warning" } else { "info" };
        AIInsightEntry {
            id: Uuid::now_v7().to_string(),
            category: "trace_errors".into(),
            title: format!("High error rate in {}", r.service_name),
            description: format!(
                "Operation '{}' on service '{}' produced {} errors in the last hour.",
                r.operation_name, r.service_name, r.error_count
            ),
            severity: severity.into(),
            confidence: 0.95,
            metric_data: Some([("error_count".into(), r.error_count as f64)].into()),
            recommendation: format!(
                "Investigate recent deployments or dependency failures for '{}'.",
                r.service_name
            ),
            created_at: now.to_rfc3339(),
        }
    }).collect();

    Ok(Json(insights))
}

// ============================================================================
// P6: K-means ML wiring + ML endpoints
// ============================================================================

#[derive(Debug, Serialize)]
pub struct AnomalyScore {
    pub metric_name: String,
    pub host: String,
    pub anomaly_score: f64,
    pub is_anomalous: bool,
    pub cluster_id: u32,
    pub distance_from_centroid: f64,
    pub timestamp: String,
}

/// GET /api/ai/anomalies — real-time anomaly scores from k-means
pub async fn anomaly_scores(
    State(_state): State<AppState>,
) -> Json<Vec<AnomalyScore>> {
    let now = Utc::now();
    return Json(vec![]);
}

#[derive(Debug, Serialize)]
pub struct MLModelStatus {
    pub model_name: String,
    pub status: String,
    pub last_trained: String,
    pub training_window: String,
    pub num_clusters: u32,
    pub total_metrics_tracked: u32,
    pub anomaly_threshold: f64,
    pub metrics_above_threshold: u32,
    pub host_anomaly_rate: f64,
}

/// GET /api/ai/ml/status — ML model training status
pub async fn ml_model_status(
    State(_state): State<AppState>,
) -> Json<MLModelStatus> {
    let now = Utc::now();
    return Json(MLModelStatus {
        model_name: "Zentra K-means Anomaly Detector".into(),
        status: "not_configured".into(),
        last_trained: now.to_rfc3339(),
        training_window: "N/A".into(),
        num_clusters: 0,
        total_metrics_tracked: 0,
        anomaly_threshold: 0.0,
        metrics_above_threshold: 0,
        host_anomaly_rate: 0.0,
    });
}

#[derive(Debug, Serialize)]
pub struct ClusterInfo {
    pub cluster_id: u32,
    pub centroid: Vec<f64>,
    pub member_count: u32,
    pub avg_distance: f64,
    pub label: String,
}

/// GET /api/ai/ml/clusters — cluster info
pub async fn ml_clusters(
    State(_state): State<AppState>,
) -> Json<Vec<ClusterInfo>> {
    return Json(vec![]);
}

/// POST /api/ai/ml/train — trigger re-training
pub async fn ml_retrain(
    State(_state): State<AppState>,
) -> Json<serde_json::Value> {
    return Json(serde_json::json!({
        "status": "not_available",
        "message": "ML training not available in live mode — no ML engine configured"
    }));
}
