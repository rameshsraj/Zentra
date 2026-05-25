use axum::{Router, routing::{get, post}};
use crate::state::AppState;
use crate::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        // AI / Chatbot
        .route("/api/ai/chat", post(handlers::ai::chat))
        .route("/api/ai/chat/history", get(handlers::ai::chat_history))
        .route("/api/ai/insights", get(handlers::ai::insights))

        // K-means ML endpoints
        .route("/api/ai/anomalies", get(handlers::ai::anomaly_scores))
        .route("/api/ai/ml/status", get(handlers::ai::ml_model_status))
        .route("/api/ai/ml/clusters", get(handlers::ai::ml_clusters))
        .route("/api/ai/ml/train", post(handlers::ai::ml_retrain))

        // MCP (Model Context Protocol) Server
        .route("/api/mcp/tools", get(handlers::mcp::list_tools))
        .route("/api/mcp/call", post(handlers::mcp::call_tool))
        .route("/api/mcp/chat", post(handlers::mcp::mcp_chat))

        // RCA (Root Cause Analysis)
        .route("/api/rca/analyze", post(handlers::rca::analyze))
        .route("/api/rca/history", get(handlers::rca::history))

        // Predictions & AI Engine
        .route("/api/predictions/forecasts", get(handlers::predictions::list_forecasts))
        .route("/api/predictions/anomalies", get(handlers::predictions::list_anomalies))
        .route("/api/predictions/trend-alerts", get(handlers::predictions::list_trend_alerts))
        .route("/api/predictions/summary", get(handlers::predictions::prediction_summary))

        // Traces & OTLP
        .route("/api/traces", get(handlers::traces::search_traces))
        .route("/api/traces/services", get(handlers::traces::list_services))
        .route("/api/traces/operations", get(handlers::traces::list_operations))
        .route("/api/traces/:traceId", get(handlers::traces::get_trace))
        .route("/api/v1/traces", post(handlers::traces::ingest_traces))

        // SW P10: TraceQL Query Language + Tempo-compatible endpoints
        .route("/api/query/traceql", post(handlers::traces::traceql_query))
        .route("/api/query/traceql/validate", get(handlers::traces::traceql_validate))
        .route("/tempo/api/search", get(handlers::traces::tempo_search))
        .route("/tempo/api/traces/:traceId", get(handlers::traces::tempo_trace))

        // SW P08: LLM / GenAI Observability
        .route("/api/llm/overview", get(handlers::llm_observability::llm_overview))
        .route("/api/llm/models", get(handlers::llm_observability::llm_models))
        .route("/api/llm/cost-trend", get(handlers::llm_observability::llm_cost_trend))
        .route("/api/llm/latency-distribution", get(handlers::llm_observability::llm_latency_distribution))
        .route("/api/llm/token-efficiency", get(handlers::llm_observability::llm_token_efficiency))

        // Global Search
        .route("/api/search", get(handlers::search::global_search))

        // Drill-down
        .route("/api/drilldown", get(handlers::drilldown::query_drilldown))

        // Correlation
        .route("/api/correlation/logs-for-metric", get(handlers::correlation::logs_for_metric))
        .route("/api/correlation/metrics-for-log", get(handlers::correlation::metrics_for_log))

        // Event History
        .route("/api/events", get(handlers::events::recent_events))
        .route("/api/events/stats", get(handlers::events::event_stats))

        // WebSocket for real-time events
        .route("/api/ws/events", get(crate::ws::ws_handler))
}
