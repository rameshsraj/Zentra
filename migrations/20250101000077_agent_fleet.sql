CREATE TABLE IF NOT EXISTS agent_fleet (
    agent_id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id               UUID NOT NULL REFERENCES organizations(org_id),
    otlp_api_key_id      UUID NOT NULL UNIQUE REFERENCES otlp_api_keys(id),
    api_key_hash         TEXT NOT NULL UNIQUE,
    datacenter_name      TEXT NOT NULL,
    description          TEXT,
    region_label         TEXT,
    agent_version        TEXT,
    active_target_count  INTEGER NOT NULL DEFAULT 0,
    last_seen            TIMESTAMPTZ,
    ip_address           TEXT,
    os_name              TEXT,
    status               TEXT NOT NULL DEFAULT 'active',
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_agent_fleet_status
        CHECK (status IN ('active', 'stale', 'offline', 'deregistered'))
);

CREATE INDEX IF NOT EXISTS idx_agent_fleet_org_id ON agent_fleet(org_id);
CREATE INDEX IF NOT EXISTS idx_agent_fleet_last_seen ON agent_fleet(last_seen);