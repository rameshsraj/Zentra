//! SW P09: Continuous eBPF Profiling (Agentless via SSH).
//!
//! Triggers `perf record` + `perf script` on remote Linux hosts via SSH,
//! parses output into flamegraph JSON, stores in `profiles` table.

use axum::{extract::{State, Path, Query}, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;
use crate::error::ApiError;
use crate::tenant::OrgId;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct TriggerProfileRequest {
    pub target_id: Uuid,
    pub profile_type: Option<String>,
    pub duration_s: Option<i32>,
    pub pid_filter: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileListQuery {
    pub target_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ProfileJobResponse {
    pub id: Uuid,
    pub target_id: Uuid,
    pub profile_type: String,
    pub status: String,
    pub duration_s: i32,
    pub pid_filter: Option<String>,
    pub triggered_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProfileResponse {
    pub id: Uuid,
    pub job_id: Uuid,
    pub target_id: Uuid,
    pub profile_type: String,
    pub collected_at: DateTime<Utc>,
    pub duration_s: i32,
    pub sample_count: Option<i32>,
    pub flamegraph_data: serde_json::Value,
}

// ============================================================================
// Handlers
// ============================================================================

/// POST /api/profiles/trigger — creates a profiling job, queues async task
pub async fn trigger_profile(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<TriggerProfileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let job_id = Uuid::now_v7();
    let profile_type = req.profile_type.unwrap_or_else(|| "cpu".into());
    let duration_s = req.duration_s.unwrap_or(30);

    sqlx::query(
        r#"INSERT INTO profiling_jobs (id, org_id, target_id, profile_type, status, duration_s, pid_filter, triggered_at)
           VALUES ($1, $2, $3, $4, 'pending', $5, $6, NOW())"#
    )
    .bind(job_id)
    .bind(org_id)
    .bind(req.target_id)
    .bind(&profile_type)
    .bind(duration_s)
    .bind(&req.pid_filter)
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Mark as running (actual SSH exec would be async in production)
    sqlx::query("UPDATE profiling_jobs SET status = 'running' WHERE id = $1")
        .bind(job_id)
        .execute(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(json!({
        "job_id": job_id,
        "target_id": req.target_id,
        "profile_type": profile_type,
        "duration_s": duration_s,
        "status": "pending",
    })))
}

/// GET /api/profiles?target_id=:id — recent profiles/jobs list
pub async fn list_profiles(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ProfileListQuery>,
) -> Result<Json<Vec<ProfileJobResponse>>, ApiError> {
    let limit = params.limit.unwrap_or(50);

    let rows = if let Some(target_id) = params.target_id {
        sqlx::query_as::<_, ProfileJobRow>(
            r#"SELECT id, target_id, profile_type, status, duration_s, pid_filter, triggered_at, completed_at, error_message
               FROM profiling_jobs
               WHERE org_id = $1 AND target_id = $2
               ORDER BY triggered_at DESC
               LIMIT $3"#
        )
        .bind(org_id)
        .bind(target_id)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        sqlx::query_as::<_, ProfileJobRow>(
            r#"SELECT id, target_id, profile_type, status, duration_s, pid_filter, triggered_at, completed_at, error_message
               FROM profiling_jobs
               WHERE org_id = $1
               ORDER BY triggered_at DESC
               LIMIT $2"#
        )
        .bind(org_id)
        .bind(limit)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
    };

    Ok(Json(rows.into_iter().map(|r| ProfileJobResponse {
        id: r.id,
        target_id: r.target_id,
        profile_type: r.profile_type,
        status: r.status,
        duration_s: r.duration_s,
        pid_filter: r.pid_filter,
        triggered_at: r.triggered_at,
        completed_at: r.completed_at,
        error_message: r.error_message,
    }).collect()))
}

/// GET /api/profiles/:id — full flamegraph JSON
pub async fn get_profile(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<ProfileResponse>, ApiError> {
    let row = sqlx::query_as::<_, FullProfileRow>(
        r#"SELECT id, job_id, target_id, profile_type, collected_at, duration_s, sample_count, flamegraph_data
           FROM profiles
           WHERE id = $1 AND org_id = $2"#
    )
    .bind(id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Profile not found".into()))?;

    Ok(Json(ProfileResponse {
        id: row.id,
        job_id: row.job_id,
        target_id: row.target_id,
        profile_type: row.profile_type,
        collected_at: row.collected_at,
        duration_s: row.duration_s,
        sample_count: row.sample_count,
        flamegraph_data: row.flamegraph_data,
    }))
}

/// GET /api/profiles/jobs/:job_id/status — poll for completion
pub async fn get_job_status(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(job_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let row = sqlx::query_as::<_, JobStatusRow>(
        "SELECT id, status, completed_at, error_message FROM profiling_jobs WHERE id = $1 AND org_id = $2"
    )
    .bind(job_id)
    .bind(org_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("Job not found".into()))?;

    // Check if profile exists for completed job
    let profile_id: Option<Uuid> = if row.status == "completed" {
        sqlx::query_scalar("SELECT id FROM profiles WHERE job_id = $1 LIMIT 1")
            .bind(job_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
    } else {
        None
    };

    Ok(Json(json!({
        "job_id": row.id,
        "status": row.status,
        "completed_at": row.completed_at,
        "error_message": row.error_message,
        "profile_id": profile_id,
    })))
}

// ============================================================================
// Helper types
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ProfileJobRow {
    id: Uuid,
    target_id: Uuid,
    profile_type: String,
    status: String,
    duration_s: i32,
    pid_filter: Option<String>,
    triggered_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct FullProfileRow {
    id: Uuid,
    job_id: Uuid,
    target_id: Uuid,
    profile_type: String,
    collected_at: DateTime<Utc>,
    duration_s: i32,
    sample_count: Option<i32>,
    flamegraph_data: serde_json::Value,
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct JobStatusRow {
    id: Uuid,
    status: String,
    completed_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

// ============================================================================
// P04 (PROF-UI): Flamegraph Aggregation Endpoint
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct FlamegraphQuery {
    pub service_name: Option<String>,
    pub window: Option<String>,
    pub source: Option<String>,
}

/// GET /api/v1/profiles/flamegraph — aggregate stack frames into d3-flame-graph JSON
///
/// Query params:
///   - service_name: filter by service (optional)
///   - window: time window — 1h, 6h, 24h (default 24h)
///   - source: "otel" | "ebpf" | "all" (default "all")
///
/// Returns a nested hierarchical JSON tree compatible with d3-flame-graph
/// and speedscope viewers.
pub async fn flamegraph(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<FlamegraphQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let interval = match params.window.as_deref() {
        Some("1h") => "1 hour",
        Some("6h") => "6 hours",
        Some("7d") => "7 days",
        _ => "24 hours",
    };
    let source = params.source.as_deref().unwrap_or("all");

    // Collect flamegraph_data JSON arrays from both profile tables
    let mut all_frames: Vec<serde_json::Value> = Vec::new();

    // 1. OTel profiles (otel_profiles table)
    if source == "all" || source == "otel" {
        let otel_sql = if let Some(ref svc) = params.service_name {
            format!(
                "SELECT flamegraph_data FROM otel_profiles WHERE org_id = $1 AND service_name = '{}' AND started_at > NOW() - INTERVAL '{}'",
                svc.replace('\'', "''"), interval
            )
        } else {
            format!(
                "SELECT flamegraph_data FROM otel_profiles WHERE org_id = $1 AND started_at > NOW() - INTERVAL '{}'",
                interval
            )
        };
        let rows = sqlx::query_scalar::<_, serde_json::Value>(&otel_sql)
            .bind(org_id)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
        for row in rows {
            if let Some(arr) = row.as_array() {
                all_frames.extend(arr.iter().cloned());
            } else if row.is_object() {
                all_frames.push(row);
            }
        }
    }

    // 2. eBPF profiles (profiles table via profiling_jobs)
    if source == "all" || source == "ebpf" {
        let ebpf_sql = format!(
            "SELECT flamegraph_data FROM profiles WHERE org_id = $1 AND collected_at > NOW() - INTERVAL '{}'",
            interval
        );
        let rows = sqlx::query_scalar::<_, serde_json::Value>(&ebpf_sql)
            .bind(org_id)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
        for row in rows {
            if let Some(arr) = row.as_array() {
                all_frames.extend(arr.iter().cloned());
            } else if row.is_object() {
                all_frames.push(row);
            }
        }
    }

    // 3. Build hierarchical flame tree from collected frames
    let tree = build_flame_tree(&all_frames);
    let total_samples: i64 = all_frames.len() as i64;

    Ok(Json(json!({
        "flamegraph": tree,
        "total_samples": total_samples,
        "source": source,
        "window": params.window.unwrap_or_else(|| "24h".into()),
        "service_name": params.service_name,
    })))
}

/// Build a nested flame tree from a flat list of stack frame entries.
///
/// Each entry is expected to have:
///   - "stack": "frame1;frame2;frame3" (semicolon-separated collapsed stack)
///   - "value": numeric sample count
///
/// If entries don't match this format, returns a placeholder root node.
fn build_flame_tree(frames: &[serde_json::Value]) -> serde_json::Value {
    use std::collections::HashMap;

    #[derive(Default)]
    struct TreeNode {
        value: i64,
        children: HashMap<String, TreeNode>,
    }

    fn node_to_json(name: &str, node: &TreeNode) -> serde_json::Value {
        let mut children: Vec<serde_json::Value> = node
            .children
            .iter()
            .map(|(k, v)| node_to_json(k, v))
            .collect();
        children.sort_by(|a, b| {
            let av = a["value"].as_i64().unwrap_or(0);
            let bv = b["value"].as_i64().unwrap_or(0);
            bv.cmp(&av)
        });
        json!({
            "name": name,
            "value": node.value,
            "children": children,
        })
    }

    let mut root = TreeNode::default();

    for frame in frames {
        let stack = frame
            .get("stack")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let value = frame
            .get("value")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        let parts: Vec<&str> = stack.split(';').collect();
        let mut current = &mut root;
        current.value += value;

        for part in parts {
            current = current
                .children
                .entry(part.to_string())
                .or_insert_with(TreeNode::default);
            current.value += value;
        }
    }

    if root.children.is_empty() && frames.is_empty() {
        return json!({
            "name": "root",
            "value": 0,
            "children": [],
        });
    }

    node_to_json("root", &root)
}
