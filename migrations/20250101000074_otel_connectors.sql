-- O10: Persist connector configuration
CREATE TABLE IF NOT EXISTS otel_connectors (
    id              UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    org_id          UUID NOT NULL,
    connector_type  TEXT NOT NULL,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    dimensions      JSONB NOT NULL DEFAULT '[]'::jsonb,
    interval_seconds BIGINT NOT NULL DEFAULT 60,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, connector_type)
);

CREATE INDEX IF NOT EXISTS idx_otel_connectors_org ON otel_connectors(org_id);
