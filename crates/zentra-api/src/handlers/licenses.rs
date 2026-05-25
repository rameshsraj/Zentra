use axum::{extract::{State, Path, Query}, Json};
use crate::state::AppState;
use crate::tenant::OrgId;
use zentra_core::models::license::{
    License, LicenseAuditEntry, LicenseCategory, LicenseType,
    CreateLicenseRequest, UpdateLicenseRequest, AssignSeatRequest, ComplianceSummary,
};
use serde::Deserialize;
use uuid::Uuid;
use chrono::NaiveDate;

#[derive(Deserialize)]
pub struct ListLicensesQuery {
    pub status: Option<String>,
    pub vendor: Option<String>,
    pub category: Option<String>,
}

#[derive(Deserialize)]
pub struct AuditLogQuery {
    pub license_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct ExpiringQuery {
    pub days: Option<i32>,
}

use crate::error::ApiError;

fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}

fn not_found(msg: &str) -> ApiError {
    ApiError::NotFound(msg.to_string())
}

fn bad_request(msg: &str) -> ApiError {
    ApiError::BadRequest(msg.to_string())
}

// ============================================================================
// License CRUD
// ============================================================================

pub async fn list_licenses(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ListLicensesQuery>,
) -> Result<Json<Vec<License>>, ApiError> {
    let licenses = zentra_db::repositories::licenses::list_licenses(
        &state.pool,
        org_id,
        params.status.as_deref(),
        params.vendor.as_deref(),
        params.category.as_deref(),
    )
    .await
    .map_err(internal)?;

    Ok(Json(licenses))
}

pub async fn get_license(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<License>, ApiError> {
    let lic = zentra_db::repositories::licenses::get_license(&state.pool, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| not_found("License not found"))?;

    Ok(Json(lic))
}

pub async fn create_license(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Json(req): Json<CreateLicenseRequest>,
) -> Result<Json<License>, ApiError> {
    if req.seat_count < 1 {
        return Err(bad_request("seat_count must be at least 1"));
    }

    let mut lic = License::new(
        org_id,
        &req.name,
        &req.product_name,
        &req.vendor,
        req.seat_count,
    );

    lic.serial_number = req.serial_number.clone();
    lic.product_version = req.product_version.clone();
    lic.cost_per_seat = req.cost_per_seat;
    lic.total_cost = req.total_cost;
    lic.currency = req.currency.clone().unwrap_or_else(|| "USD".into());
    lic.po_number = req.po_number.clone();
    lic.order_number = req.order_number.clone();
    lic.supplier_name = req.supplier_name.clone();
    lic.supplier_email = req.supplier_email.clone();
    lic.supplier_phone = req.supplier_phone.clone();
    lic.contract_url = req.contract_url.clone();
    lic.notes = req.notes.clone();
    lic.is_maintained = req.is_maintained.unwrap_or(false);
    lic.is_reassignable = req.is_reassignable.unwrap_or(true);
    lic.environment = req.environment.clone().unwrap_or_else(|| "production".into());

    if let Some(ref cat) = req.category {
        lic.category = match cat.as_str() {
            "hardware" => LicenseCategory::Hardware,
            "cloud" => LicenseCategory::Cloud,
            "saas" => LicenseCategory::Saas,
            "perpetual" => LicenseCategory::Perpetual,
            _ => LicenseCategory::Software,
        };
    }
    if let Some(ref lt) = req.license_type {
        lic.license_type = match lt.as_str() {
            "per_device" => LicenseType::PerDevice,
            "site" => LicenseType::Site,
            "enterprise" => LicenseType::Enterprise,
            "concurrent" => LicenseType::Concurrent,
            "subscription" => LicenseType::Subscription,
            _ => LicenseType::PerSeat,
        };
    }
    if let Some(ref tags) = req.tags {
        lic.tags = tags.clone();
    }

    fn parse_date(s: &str) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
    }
    lic.purchase_date = req.purchase_date.as_deref().and_then(parse_date);
    lic.expiry_date = req.expiry_date.as_deref().and_then(parse_date);
    lic.renewal_date = req.renewal_date.as_deref().and_then(parse_date);

    // Insert into DB
    zentra_db::repositories::licenses::insert_license(&state.pool, &lic)
        .await
        .map_err(internal)?;

    // Create seat records
    zentra_db::repositories::licenses::create_seats(
        &state.pool,
        lic.license_id,
        org_id,
        lic.seat_count,
    )
    .await
    .map_err(internal)?;

    // Audit log
    let audit = LicenseAuditEntry::new(lic.license_id, org_id, "created", "api")
        .with_changes(serde_json::json!({
            "name": lic.name,
            "vendor": lic.vendor,
            "seat_count": lic.seat_count,
            "product_name": lic.product_name,
        }));
    let _ = zentra_db::repositories::licenses::insert_audit_entry(&state.pool, &audit).await;

    Ok(Json(lic))
}

pub async fn update_license(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLicenseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    fn parse_date(s: &str) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
    }

    let expiry = req.expiry_date.as_deref().and_then(parse_date);

    zentra_db::repositories::licenses::update_license(
        &state.pool,
        id,
        req.name.as_deref(),
        req.seat_count,
        req.status.as_deref(),
        expiry,
        req.notes.as_deref(),
    )
    .await
    .map_err(internal)?;

    // Audit
    let audit = LicenseAuditEntry::new(id, org_id, "updated", "api")
        .with_changes(serde_json::to_value(&req).unwrap_or_default());
    let _ = zentra_db::repositories::licenses::insert_audit_entry(&state.pool, &audit).await;

    Ok(Json(serde_json::json!({"updated": true, "license_id": id})))
}

pub async fn delete_license(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    zentra_db::repositories::licenses::delete_license(&state.pool, id)
        .await
        .map_err(internal)?;

    let audit = LicenseAuditEntry::new(id, org_id, "deleted", "api");
    let _ = zentra_db::repositories::licenses::insert_audit_entry(&state.pool, &audit).await;

    Ok(Json(serde_json::json!({"deleted": true})))
}

// ============================================================================
// Seat operations
// ============================================================================

pub async fn list_seats(
    State(state): State<AppState>,
    Path(license_id): Path<Uuid>,
) -> Result<Json<Vec<zentra_core::LicenseSeat>>, ApiError> {
    let seats = zentra_db::repositories::licenses::list_seats(&state.pool, license_id)
        .await
        .map_err(internal)?;
    Ok(Json(seats))
}

pub async fn assign_seat(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path(license_id): Path<Uuid>,
    Json(req): Json<AssignSeatRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let seat_id = zentra_db::repositories::licenses::get_available_seat(&state.pool, license_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| bad_request("No available seats"))?;

    zentra_db::repositories::licenses::assign_seat(
        &state.pool,
        seat_id,
        &req.assigned_to,
        req.assigned_asset.as_deref(),
        req.assigned_target_id,
        req.notes.as_deref(),
    )
    .await
    .map_err(internal)?;

    let audit = LicenseAuditEntry::new(license_id, org_id, "seat_assigned", "api")
        .with_changes(serde_json::json!({
            "seat_id": seat_id,
            "assigned_to": req.assigned_to,
        }));
    let _ = zentra_db::repositories::licenses::insert_audit_entry(&state.pool, &audit).await;

    Ok(Json(serde_json::json!({"assigned": true, "seat_id": seat_id})))
}

pub async fn release_seat(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Path((license_id, seat_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    zentra_db::repositories::licenses::release_seat(&state.pool, seat_id)
        .await
        .map_err(internal)?;

    let audit = LicenseAuditEntry::new(license_id, org_id, "seat_released", "api")
        .with_changes(serde_json::json!({"seat_id": seat_id}));
    let _ = zentra_db::repositories::licenses::insert_audit_entry(&state.pool, &audit).await;

    Ok(Json(serde_json::json!({"released": true, "seat_id": seat_id})))
}

// ============================================================================
// Compliance & Audit
// ============================================================================

pub async fn get_compliance(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
) -> Result<Json<ComplianceSummary>, ApiError> {
    let summary = zentra_db::repositories::licenses::get_compliance_summary(&state.pool, org_id)
        .await
        .map_err(internal)?;
    Ok(Json(summary))
}

pub async fn get_expiring(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<ExpiringQuery>,
) -> Result<Json<Vec<License>>, ApiError> {
    let days = params.days.unwrap_or(90);
    let licenses = zentra_db::repositories::licenses::get_expiring_licenses(&state.pool, org_id, days)
        .await
        .map_err(internal)?;
    Ok(Json(licenses))
}

pub async fn get_audit_log(
    State(state): State<AppState>,
    OrgId(org_id): OrgId,
    Query(params): Query<AuditLogQuery>,
) -> Result<Json<Vec<LicenseAuditEntry>>, ApiError> {
    let entries = zentra_db::repositories::licenses::list_audit_log(
        &state.pool,
        params.license_id,
        org_id,
        params.limit.unwrap_or(100),
    )
    .await
    .map_err(internal)?;
    Ok(Json(entries))
}
