-- OTel-specific alert rules table referenced by /api/otel/alert-rules handlers.
-- Distinct from the generic `alert_rules` table — these only fire on OTel signals
-- (traces / metrics / logs / profiles) and use a richer condition spec.
CREATE TABLE IF NOT EXISTS otel_alert_rules (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    signal          TEXT NOT NULL,                -- 'traces' | 'metrics' | 'logs' | 'profiles'
    metric          TEXT NOT NULL,                -- e.g. 'error_rate', 'p95_duration_ms'
    condition       TEXT NOT NULL,                -- '>', '<', '>=', '<=', '=='
    threshold       DOUBLE PRECISION NOT NULL,
    window_seconds  INTEGER NOT NULL DEFAULT 300,
    severity        TEXT NOT NULL DEFAULT 'warning',
    description     TEXT,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chk_otel_alert_rules_signal CHECK (signal IN ('traces','metrics','logs','profiles')),
    CONSTRAINT chk_otel_alert_rules_severity CHECK (severity IN ('info','warning','critical'))
);

CREATE INDEX IF NOT EXISTS idx_otel_alert_rules_org     ON otel_alert_rules(org_id);
CREATE INDEX IF NOT EXISTS idx_otel_alert_rules_enabled ON otel_alert_rules(org_id, enabled);
