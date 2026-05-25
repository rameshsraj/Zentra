-- Zentra License Management Schema v1.0
-- Migration 003: License tracking, seat management, compliance, audit logging

-- ============================================================================
-- LICENSES — software/hardware license records
-- ============================================================================
CREATE TABLE IF NOT EXISTS licenses (
    license_id      UUID PRIMARY KEY,
    org_id          UUID NOT NULL,
    name            VARCHAR(255) NOT NULL,
    serial_number   VARCHAR(500),
    product_name    VARCHAR(255) NOT NULL,
    product_version VARCHAR(100),
    vendor          VARCHAR(255) NOT NULL,
    category        VARCHAR(100) NOT NULL DEFAULT 'software',  -- software, hardware, cloud, saas, perpetual
    license_type    VARCHAR(50) NOT NULL DEFAULT 'per_seat',   -- per_seat, per_device, site, enterprise, concurrent, subscription
    seat_count      INTEGER NOT NULL DEFAULT 1,
    cost_per_seat   NUMERIC(12, 2),
    total_cost      NUMERIC(14, 2),
    currency        VARCHAR(3) DEFAULT 'USD',
    purchase_date   DATE,
    expiry_date     DATE,
    renewal_date    DATE,
    termination_date DATE,
    po_number       VARCHAR(255),
    order_number    VARCHAR(255),
    supplier_name   VARCHAR(255),
    supplier_email  VARCHAR(255),
    supplier_phone  VARCHAR(100),
    contract_url    TEXT,
    notes           TEXT,
    is_maintained   BOOLEAN DEFAULT false,
    is_reassignable BOOLEAN DEFAULT true,
    status          VARCHAR(20) NOT NULL DEFAULT 'active',     -- active, expired, terminated, pending, suspended
    environment     VARCHAR(50) DEFAULT 'production',
    tags            JSONB DEFAULT '{}'::jsonb,
    metadata        JSONB DEFAULT '{}'::jsonb,
    created_by      VARCHAR(255),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_licenses_org ON licenses(org_id);
CREATE INDEX IF NOT EXISTS idx_licenses_vendor ON licenses(vendor);
CREATE INDEX IF NOT EXISTS idx_licenses_status ON licenses(status);
CREATE INDEX IF NOT EXISTS idx_licenses_expiry ON licenses(expiry_date);
CREATE INDEX IF NOT EXISTS idx_licenses_category ON licenses(category);

-- ============================================================================
-- LICENSE_SEATS — individual seat/entitlement records
-- ============================================================================
CREATE TABLE IF NOT EXISTS license_seats (
    seat_id         UUID PRIMARY KEY,
    license_id      UUID NOT NULL REFERENCES licenses(license_id) ON DELETE CASCADE,
    org_id          UUID NOT NULL,
    assigned_to     VARCHAR(255),           -- user name or email
    assigned_user_id UUID,                  -- optional FK to a users table
    assigned_asset  VARCHAR(255),           -- device/server name
    assigned_target_id UUID,                -- optional FK to targets table
    notes           TEXT,
    checkout_at     TIMESTAMPTZ,
    checkin_at      TIMESTAMPTZ,
    is_active       BOOLEAN DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_license_seats_license ON license_seats(license_id);
CREATE INDEX IF NOT EXISTS idx_license_seats_org ON license_seats(org_id);
CREATE INDEX IF NOT EXISTS idx_license_seats_assigned ON license_seats(assigned_to);

-- ============================================================================
-- LICENSE_ASSIGNMENTS — tracks which targets/assets are using a license
-- ============================================================================
CREATE TABLE IF NOT EXISTS license_assignments (
    assignment_id   UUID PRIMARY KEY,
    license_id      UUID NOT NULL REFERENCES licenses(license_id) ON DELETE CASCADE,
    seat_id         UUID REFERENCES license_seats(seat_id) ON DELETE SET NULL,
    org_id          UUID NOT NULL,
    target_id       UUID,                   -- linked infrastructure target
    assigned_to     VARCHAR(255) NOT NULL,  -- user or asset identifier
    assignment_type VARCHAR(50) NOT NULL DEFAULT 'user',  -- user, device, service
    assigned_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at     TIMESTAMPTZ,
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_license_assignments_license ON license_assignments(license_id);
CREATE INDEX IF NOT EXISTS idx_license_assignments_target ON license_assignments(target_id);

-- ============================================================================
-- LICENSE_AUDIT_LOG — immutable audit trail for all license changes
-- ============================================================================
CREATE TABLE IF NOT EXISTS license_audit_log (
    audit_id        UUID PRIMARY KEY,
    license_id      UUID NOT NULL REFERENCES licenses(license_id) ON DELETE CASCADE,
    org_id          UUID NOT NULL,
    action          VARCHAR(50) NOT NULL,   -- created, updated, deleted, seat_assigned, seat_released, renewed, expired
    actor           VARCHAR(255) NOT NULL DEFAULT 'system',
    changes         JSONB DEFAULT '{}'::jsonb,
    details         TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_license_audit_license ON license_audit_log(license_id);
CREATE INDEX IF NOT EXISTS idx_license_audit_org ON license_audit_log(org_id);
CREATE INDEX IF NOT EXISTS idx_license_audit_action ON license_audit_log(action);
CREATE INDEX IF NOT EXISTS idx_license_audit_created ON license_audit_log(created_at DESC);

-- ============================================================================
-- VIEWS — compliance and expiry helpers
-- ============================================================================
CREATE OR REPLACE VIEW license_compliance_summary AS
SELECT
    l.license_id,
    l.org_id,
    l.name,
    l.vendor,
    l.product_name,
    l.seat_count AS total_seats,
    COUNT(ls.seat_id) FILTER (WHERE ls.is_active AND ls.assigned_to IS NOT NULL) AS assigned_seats,
    l.seat_count - COUNT(ls.seat_id) FILTER (WHERE ls.is_active AND ls.assigned_to IS NOT NULL) AS free_seats,
    l.expiry_date,
    l.status,
    CASE
        WHEN COUNT(ls.seat_id) FILTER (WHERE ls.is_active AND ls.assigned_to IS NOT NULL) > l.seat_count THEN 'over_licensed'
        WHEN COUNT(ls.seat_id) FILTER (WHERE ls.is_active AND ls.assigned_to IS NOT NULL) = l.seat_count THEN 'fully_utilized'
        WHEN COUNT(ls.seat_id) FILTER (WHERE ls.is_active AND ls.assigned_to IS NOT NULL)::float / GREATEST(l.seat_count, 1)::float < 0.5 THEN 'under_licensed'
        ELSE 'ok'
    END AS compliance_status,
    l.total_cost,
    l.cost_per_seat
FROM licenses l
LEFT JOIN license_seats ls ON ls.license_id = l.license_id
GROUP BY l.license_id;

CREATE OR REPLACE VIEW licenses_expiring_soon AS
SELECT * FROM licenses
WHERE status = 'active'
  AND expiry_date IS NOT NULL
  AND expiry_date BETWEEN now() AND now() + INTERVAL '90 days'
ORDER BY expiry_date ASC;
