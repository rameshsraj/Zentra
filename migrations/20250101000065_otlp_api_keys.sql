-- OTel P12: Security & Compliance Posture — OTLP API Keys
CREATE TABLE IF NOT EXISTS otlp_api_keys (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    key_hash    TEXT NOT NULL,
    name        TEXT NOT NULL,
    permissions TEXT[] NOT NULL DEFAULT '{traces,metrics,logs,profiles}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ,
    last_used   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_otlp_api_keys_org ON otlp_api_keys(org_id);
CREATE INDEX IF NOT EXISTS idx_otlp_api_keys_hash ON otlp_api_keys(key_hash);
