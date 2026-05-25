-- Zentra Dashboard Management
-- Migration 007: Dashboards, panels, template variables, export history

CREATE TABLE IF NOT EXISTS dashboards (
    dashboard_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    version         INTEGER NOT NULL DEFAULT 1,
    created_by      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_dashboards_org ON dashboards(org_id);
CREATE INDEX idx_dashboards_name ON dashboards(org_id, name);

-- Panels within a dashboard
CREATE TABLE IF NOT EXISTS dashboard_panels (
    panel_id        TEXT NOT NULL,
    dashboard_id    UUID NOT NULL REFERENCES dashboards(dashboard_id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    panel_type      TEXT NOT NULL DEFAULT 'timeseries'
                    CHECK (panel_type IN ('timeseries', 'gauge', 'stat', 'table',
                                          'barchart', 'piechart', 'heatmap', 'logs', 'text')),
    metric_query    TEXT NOT NULL DEFAULT '',
    width           INTEGER NOT NULL DEFAULT 6,
    height          INTEGER NOT NULL DEFAULT 4,
    position_x      INTEGER NOT NULL DEFAULT 0,
    position_y      INTEGER NOT NULL DEFAULT 0,
    options         JSONB NOT NULL DEFAULT '{}',
    PRIMARY KEY (dashboard_id, panel_id)
);

CREATE INDEX idx_panels_dashboard ON dashboard_panels(dashboard_id);

-- Template variables for dashboards
CREATE TABLE IF NOT EXISTS dashboard_variables (
    variable_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dashboard_id    UUID NOT NULL REFERENCES dashboards(dashboard_id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    label           TEXT,
    variable_type   TEXT NOT NULL DEFAULT 'custom'
                    CHECK (variable_type IN ('query', 'custom', 'interval',
                                              'textbox', 'constant', 'datasource')),
    query           TEXT,
    options         TEXT[] NOT NULL DEFAULT '{}',
    current_value   TEXT,
    multi           BOOLEAN NOT NULL DEFAULT false,
    include_all     BOOLEAN NOT NULL DEFAULT false,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    UNIQUE (dashboard_id, name)
);

CREATE INDEX idx_variables_dashboard ON dashboard_variables(dashboard_id);

-- Dashboard export/import history
CREATE TABLE IF NOT EXISTS dashboard_exports (
    export_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dashboard_id    UUID NOT NULL REFERENCES dashboards(dashboard_id) ON DELETE CASCADE,
    exported_by     TEXT,
    export_format   TEXT NOT NULL DEFAULT 'Zentra-dashboard-v1',
    export_json     JSONB NOT NULL,
    exported_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_exports_dashboard ON dashboard_exports(dashboard_id);

-- View: full dashboard with panel/variable counts
CREATE OR REPLACE VIEW dashboard_summary AS
SELECT
    d.dashboard_id,
    d.org_id,
    d.name,
    d.description,
    d.version,
    d.created_by,
    d.created_at,
    d.updated_at,
    COUNT(DISTINCT dp.panel_id) AS panel_count,
    COUNT(DISTINCT dv.variable_id) AS variable_count,
    (SELECT COUNT(*) FROM dashboard_exports de WHERE de.dashboard_id = d.dashboard_id) AS export_count
FROM dashboards d
LEFT JOIN dashboard_panels dp ON dp.dashboard_id = d.dashboard_id
LEFT JOIN dashboard_variables dv ON dv.dashboard_id = d.dashboard_id
GROUP BY d.dashboard_id, d.org_id, d.name, d.description, d.version,
         d.created_by, d.created_at, d.updated_at;

-- View: most recently used dashboards
CREATE OR REPLACE VIEW recent_dashboards AS
SELECT * FROM dashboards
ORDER BY updated_at DESC
LIMIT 10;
