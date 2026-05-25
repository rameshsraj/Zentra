-- SkyWalking P01: Entity-Scoped Dashboard Model
-- Adds entity binding to dashboards and a template system for per-entity-type dashboards.

ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS entity_type  TEXT;
ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS entity_scope TEXT;
-- entity_type values: 'server'|'database'|'k8s_node'|'k8s_pod'|'network_device'|'cilium'|'service'|'global'
CREATE INDEX IF NOT EXISTS idx_dashboards_entity ON dashboards(entity_type, entity_scope);

CREATE TABLE IF NOT EXISTS dashboard_templates (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    description TEXT,
    panels      JSONB NOT NULL DEFAULT '[]',
    is_builtin  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dashboard_templates_entity_name
    ON dashboard_templates(entity_type, name) WHERE is_builtin = TRUE;
