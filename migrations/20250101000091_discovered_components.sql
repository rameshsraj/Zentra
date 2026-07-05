-- Components discovered by an agent on its own host (local service detection).
-- Distinct from discovered_hosts (network scan) — these are services/processes
-- running locally on the agent machine detected via /proc, systemd, or port probing.
CREATE TABLE IF NOT EXISTS discovered_components (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        UUID NOT NULL REFERENCES agent_fleet(agent_id) ON DELETE CASCADE,
    org_id          UUID NOT NULL REFERENCES organizations(org_id),
    -- component identification
    component_type  TEXT NOT NULL,  -- "linux", "postgres", "mysql", "redis", "nginx", "apache", "oracle", "mongodb"
    component_name  TEXT NOT NULL,  -- human display name e.g. "PostgreSQL 15", "nginx"
    address         TEXT NOT NULL DEFAULT '127.0.0.1',
    port            INTEGER,        -- NULL if not applicable (e.g. OS itself)
    version         TEXT,           -- detected version string
    extra           JSONB NOT NULL DEFAULT '{}',  -- freeform metadata (process name, config path, etc.)
    -- state
    confirmed       BOOLEAN NOT NULL DEFAULT FALSE,  -- user confirmed → target created
    ignored         BOOLEAN NOT NULL DEFAULT FALSE,
    target_id       UUID REFERENCES targets(target_id) ON DELETE SET NULL,
    first_seen      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_discovered_comp_agent_type_port UNIQUE (agent_id, component_type, port)
);

CREATE INDEX IF NOT EXISTS idx_disc_comp_agent   ON discovered_components(agent_id);
CREATE INDEX IF NOT EXISTS idx_disc_comp_org     ON discovered_components(org_id);
CREATE INDEX IF NOT EXISTS idx_disc_comp_pending ON discovered_components(org_id, confirmed, ignored);
