-- SLO targets table for user-defined service level objectives
CREATE TABLE IF NOT EXISTS slo_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id UUID NOT NULL,
    service_name TEXT NOT NULL,
    metric TEXT NOT NULL CHECK (metric IN ('error_rate', 'p99_latency', 'p95_latency', 'availability')),
    threshold DOUBLE PRECISION NOT NULL,
    window_days INTEGER NOT NULL DEFAULT 30,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_slo_targets_org_id ON slo_targets (org_id);
CREATE INDEX IF NOT EXISTS idx_slo_targets_service ON slo_targets (org_id, service_name);
