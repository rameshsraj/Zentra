-- Hosts discovered by an agent via network scan.
-- Users opt-in individual IPs from this table to create monitored targets.
CREATE TABLE IF NOT EXISTS discovered_hosts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id        UUID NOT NULL REFERENCES agent_fleet(agent_id) ON DELETE CASCADE,
    org_id          UUID NOT NULL REFERENCES organizations(org_id),
    ip_address      TEXT NOT NULL,
    hostname        TEXT,
    open_ports      JSONB NOT NULL DEFAULT '[]',   -- [22, 80, 443, ...]
    os_hint         TEXT,                           -- "linux", "windows", "unknown"
    first_seen      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    opted_in        BOOLEAN NOT NULL DEFAULT FALSE, -- true = user added as target
    ignored         BOOLEAN NOT NULL DEFAULT FALSE, -- true = user dismissed it
    target_id       UUID REFERENCES targets(target_id) ON DELETE SET NULL,
    CONSTRAINT uq_discovered_hosts_agent_ip UNIQUE (agent_id, ip_address)
);

CREATE INDEX IF NOT EXISTS idx_discovered_hosts_agent  ON discovered_hosts(agent_id);
CREATE INDEX IF NOT EXISTS idx_discovered_hosts_org    ON discovered_hosts(org_id);
CREATE INDEX IF NOT EXISTS idx_discovered_hosts_optin  ON discovered_hosts(org_id, opted_in);
