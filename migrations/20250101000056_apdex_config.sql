-- SkyWalking P04: Apdex Scoring on Services
CREATE TABLE IF NOT EXISTS apdex_config (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id         UUID NOT NULL,
    service_name   TEXT NOT NULL,
    satisfied_ms   INTEGER NOT NULL DEFAULT 200,
    tolerating_ms  INTEGER NOT NULL DEFAULT 800,
    UNIQUE(org_id, service_name)
);
