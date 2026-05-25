use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use parking_lot::RwLock;
use std::sync::OnceLock;
use crate::state::AppState;

use crate::error::ApiError;

// ============================================================================
// On-Call Management — Schedules, Rotations, Shifts, Overrides, Escalation
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OncallSchedule {
    pub id: String,
    pub name: String,
    pub rotation_type: String,  // weekly, follow_sun, custom, daily
    pub shift_duration_hours: u32,
    pub handoff_time: String,   // HH:MM in 24h
    pub timezone: String,
    pub enabled: bool,
    pub team: String,
    pub description: Option<String>,
    pub created_at: String,
    pub layers: Vec<OncallLayer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OncallLayer {
    pub id: String,
    pub name: String,           // Primary, Secondary, Tertiary
    pub order: u32,
    pub members: Vec<OncallMember>,
    pub restriction: Option<TimeRestriction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OncallMember {
    pub user_id: String,
    pub user_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub rotation_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestriction {
    pub start_day: String,      // Mon, Tue, ... Sun
    pub end_day: String,
    pub start_time: String,     // HH:MM
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OncallShift {
    pub id: String,
    pub schedule_id: String,
    pub layer_id: String,
    pub user_id: String,
    pub user_name: String,
    pub email: String,
    pub start_time: String,
    pub end_time: String,
    pub is_override: bool,
    pub override_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentOncall {
    pub schedule_id: String,
    pub schedule_name: String,
    pub primary: OncallMember,
    pub secondary: Option<OncallMember>,
    pub current_shift_start: String,
    pub current_shift_end: String,
    pub next_handoff: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub name: String,
    pub rotation_type: Option<String>,
    pub shift_duration_hours: Option<u32>,
    pub handoff_time: Option<String>,
    pub timezone: Option<String>,
    pub team: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOverrideRequest {
    pub user_id: String,
    pub user_name: String,
    pub email: String,
    pub start_time: String,
    pub end_time: String,
    pub reason: Option<String>,
}

struct OncallStore {
    schedules: Vec<OncallSchedule>,
    shifts: Vec<OncallShift>,
}

fn store() -> &'static RwLock<OncallStore> {
    static INSTANCE: OnceLock<RwLock<OncallStore>> = OnceLock::new();
    INSTANCE.get_or_init(|| RwLock::new(init_store()))
}

fn init_store() -> OncallStore {
    let now = Utc::now();

    let schedules = vec![
        OncallSchedule {
            id: "sched-001".into(),
            name: "Infrastructure Primary".into(),
            rotation_type: "weekly".into(),
            shift_duration_hours: 168,
            handoff_time: "09:00".into(),
            timezone: "America/New_York".into(),
            enabled: true,
            team: "Infrastructure".into(),
            description: Some("Primary on-call rotation for infrastructure incidents".into()),
            created_at: (now - chrono::Duration::days(90)).to_rfc3339(),
            layers: vec![
                OncallLayer {
                    id: "layer-001".into(),
                    name: "Primary".into(),
                    order: 1,
                    members: vec![
                        OncallMember {
                            user_id: "user-001".into(),
                            user_name: "Alice Johnson".into(),
                            email: "alice@example.com".into(),
                            phone: Some("+1-555-0101".into()),
                            rotation_index: 0,
                        },
                        OncallMember {
                            user_id: "user-002".into(),
                            user_name: "Bob Smith".into(),
                            email: "bob@example.com".into(),
                            phone: Some("+1-555-0102".into()),
                            rotation_index: 1,
                        },
                        OncallMember {
                            user_id: "user-003".into(),
                            user_name: "Carol Davis".into(),
                            email: "carol@example.com".into(),
                            phone: Some("+1-555-0103".into()),
                            rotation_index: 2,
                        },
                    ],
                    restriction: None,
                },
                OncallLayer {
                    id: "layer-002".into(),
                    name: "Secondary".into(),
                    order: 2,
                    members: vec![
                        OncallMember {
                            user_id: "user-004".into(),
                            user_name: "Dave Wilson".into(),
                            email: "dave@example.com".into(),
                            phone: Some("+1-555-0104".into()),
                            rotation_index: 0,
                        },
                        OncallMember {
                            user_id: "user-005".into(),
                            user_name: "Eve Martinez".into(),
                            email: "eve@example.com".into(),
                            phone: None,
                            rotation_index: 1,
                        },
                    ],
                    restriction: Some(TimeRestriction {
                        start_day: "Mon".into(),
                        end_day: "Fri".into(),
                        start_time: "09:00".into(),
                        end_time: "17:00".into(),
                    }),
                },
            ],
        },
        OncallSchedule {
            id: "sched-002".into(),
            name: "Database Follow-the-Sun".into(),
            rotation_type: "follow_sun".into(),
            shift_duration_hours: 8,
            handoff_time: "08:00".into(),
            timezone: "UTC".into(),
            enabled: true,
            team: "Database".into(),
            description: Some("Follow-the-sun coverage across US, Europe, APAC".into()),
            created_at: (now - chrono::Duration::days(45)).to_rfc3339(),
            layers: vec![
                OncallLayer {
                    id: "layer-003".into(),
                    name: "APAC".into(),
                    order: 1,
                    restriction: Some(TimeRestriction {
                        start_day: "Mon".into(),
                        end_day: "Fri".into(),
                        start_time: "00:00".into(),
                        end_time: "08:00".into(),
                    }),
                    members: vec![
                        OncallMember {
                            user_id: "user-006".into(),
                            user_name: "Frank Lee".into(),
                            email: "frank@example.com".into(),
                            phone: Some("+65-9000-0001".into()),
                            rotation_index: 0,
                        },
                    ],
                },
                OncallLayer {
                    id: "layer-004".into(),
                    name: "EMEA".into(),
                    order: 2,
                    restriction: Some(TimeRestriction {
                        start_day: "Mon".into(),
                        end_day: "Fri".into(),
                        start_time: "08:00".into(),
                        end_time: "16:00".into(),
                    }),
                    members: vec![
                        OncallMember {
                            user_id: "user-007".into(),
                            user_name: "Grace König".into(),
                            email: "grace@example.com".into(),
                            phone: Some("+49-30-000001".into()),
                            rotation_index: 0,
                        },
                    ],
                },
                OncallLayer {
                    id: "layer-005".into(),
                    name: "Americas".into(),
                    order: 3,
                    restriction: Some(TimeRestriction {
                        start_day: "Mon".into(),
                        end_day: "Fri".into(),
                        start_time: "16:00".into(),
                        end_time: "00:00".into(),
                    }),
                    members: vec![
                        OncallMember {
                            user_id: "user-001".into(),
                            user_name: "Alice Johnson".into(),
                            email: "alice@example.com".into(),
                            phone: Some("+1-555-0101".into()),
                            rotation_index: 0,
                        },
                    ],
                },
            ],
        },
    ];

    let shift_start = (now - chrono::Duration::days(2)).to_rfc3339();
    let shift_end = (now + chrono::Duration::days(5)).to_rfc3339();

    let shifts = vec![
        OncallShift {
            id: "shift-001".into(),
            schedule_id: "sched-001".into(),
            layer_id: "layer-001".into(),
            user_id: "user-001".into(),
            user_name: "Alice Johnson".into(),
            email: "alice@example.com".into(),
            start_time: shift_start.clone(),
            end_time: shift_end.clone(),
            is_override: false,
            override_reason: None,
        },
        OncallShift {
            id: "shift-002".into(),
            schedule_id: "sched-001".into(),
            layer_id: "layer-002".into(),
            user_id: "user-004".into(),
            user_name: "Dave Wilson".into(),
            email: "dave@example.com".into(),
            start_time: shift_start,
            end_time: shift_end,
            is_override: false,
            override_reason: None,
        },
    ];

    OncallStore { schedules, shifts }
}

pub async fn list_schedules(
    State(_s): State<AppState>,
) -> Result<Json<Vec<OncallSchedule>>, ApiError> {
    let store = store().read();
    Ok(Json(store.schedules.clone()))
}

pub async fn get_schedule(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<OncallSchedule>, ApiError> {
    let store = store().read();
    store.schedules.iter().find(|s| s.id == id).cloned().map(Json)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Schedule not found".into())))
}

pub async fn create_schedule(
    State(_s): State<AppState>,
    Json(req): Json<CreateScheduleRequest>,
) -> Result<Json<OncallSchedule>, ApiError> {
    let schedule = OncallSchedule {
        id: crate::error::short_id("sched"),
        name: req.name,
        rotation_type: req.rotation_type.unwrap_or_else(|| "weekly".into()),
        shift_duration_hours: req.shift_duration_hours.unwrap_or(168),
        handoff_time: req.handoff_time.unwrap_or_else(|| "09:00".into()),
        timezone: req.timezone.unwrap_or_else(|| "UTC".into()),
        enabled: true,
        team: req.team.unwrap_or_else(|| "Engineering".into()),
        description: req.description,
        created_at: Utc::now().to_rfc3339(),
        layers: vec![],
    };
    store().write().schedules.push(schedule.clone());
    Ok(Json(schedule))
}

pub async fn delete_schedule(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = store().write();
    let before = store.schedules.len();
    store.schedules.retain(|s| s.id != id);
    if store.schedules.len() < before {
        Ok(Json(serde_json::json!({ "deleted": id })))
    } else {
        Err(ApiError::from((StatusCode::NOT_FOUND, "Schedule not found".into())))
    }
}

pub async fn current_oncall(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CurrentOncall>, ApiError> {
    let store = store().read();
    let sched = store.schedules.iter().find(|s| s.id == id)
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "Schedule not found".into())))?;

    let primary_layer = sched.layers.iter().find(|l| l.order == 1);
    let secondary_layer = sched.layers.iter().find(|l| l.order == 2);

    let now = Utc::now();
    let primary_member = primary_layer.and_then(|l| l.members.first()).cloned()
        .ok_or(ApiError::from((StatusCode::NOT_FOUND, "No members in primary layer".into())))?;

    let secondary_member = secondary_layer.and_then(|l| l.members.first()).cloned();

    let shift_start = (now - chrono::Duration::days(2)).to_rfc3339();
    let shift_end = (now + chrono::Duration::days(5)).to_rfc3339();
    let next_handoff = (now + chrono::Duration::days(5)).to_rfc3339();

    Ok(Json(CurrentOncall {
        schedule_id: sched.id.clone(),
        schedule_name: sched.name.clone(),
        primary: primary_member,
        secondary: secondary_member,
        current_shift_start: shift_start,
        current_shift_end: shift_end,
        next_handoff,
    }))
}

pub async fn list_shifts(
    State(_s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<OncallShift>>, ApiError> {
    let store = store().read();
    let shifts: Vec<OncallShift> = store.shifts.iter()
        .filter(|s| s.schedule_id == id)
        .cloned().collect();
    Ok(Json(shifts))
}

pub async fn create_override(
    State(_s): State<AppState>,
    Path((schedule_id, layer_id)): Path<(String, String)>,
    Json(req): Json<CreateOverrideRequest>,
) -> Result<Json<OncallShift>, ApiError> {
    {
        let store = store().read();
        if !store.schedules.iter().any(|s| s.id == schedule_id) {
            return Err(ApiError::from((StatusCode::NOT_FOUND, "Schedule not found".into())));
        }
    }
    let shift = OncallShift {
        id: crate::error::short_id("shift"),
        schedule_id,
        layer_id,
        user_id: req.user_id,
        user_name: req.user_name,
        email: req.email,
        start_time: req.start_time,
        end_time: req.end_time,
        is_override: true,
        override_reason: req.reason,
    };
    store().write().shifts.push(shift.clone());
    Ok(Json(shift))
}

pub async fn oncall_summary(
    State(_s): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let store = store().read();
    let total = store.schedules.len();
    let enabled = store.schedules.iter().filter(|s| s.enabled).count();
    let total_members: usize = store.schedules.iter()
        .flat_map(|s| s.layers.iter())
        .map(|l| l.members.len())
        .sum();
    let overrides = store.shifts.iter().filter(|s| s.is_override).count();
    Ok(Json(serde_json::json!({
        "total_schedules": total,
        "enabled_schedules": enabled,
        "total_members": total_members,
        "active_overrides": overrides,
        "rotation_types": {
            "weekly": store.schedules.iter().filter(|s| s.rotation_type == "weekly").count(),
            "follow_sun": store.schedules.iter().filter(|s| s.rotation_type == "follow_sun").count(),
            "custom": store.schedules.iter().filter(|s| s.rotation_type == "custom").count(),
        }
    })))
}
