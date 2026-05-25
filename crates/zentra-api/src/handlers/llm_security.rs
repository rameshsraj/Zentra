use axum::{extract::{State, Path, Query}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::state::AppState;
use crate::error::ApiError;

// ═══════════════════════════════════════════════════════════════════════════
// Models
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmEvent {
    pub id: String,
    pub event_type: String,  // "prompt_injection", "pii_leak", "toxicity", "hallucination", "jailbreak"
    pub severity: String,    // "critical", "high", "medium", "low"
    pub model: String,
    pub prompt_preview: String,
    pub response_preview: Option<String>,
    pub score: f64,
    pub details: Value,
    pub source_ip: Option<String>,
    pub user_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRule {
    pub id: String,
    pub name: String,
    pub rule_type: String,
    pub pattern: String,
    pub severity: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModel {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub requests_24h: u64,
    pub flagged_24h: u64,
    pub avg_score: f64,
    pub status: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// In-memory store
// ═══════════════════════════════════════════════════════════════════════════

fn llm_events() -> &'static RwLock<Vec<LlmEvent>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<LlmEvent>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
fn llm_rules() -> &'static RwLock<Vec<LlmRule>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<LlmRule>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}
fn llm_models() -> &'static RwLock<Vec<LlmModel>> {
    static INSTANCE: std::sync::OnceLock<RwLock<Vec<LlmModel>>> = std::sync::OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(vec![]))
}


// ═══════════════════════════════════════════════════════════════════════════
// Query / Request types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
pub struct EventListQuery {
    pub event_type: Option<String>,
    pub severity: Option<String>,
    pub model: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    pub prompt: String,
    pub response: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub rule_type: String,
    pub pattern: String,
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRuleRequest {
    pub name: Option<String>,
    pub pattern: Option<String>,
    pub severity: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct IngestRequest {
    pub events: Vec<Value>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Handlers
// ═══════════════════════════════════════════════════════════════════════════

/// GET /api/llm-security/overview
pub async fn overview(State(_s): State<AppState>) -> Json<Value> {
    let events = llm_events().read();
    let models = llm_models().read();
    let total_requests: u64 = models.iter().map(|m| m.requests_24h).sum();
    let total_flagged: u64 = models.iter().map(|m| m.flagged_24h).sum();
    let critical = events.iter().filter(|e| e.severity == "critical").count();
    let high = events.iter().filter(|e| e.severity == "high").count();

    let flagged_rate = if total_requests > 0 { total_flagged as f64 / total_requests as f64 * 100.0 } else { 0.0 };
    let active_models = models.iter().filter(|m| m.status == "active").count();
    let active_rules = llm_rules().read().iter().filter(|r| r.enabled).count();
    let mut event_type_counts = std::collections::HashMap::new();
    for e in events.iter() { *event_type_counts.entry(e.event_type.clone()).or_insert(0u32) += 1; }

    Json(json!({
        "total_requests_24h": total_requests,
        "total_flagged_24h": total_flagged,
        "flagged_rate": flagged_rate,
        "critical_events": critical,
        "high_events": high,
        "active_models": active_models,
        "active_rules": active_rules,
        "event_types": event_type_counts,
    }))
}

/// POST /api/llm-security/scan
pub async fn scan_prompt(State(_s): State<AppState>, Json(req): Json<ScanRequest>) -> Json<Value> {
    let rules = llm_rules().read();
    let mut findings: Vec<Value> = Vec::new();
    let mut max_score = 0.0f64;

    for rule in rules.iter().filter(|r| r.enabled) {
        let text = &req.prompt;
        if let Ok(re) = regex::Regex::new(&rule.pattern) {
            if re.is_match(text) {
                let score = match rule.severity.as_str() {
                    "critical" => 0.95, "high" => 0.80, "medium" => 0.60, _ => 0.40,
                };
                if score > max_score { max_score = score; }
                findings.push(json!({
                    "rule_id": rule.id,
                    "rule_name": rule.name,
                    "rule_type": rule.rule_type,
                    "severity": rule.severity,
                    "score": score,
                    "matched": true,
                }));
            }
        }
    }

    let verdict = if max_score >= 0.9 { "block" } else if max_score >= 0.6 { "warn" } else { "pass" };

    Json(json!({
        "verdict": verdict,
        "score": max_score,
        "findings": findings,
        "scanned_at": Utc::now().to_rfc3339(),
    }))
}

/// GET /api/llm-security/events
pub async fn list_events(
    State(_s): State<AppState>,
    Query(q): Query<EventListQuery>,
) -> Json<Value> {
    let store = llm_events().read();
    let mut out: Vec<&LlmEvent> = store.iter().collect();

    if let Some(t) = &q.event_type { out.retain(|e| &e.event_type == t); }
    if let Some(s) = &q.severity { out.retain(|e| &e.severity == s); }
    if let Some(m) = &q.model { out.retain(|e| e.model.to_lowercase().contains(&m.to_lowercase())); }

    let total = out.len();
    let page = q.page.unwrap_or(1).max(1) as usize;
    let per_page = q.per_page.unwrap_or(20).min(100) as usize;
    let start = (page - 1) * per_page;
    let items: Vec<&LlmEvent> = out.into_iter().skip(start).take(per_page).collect();

    Json(json!({ "events": items, "total": total, "page": page, "per_page": per_page }))
}

/// GET /api/llm-security/events/:id
pub async fn get_event(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LlmEvent>, ApiError> {
    llm_events().read().iter().find(|e| e.id == id).cloned()
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("Event not found".into()))
}

/// GET /api/llm-security/stats
pub async fn stats(State(_s): State<AppState>) -> Json<Value> {
    let events = llm_events().read();
    let mut by_type = std::collections::HashMap::<String, u32>::new();
    let mut by_severity = std::collections::HashMap::<String, u32>::new();
    let mut by_model = std::collections::HashMap::<String, u32>::new();
    for e in events.iter() {
        *by_type.entry(e.event_type.clone()).or_default() += 1;
        *by_severity.entry(e.severity.clone()).or_default() += 1;
        *by_model.entry(e.model.clone()).or_default() += 1;
    }
    Json(json!({ "by_type": by_type, "by_severity": by_severity, "by_model": by_model, "total": events.len() }))
}

/// GET /api/llm-security/rules
pub async fn list_rules(State(_s): State<AppState>) -> Json<Vec<LlmRule>> {
    Json(llm_rules().read().clone())
}

/// POST /api/llm-security/rules
pub async fn create_rule(
    State(_s): State<AppState>,
    Json(req): Json<CreateRuleRequest>,
) -> Json<LlmRule> {
    let rule = LlmRule {
        id: Uuid::new_v4().to_string(),
        name: req.name,
        rule_type: req.rule_type,
        pattern: req.pattern,
        severity: req.severity.unwrap_or_else(|| "medium".into()),
        enabled: true,
        created_at: Utc::now().to_rfc3339(),
    };
    llm_rules().write().push(rule.clone());
    Json(rule)
}

/// PUT /api/llm-security/rules/:id
pub async fn update_rule(
    State(_s): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRuleRequest>,
) -> Result<Json<LlmRule>, ApiError> {
    let mut store = llm_rules().write();
    if let Some(r) = store.iter_mut().find(|r| r.id == id) {
        if let Some(n) = req.name { r.name = n; }
        if let Some(p) = req.pattern { r.pattern = p; }
        if let Some(s) = req.severity { r.severity = s; }
        if let Some(e) = req.enabled { r.enabled = e; }
        Ok(Json(r.clone()))
    } else {
        Err(ApiError::NotFound("Rule not found".into()))
    }
}

/// GET /api/llm-security/models
pub async fn list_models(State(_s): State<AppState>) -> Json<Vec<LlmModel>> {
    Json(llm_models().read().clone())
}

/// POST /api/llm-security/ingest
pub async fn ingest(
    State(_s): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Json<Value> {
    let mut store = llm_events().write();
    let count = req.events.len();
    for evt in req.events {
        let event = LlmEvent {
            id: Uuid::new_v4().to_string(),
            event_type: evt.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown").into(),
            severity: evt.get("severity").and_then(|v| v.as_str()).unwrap_or("low").into(),
            model: evt.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").into(),
            prompt_preview: evt.get("prompt").and_then(|v| v.as_str()).unwrap_or("").chars().take(100).collect(),
            response_preview: evt.get("response").and_then(|v| v.as_str()).map(|s| s.chars().take(100).collect()),
            score: evt.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0),
            details: evt.clone(),
            source_ip: evt.get("source_ip").and_then(|v| v.as_str()).map(String::from),
            user_id: evt.get("user_id").and_then(|v| v.as_str()).map(String::from),
            created_at: Utc::now().to_rfc3339(),
        };
        store.push(event);
    }
    Json(json!({"ingested": count}))
}
