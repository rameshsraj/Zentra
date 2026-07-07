-- Migration 079: Datacenters table + miscellaneous missing tables
-- All CREATE TABLE / CREATE INDEX use IF NOT EXISTS so this is safe to re-run.

-- ── Datacenters ───────────────────────────────────────────────────────────────
-- Used by the Fleet Install Wizard "Register Datacenter" step.
CREATE TABLE IF NOT EXISTS datacenters (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID        NOT NULL DEFAULT '00000000-0000-0000-0000-000000000001',
    name        TEXT        NOT NULL,
    region      TEXT,
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, name)
);

CREATE INDEX IF NOT EXISTS idx_datacenters_org ON datacenters (org_id);
