-- SECURITY-1/2/3: Org enrollment keys + per-agent operational keys + audit trail

CREATE TABLE IF NOT EXISTS org_enrollment_keys (
    id                         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id                     UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
    key_hash                   TEXT NOT NULL UNIQUE,
    display_prefix             TEXT NOT NULL,
    enabled                    BOOLEAN NOT NULL DEFAULT TRUE,
    max_registrations_per_hour INTEGER NOT NULL DEFAULT 5,
    allowed_cidr               INET,
    require_approval           BOOLEAN NOT NULL DEFAULT FALSE,
    expires_at                 TIMESTAMPTZ,
    created_by                 UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at               TIMESTAMPTZ,
    registration_count         INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_org_enrollment_keys_org_id
    ON org_enrollment_keys(org_id, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_api_keys (
    id                        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id                    UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
    agent_id                  UUID NOT NULL REFERENCES agent_fleet(agent_id) ON DELETE CASCADE,
    key_hash                  TEXT NOT NULL UNIQUE,
    display_prefix            TEXT NOT NULL,
    enabled                   BOOLEAN NOT NULL DEFAULT TRUE,
    registered_ip             INET,
    ip_locked                 BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at                TIMESTAMPTZ NOT NULL,
    last_used                 TIMESTAMPTZ,
    last_rotated_at           TIMESTAMPTZ,
    enrollment_key_id         UUID REFERENCES org_enrollment_keys(id) ON DELETE SET NULL,
    replaced_by_key_id        UUID REFERENCES agent_api_keys(id) ON DELETE SET NULL,
    grace_until               TIMESTAMPTZ,
    revoked_at                TIMESTAMPTZ,
    created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at                TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agent_api_keys_agent
    ON agent_api_keys(agent_id, enabled);
CREATE INDEX IF NOT EXISTS idx_agent_api_keys_org
    ON agent_api_keys(org_id, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_registration_audit (
    id                   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id               UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
    enrollment_key_id    UUID REFERENCES org_enrollment_keys(id) ON DELETE SET NULL,
    agent_id             UUID REFERENCES agent_fleet(agent_id) ON DELETE SET NULL,
    request_ip           INET,
    hostname             TEXT,
    status               TEXT NOT NULL,
    reason               TEXT,
    payload              JSONB NOT NULL DEFAULT '{}',
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_agent_registration_audit_org
    ON agent_registration_audit(org_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_registration_audit_enrollment_key
    ON agent_registration_audit(enrollment_key_id, created_at DESC);

ALTER TABLE agent_fleet
    ADD COLUMN IF NOT EXISTS enrollment_key_id UUID REFERENCES org_enrollment_keys(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS hostname TEXT;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM pg_constraint
        WHERE conname = 'chk_agent_fleet_status'
          AND conrelid = 'agent_fleet'::regclass
    ) THEN
        ALTER TABLE agent_fleet DROP CONSTRAINT chk_agent_fleet_status;
    END IF;
END $$;

ALTER TABLE agent_fleet
    ADD CONSTRAINT chk_agent_fleet_status
    CHECK (status IN ('active', 'stale', 'offline', 'deregistered', 'pending_approval'));
