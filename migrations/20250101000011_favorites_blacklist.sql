-- 011: Persist network favorites and blacklist to DB
-- These were previously in-memory only

CREATE TABLE IF NOT EXISTS network_favorites (
    id TEXT PRIMARY KEY,
    org_id UUID NOT NULL,
    label TEXT NOT NULL,
    ip_or_host TEXT NOT NULL,
    value JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS network_blacklist (
    id TEXT PRIMARY KEY,
    org_id UUID NOT NULL,
    ip TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT 'Manually blacklisted',
    source TEXT NOT NULL DEFAULT 'manual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_favorites_org ON network_favorites(org_id);
CREATE INDEX IF NOT EXISTS idx_blacklist_org ON network_blacklist(org_id);
CREATE INDEX IF NOT EXISTS idx_blacklist_ip ON network_blacklist(ip);
