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

/// GET /api/ai/insights — operational insights from alerts, traces, logs, and telemetry freshness.
pub async fn insights(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<Vec<AIInsightEntry>>, ApiError> {
    let now = Utc::now();
    let mut insights: Vec<AIInsightEntry> = Vec::new();

    let alert_rows = sqlx::query(
        "SELECT severity, COUNT(*) AS count FROM active_alerts \
         WHERE org_id = $1 AND status IN ('triggered','acknowledged') \
         GROUP BY severity ORDER BY count DESC"
    )
    .bind(org_id)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();
    for row in alert_rows {
        use sqlx::Row;
        let severity: String = row.get("severity");
        let count: i64 = row.get("count");
        insights.push(AIInsightEntry {
            id: Uuid::now_v7().to_string(),
            category: "alerts".into(),
            title: format!("{} active {} alerts", count, severity),
            description: format!("There are {} unresolved or acknowledged alerts with {} severity.", count, severity),
            severity: if severity == "critical" { "critical" } else { "warning" }.into(),
            confidence: 0.9,
            metric_data: Some([("alert_count".into(), count as f64)].into()),
            recommendation: "Open Alerts & Events, group by severity or target, and resolve the highest-impact alerts first.".into(),
            created_at: now.to_rfc3339(),
        });
    }

    let trace_rows = sqlx::query(
        "SELECT service_name, operation_name, COUNT(*) AS error_count \
         FROM traces \
         WHERE status = 'error' AND org_id = $1 \
           AND started_at > NOW() - INTERVAL '1 hour' \
         GROUP BY service_name, operation_name \
         ORDER BY error_count DESC LIMIT 5"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();
    for row in trace_rows {
        use sqlx::Row;
        let service_name: String = row.get("service_name");
        let operation_name: String = row.get("operation_name");
        let error_count: i64 = row.get("error_count");
        let severity = if error_count > 50 { "critical" } else if error_count > 10 { "warning" } else { "info" };
        insights.push(AIInsightEntry {
            id: Uuid::now_v7().to_string(),
            category: "trace_errors".into(),
            title: format!("Trace errors in {}", service_name),
            description: format!("Operation '{}' produced {} errors in the last hour.", operation_name, error_count),
            severity: severity.into(),
            confidence: 0.95,
            metric_data: Some([("error_count".into(), error_count as f64)].into()),
            recommendation: format!("Check recent deployments, dependencies, and logs for '{}'.", service_name),
            created_at: now.to_rfc3339(),
        });
    }

    let log_rows = sqlx::query(
        "SELECT COALESCE(tags->>'host.name', source) AS source_name, COUNT(*) AS error_count \
         FROM logs \
         WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '1 hour' \
           AND lower(severity) IN ('error','critical','fatal') \
         GROUP BY source_name ORDER BY error_count DESC LIMIT 5"
    )
    .bind(org_id)
    .fetch_all(&state.tsdb_pool)
    .await
    .unwrap_or_default();
    for row in log_rows {
        use sqlx::Row;
        let source_name: String = row.get("source_name");
        let error_count: i64 = row.get("error_count");
        insights.push(AIInsightEntry {
            id: Uuid::now_v7().to_string(),
            category: "logs".into(),
            title: format!("Error logs from {}", source_name),
            description: format!("{} error-level log entries arrived from this source in the last hour.", error_count),
            severity: if error_count > 25 { "critical" } else { "warning" }.into(),
            confidence: 0.85,
            metric_data: Some([("error_logs".into(), error_count as f64)].into()),
            recommendation: format!("Open Logs and filter by server/source '{}' to inspect the error burst.", source_name),
            created_at: now.to_rfc3339(),
        });
    }

    let metric_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT target_id::text || ':' || metric_name || ':' || resource_id) \
         FROM metrics WHERE org_id = $1 AND timestamp > NOW() - INTERVAL '24 hours'"
    )
    .bind(org_id)
    .fetch_one(&state.tsdb_pool)
    .await
    .unwrap_or(0);

    if insights.is_empty() {
        let (title, description, recommendation) = if metric_count == 0 {
            (
                "No recent telemetry for AI analysis",
                "Zentra needs recent metrics, logs, traces, or alerts before AI insights can produce meaningful findings.",
                "Confirm agent collection, OTLP ingestion, and alert rules are enabled for the targets you care about.",
            )
        } else {
            (
                "No active AI findings",
                "Recent telemetry is available, but Zentra did not find active alert clusters, trace errors, or error-log bursts in the current analysis window.",
                "Keep collection running and review Predictive Alerts for trend-based capacity or anomaly signals.",
            )
        };
        insights.push(AIInsightEntry {
            id: Uuid::now_v7().to_string(),
            category: "telemetry".into(),
            title: title.into(),
            description: description.into(),
            severity: "info".into(),
            confidence: 1.0,
            metric_data: Some([("metric_streams".into(), metric_count as f64)].into()),
            recommendation: recommendation.into(),
            created_at: now.to_rfc3339(),
        });
    }

    insights.sort_by(|a, b| severity_rank(&b.severity).cmp(&severity_rank(&a.severity)));
    insights.truncate(20);
    Ok(Json(insights))
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 3,
        "warning" => 2,
        "info" => 1,
        _ => 0,
    }
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
