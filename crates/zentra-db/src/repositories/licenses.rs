use zentra_core::models::license::{
    License, LicenseAuditEntry, LicenseCategory, LicenseStatus, LicenseType,
    LicenseSeat, LicenseAssignment, ComplianceSummary,
};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, NaiveDate, Utc};
use tracing::info;

// ============================================================================
// License CRUD
// ============================================================================

pub async fn insert_license(pool: &PgPool, lic: &License) -> Result<(), sqlx::Error> {
    let tags = serde_json::to_value(&lic.tags).unwrap_or_default();

    sqlx::query(
        r#"INSERT INTO licenses
           (license_id, org_id, name, serial_number, product_name, product_version, vendor,
            category, license_type, seat_count, cost_per_seat, total_cost, currency,
            purchase_date, expiry_date, renewal_date, termination_date,
            po_number, order_number, supplier_name, supplier_email, supplier_phone,
            contract_url, notes, is_maintained, is_reassignable, status, environment,
            tags, metadata, created_by, created_at, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30,$31,$32,$33)"#
    )
    .bind(lic.license_id)
    .bind(lic.org_id)
    .bind(&lic.name)
    .bind(&lic.serial_number)
    .bind(&lic.product_name)
    .bind(&lic.product_version)
    .bind(&lic.vendor)
    .bind(lic.category.as_str())
    .bind(lic.license_type.as_str())
    .bind(lic.seat_count)
    .bind(lic.cost_per_seat)
    .bind(lic.total_cost)
    .bind(&lic.currency)
    .bind(lic.purchase_date)
    .bind(lic.expiry_date)
    .bind(lic.renewal_date)
    .bind(lic.termination_date)
    .bind(&lic.po_number)
    .bind(&lic.order_number)
    .bind(&lic.supplier_name)
    .bind(&lic.supplier_email)
    .bind(&lic.supplier_phone)
    .bind(&lic.contract_url)
    .bind(&lic.notes)
    .bind(lic.is_maintained)
    .bind(lic.is_reassignable)
    .bind(lic.status.as_str())
    .bind(&lic.environment)
    .bind(&tags)
    .bind(&lic.metadata)
    .bind(&lic.created_by)
    .bind(lic.created_at)
    .bind(lic.updated_at)
    .execute(pool)
    .await?;

    info!(license_id = %lic.license_id, name = %lic.name, "License inserted");
    Ok(())
}

pub async fn list_licenses(
    pool: &PgPool,
    org_id: Uuid,
    status_filter: Option<&str>,
    vendor_filter: Option<&str>,
    category_filter: Option<&str>,
) -> Result<Vec<License>, sqlx::Error> {
    let mut sql = String::from(
        r#"SELECT l.*, 
           COALESCE(cs.assigned_seats, 0) as computed_assigned,
           COALESCE(cs.free_seats, l.seat_count) as computed_free
           FROM licenses l
           LEFT JOIN license_compliance_summary cs ON cs.license_id = l.license_id
           WHERE l.org_id = $1"#
    );
    let mut param_idx = 2;
    let mut binds: Vec<String> = Vec::new();

    if let Some(s) = status_filter {
        sql.push_str(&format!(" AND l.status = ${param_idx}"));
        param_idx += 1;
        binds.push(s.to_string());
    }
    if let Some(v) = vendor_filter {
        sql.push_str(&format!(" AND l.vendor ILIKE ${param_idx}"));
        param_idx += 1;
        binds.push(format!("%{v}%"));
    }
    if let Some(c) = category_filter {
        sql.push_str(&format!(" AND l.category = ${param_idx}"));
        let _ = param_idx;
        binds.push(c.to_string());
    }
    sql.push_str(" ORDER BY l.name");

    // Use dynamic query building
    let mut query = sqlx::query_as::<_, LicenseRow>(&sql).bind(org_id);
    for b in &binds {
        query = query.bind(b);
    }

    let rows = query.fetch_all(pool).await?;
    Ok(rows.into_iter().filter_map(|r| r.try_into().ok()).collect())
}

pub async fn get_license(pool: &PgPool, license_id: Uuid) -> Result<Option<License>, sqlx::Error> {
    let row = sqlx::query_as::<_, LicenseRow>(
        r#"SELECT l.*,
           COALESCE(cs.assigned_seats, 0) as computed_assigned,
           COALESCE(cs.free_seats, l.seat_count) as computed_free
           FROM licenses l
           LEFT JOIN license_compliance_summary cs ON cs.license_id = l.license_id
           WHERE l.license_id = $1"#
    )
    .bind(license_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|r| r.try_into().ok()))
}

pub async fn update_license(
    pool: &PgPool,
    license_id: Uuid,
    name: Option<&str>,
    seat_count: Option<i32>,
    status: Option<&str>,
    expiry_date: Option<NaiveDate>,
    notes: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE licenses SET
           name = COALESCE($2, name),
           seat_count = COALESCE($3, seat_count),
           status = COALESCE($4, status),
           expiry_date = COALESCE($5, expiry_date),
           notes = COALESCE($6, notes),
           updated_at = now()
           WHERE license_id = $1"#
    )
    .bind(license_id)
    .bind(name)
    .bind(seat_count)
    .bind(status)
    .bind(expiry_date)
    .bind(notes)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_license(pool: &PgPool, license_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE licenses SET status = 'terminated', updated_at = now() WHERE license_id = $1")
        .bind(license_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ============================================================================
// Seat management
// ============================================================================

pub async fn create_seats(pool: &PgPool, license_id: Uuid, org_id: Uuid, count: i32) -> Result<Vec<Uuid>, sqlx::Error> {
    let mut ids = Vec::new();
    for _ in 0..count {
        let seat = LicenseSeat::new(license_id, org_id);
        sqlx::query(
            r#"INSERT INTO license_seats
               (seat_id, license_id, org_id, is_active, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6)"#
        )
        .bind(seat.seat_id)
        .bind(seat.license_id)
        .bind(seat.org_id)
        .bind(seat.is_active)
        .bind(seat.created_at)
        .bind(seat.updated_at)
        .execute(pool)
        .await?;
        ids.push(seat.seat_id);
    }
    info!(license_id = %license_id, count = count, "Created license seats");
    Ok(ids)
}

pub async fn list_seats(pool: &PgPool, license_id: Uuid) -> Result<Vec<LicenseSeat>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SeatRow>(
        r#"SELECT seat_id, license_id, org_id, assigned_to, assigned_user_id, assigned_asset,
           assigned_target_id, notes, checkout_at, checkin_at, is_active, created_at, updated_at
           FROM license_seats WHERE license_id = $1 AND is_active = true ORDER BY created_at"#
    )
    .bind(license_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn assign_seat(
    pool: &PgPool,
    seat_id: Uuid,
    assigned_to: &str,
    assigned_asset: Option<&str>,
    assigned_target_id: Option<Uuid>,
    notes: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE license_seats SET
           assigned_to = $2, assigned_asset = $3, assigned_target_id = $4,
           notes = $5, checkout_at = now(), checkin_at = NULL, updated_at = now()
           WHERE seat_id = $1"#
    )
    .bind(seat_id)
    .bind(assigned_to)
    .bind(assigned_asset)
    .bind(assigned_target_id)
    .bind(notes)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn release_seat(pool: &PgPool, seat_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE license_seats SET
           assigned_to = NULL, assigned_user_id = NULL, assigned_asset = NULL,
           assigned_target_id = NULL, checkin_at = now(), updated_at = now()
           WHERE seat_id = $1"#
    )
    .bind(seat_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_available_seat(pool: &PgPool, license_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    let row = sqlx::query_as::<_, (Uuid,)>(
        "SELECT seat_id FROM license_seats WHERE license_id = $1 AND is_active = true AND assigned_to IS NULL LIMIT 1"
    )
    .bind(license_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}

// ============================================================================
// Audit log
// ============================================================================

pub async fn insert_audit_entry(pool: &PgPool, entry: &LicenseAuditEntry) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO license_audit_log
           (audit_id, license_id, org_id, action, actor, changes, details, created_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#
    )
    .bind(entry.audit_id)
    .bind(entry.license_id)
    .bind(entry.org_id)
    .bind(&entry.action)
    .bind(&entry.actor)
    .bind(&entry.changes)
    .bind(&entry.details)
    .bind(entry.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_audit_log(
    pool: &PgPool,
    license_id: Option<Uuid>,
    org_id: Uuid,
    limit: i64,
) -> Result<Vec<LicenseAuditEntry>, sqlx::Error> {
    let rows = if let Some(lid) = license_id {
        sqlx::query_as::<_, AuditRow>(
            r#"SELECT audit_id, license_id, org_id, action, actor, changes, details, created_at
               FROM license_audit_log WHERE license_id = $1 AND org_id = $2
               ORDER BY created_at DESC LIMIT $3"#
        )
        .bind(lid)
        .bind(org_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, AuditRow>(
            r#"SELECT audit_id, license_id, org_id, action, actor, changes, details, created_at
               FROM license_audit_log WHERE org_id = $1
               ORDER BY created_at DESC LIMIT $2"#
        )
        .bind(org_id)
        .bind(limit)
        .fetch_all(pool)
        .await?
    };

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

// ============================================================================
// Compliance summary
// ============================================================================

pub async fn get_compliance_summary(pool: &PgPool, org_id: Uuid) -> Result<ComplianceSummary, sqlx::Error> {
    let row = sqlx::query_as::<_, ComplianceRow>(
        r#"SELECT
           COUNT(*) as total_licenses,
           COUNT(*) FILTER (WHERE status = 'active') as active_licenses,
           COUNT(*) FILTER (WHERE status = 'expired') as expired_licenses,
           COUNT(*) FILTER (WHERE status = 'active' AND expiry_date IS NOT NULL AND expiry_date BETWEEN now() AND now() + INTERVAL '90 days') as expiring_soon,
           COALESCE(SUM(seat_count), 0) as total_seats,
           COALESCE(SUM(seat_count) - (SELECT COUNT(*) FROM license_seats ls WHERE ls.org_id = $1 AND ls.is_active = true AND ls.assigned_to IS NULL), 0) as assigned_seats,
           COALESCE((SELECT COUNT(*) FROM license_seats ls WHERE ls.org_id = $1 AND ls.is_active = true AND ls.assigned_to IS NULL), 0) as free_seats,
           0::bigint as over_licensed,
           0::bigint as under_licensed,
           COALESCE(SUM(total_cost), 0) as total_cost_sum,
           COALESCE(SUM(CASE WHEN license_type = 'subscription' THEN total_cost ELSE 0 END), 0) as annual_cost
           FROM licenses WHERE org_id = $1"#
    )
    .bind(org_id)
    .fetch_one(pool)
    .await?;

    Ok(ComplianceSummary {
        total_licenses: row.total_licenses.unwrap_or(0),
        active_licenses: row.active_licenses.unwrap_or(0),
        expired_licenses: row.expired_licenses.unwrap_or(0),
        expiring_soon: row.expiring_soon.unwrap_or(0),
        total_seats: row.total_seats.unwrap_or(0),
        assigned_seats: row.assigned_seats.unwrap_or(0),
        free_seats: row.free_seats.unwrap_or(0),
        over_licensed: row.over_licensed.unwrap_or(0),
        under_licensed: row.under_licensed.unwrap_or(0),
        total_cost: row.total_cost_sum.unwrap_or(0.0),
        annual_cost: row.annual_cost.unwrap_or(0.0),
    })
}

pub async fn get_expiring_licenses(pool: &PgPool, org_id: Uuid, days: i32) -> Result<Vec<License>, sqlx::Error> {
    let rows = sqlx::query_as::<_, LicenseRow>(
        r#"SELECT l.*,
           COALESCE(cs.assigned_seats, 0) as computed_assigned,
           COALESCE(cs.free_seats, l.seat_count) as computed_free
           FROM licenses l
           LEFT JOIN license_compliance_summary cs ON cs.license_id = l.license_id
           WHERE l.org_id = $1 AND l.status = 'active'
           AND l.expiry_date IS NOT NULL
           AND l.expiry_date BETWEEN now() AND now() + ($2 || ' days')::INTERVAL
           ORDER BY l.expiry_date ASC"#
    )
    .bind(org_id)
    .bind(days.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().filter_map(|r| r.try_into().ok()).collect())
}

// ============================================================================
// Row types for sqlx
// ============================================================================

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct LicenseRow {
    license_id: Uuid,
    org_id: Uuid,
    name: String,
    serial_number: Option<String>,
    product_name: String,
    product_version: Option<String>,
    vendor: String,
    category: String,
    license_type: String,
    seat_count: i32,
    cost_per_seat: Option<f64>,
    total_cost: Option<f64>,
    currency: Option<String>,
    purchase_date: Option<NaiveDate>,
    expiry_date: Option<NaiveDate>,
    renewal_date: Option<NaiveDate>,
    termination_date: Option<NaiveDate>,
    po_number: Option<String>,
    order_number: Option<String>,
    supplier_name: Option<String>,
    supplier_email: Option<String>,
    supplier_phone: Option<String>,
    contract_url: Option<String>,
    notes: Option<String>,
    is_maintained: bool,
    is_reassignable: bool,
    status: String,
    environment: String,
    tags: serde_json::Value,
    metadata: serde_json::Value,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    computed_assigned: Option<i64>,
    computed_free: Option<i64>,
}

impl TryFrom<LicenseRow> for License {
    type Error = String;
    fn try_from(r: LicenseRow) -> Result<Self, String> {
        let category = match r.category.as_str() {
            "software" => LicenseCategory::Software,
            "hardware" => LicenseCategory::Hardware,
            "cloud" => LicenseCategory::Cloud,
            "saas" => LicenseCategory::Saas,
            "perpetual" => LicenseCategory::Perpetual,
            _ => LicenseCategory::Software,
        };
        let license_type = match r.license_type.as_str() {
            "per_seat" => LicenseType::PerSeat,
            "per_device" => LicenseType::PerDevice,
            "site" => LicenseType::Site,
            "enterprise" => LicenseType::Enterprise,
            "concurrent" => LicenseType::Concurrent,
            "subscription" => LicenseType::Subscription,
            _ => LicenseType::PerSeat,
        };

        Ok(License {
            license_id: r.license_id,
            org_id: r.org_id,
            name: r.name,
            serial_number: r.serial_number,
            product_name: r.product_name,
            product_version: r.product_version,
            vendor: r.vendor,
            category,
            license_type,
            seat_count: r.seat_count,
            cost_per_seat: r.cost_per_seat,
            total_cost: r.total_cost,
            currency: r.currency.unwrap_or_else(|| "USD".into()),
            purchase_date: r.purchase_date,
            expiry_date: r.expiry_date,
            renewal_date: r.renewal_date,
            termination_date: r.termination_date,
            po_number: r.po_number,
            order_number: r.order_number,
            supplier_name: r.supplier_name,
            supplier_email: r.supplier_email,
            supplier_phone: r.supplier_phone,
            contract_url: r.contract_url,
            notes: r.notes,
            is_maintained: r.is_maintained,
            is_reassignable: r.is_reassignable,
            status: LicenseStatus::from_str(&r.status),
            environment: r.environment,
            tags: serde_json::from_value(r.tags).unwrap_or_default(),
            metadata: r.metadata,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
            assigned_seats: r.computed_assigned.unwrap_or(0) as i32,
            free_seats: r.computed_free.unwrap_or(r.seat_count as i64) as i32,
        })
    }
}

#[derive(sqlx::FromRow)]
struct SeatRow {
    seat_id: Uuid,
    license_id: Uuid,
    org_id: Uuid,
    assigned_to: Option<String>,
    assigned_user_id: Option<Uuid>,
    assigned_asset: Option<String>,
    assigned_target_id: Option<Uuid>,
    notes: Option<String>,
    checkout_at: Option<DateTime<Utc>>,
    checkin_at: Option<DateTime<Utc>>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<SeatRow> for LicenseSeat {
    fn from(r: SeatRow) -> Self {
        LicenseSeat {
            seat_id: r.seat_id,
            license_id: r.license_id,
            org_id: r.org_id,
            assigned_to: r.assigned_to,
            assigned_user_id: r.assigned_user_id,
            assigned_asset: r.assigned_asset,
            assigned_target_id: r.assigned_target_id,
            notes: r.notes,
            checkout_at: r.checkout_at,
            checkin_at: r.checkin_at,
            is_active: r.is_active,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct AuditRow {
    audit_id: Uuid,
    license_id: Uuid,
    org_id: Uuid,
    action: String,
    actor: String,
    changes: serde_json::Value,
    details: Option<String>,
    created_at: DateTime<Utc>,
}

impl From<AuditRow> for LicenseAuditEntry {
    fn from(r: AuditRow) -> Self {
        LicenseAuditEntry {
            audit_id: r.audit_id,
            license_id: r.license_id,
            org_id: r.org_id,
            action: r.action,
            actor: r.actor,
            changes: r.changes,
            details: r.details,
            created_at: r.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ComplianceRow {
    total_licenses: Option<i64>,
    active_licenses: Option<i64>,
    expired_licenses: Option<i64>,
    expiring_soon: Option<i64>,
    total_seats: Option<i64>,
    assigned_seats: Option<i64>,
    free_seats: Option<i64>,
    over_licensed: Option<i64>,
    under_licensed: Option<i64>,
    total_cost_sum: Option<f64>,
    annual_cost: Option<f64>,
}
