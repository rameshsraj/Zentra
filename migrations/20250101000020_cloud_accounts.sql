-- Cloud account registrations for AWS, Azure, GCP monitoring (SDK integration deferred to P2)
CREATE TABLE IF NOT EXISTS cloud_accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL,
    provider        TEXT NOT NULL,        -- aws | azure | gcp | other
    account_id      TEXT,                 -- AWS account ID / Azure subscription / GCP project
    region          TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',  -- pending | pending_validation | active | error
    last_sync       TIMESTAMPTZ,
    resource_count  BIGINT DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cloud_accounts_provider ON cloud_accounts (provider);
CREATE INDEX IF NOT EXISTS idx_cloud_accounts_status   ON cloud_accounts (status);
