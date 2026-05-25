-- Zentra Annotations (Grafana-inspired)
-- Migration 006: Annotation events for deployment markers, incidents, alerts

CREATE TABLE IF NOT EXISTS annotations (
    annotation_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    title           TEXT NOT NULL,
    text            TEXT NOT NULL DEFAULT '',
    annotation_type TEXT NOT NULL DEFAULT 'manual'
                    CHECK (annotation_type IN ('manual', 'deployment', 'alert',
                                               'incident', 'maintenance', 'change')),
    tags            TEXT[] NOT NULL DEFAULT '{}',
    target_id       UUID REFERENCES targets(target_id) ON DELETE SET NULL,
    dashboard_id    TEXT,
    panel_id        TEXT,
    epoch_start     BIGINT NOT NULL,
    epoch_end       BIGINT,
    created_by      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_annotations_org ON annotations(org_id);
CREATE INDEX IF NOT EXISTS idx_annotations_type ON annotations(annotation_type);
CREATE INDEX IF NOT EXISTS idx_annotations_epoch ON annotations(epoch_start, epoch_end);
CREATE INDEX IF NOT EXISTS idx_annotations_dashboard ON annotations(dashboard_id)
    WHERE dashboard_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_annotations_target ON annotations(target_id)
    WHERE target_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_annotations_tags ON annotations USING gin(tags);

-- Dashboard configurations (for export/import)
CREATE TABLE IF NOT EXISTS dashboard_configs (
    dashboard_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    config_json     JSONB NOT NULL DEFAULT '{}',
    variables       JSONB NOT NULL DEFAULT '[]',
    panels          JSONB NOT NULL DEFAULT '[]',
    version         INT NOT NULL DEFAULT 1,
    created_by      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_dashboards_org ON dashboard_configs(org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_dashboards_name ON dashboard_configs(org_id, name);

-- Template variables
CREATE TABLE IF NOT EXISTS dashboard_variables (
    variable_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dashboard_id    UUID NOT NULL REFERENCES dashboard_configs(dashboard_id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    label           TEXT,
    variable_type   TEXT NOT NULL DEFAULT 'query'
                    CHECK (variable_type IN ('query', 'custom', 'constant',
                                             'datasource', 'interval', 'textbox')),
    query           TEXT,
    options         JSONB NOT NULL DEFAULT '[]',
    current_value   TEXT,
    multi           BOOLEAN NOT NULL DEFAULT FALSE,
    include_all     BOOLEAN NOT NULL DEFAULT FALSE,
    sort_order      INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_variables_dashboard ON dashboard_variables(dashboard_id);
