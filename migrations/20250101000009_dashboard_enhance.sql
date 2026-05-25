-- ============================================================================
-- Migration 009: Dashboard Folders, Versioning, Library Panels
-- ============================================================================

-- Dashboard folders (hierarchical)
CREATE TABLE IF NOT EXISTS dashboard_folders (
    folder_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    parent_id       UUID REFERENCES dashboard_folders(folder_id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    slug            TEXT NOT NULL,
    description     TEXT,
    sort_order      INT DEFAULT 0,
    created_by      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, parent_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_dashboard_folders_org ON dashboard_folders(org_id);
CREATE INDEX IF NOT EXISTS idx_dashboard_folders_parent ON dashboard_folders(parent_id);

-- Add folder reference to dashboards
ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS folder_id UUID REFERENCES dashboard_folders(folder_id) ON DELETE SET NULL;
ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS starred BOOLEAN DEFAULT false;
ALTER TABLE dashboards ADD COLUMN IF NOT EXISTS sort_order INT DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_dashboards_folder ON dashboards(folder_id);

-- Dashboard version history
CREATE TABLE IF NOT EXISTS dashboard_versions (
    version_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    dashboard_id    UUID NOT NULL,
    org_id          UUID NOT NULL,
    version_number  INT NOT NULL,
    data            JSONB NOT NULL,
    message         TEXT,
    created_by      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(dashboard_id, version_number)
);

CREATE INDEX IF NOT EXISTS idx_dashboard_versions_dash ON dashboard_versions(dashboard_id);
CREATE INDEX IF NOT EXISTS idx_dashboard_versions_org ON dashboard_versions(org_id);

-- Library panels (reusable panel definitions shared across dashboards)
CREATE TABLE IF NOT EXISTS library_panels (
    panel_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    folder_id       UUID REFERENCES dashboard_folders(folder_id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    panel_type      TEXT NOT NULL,
    panel_config    JSONB NOT NULL DEFAULT '{}',
    tags            TEXT[] DEFAULT '{}',
    version         INT NOT NULL DEFAULT 1,
    created_by      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_library_panels_org ON library_panels(org_id);
CREATE INDEX IF NOT EXISTS idx_library_panels_folder ON library_panels(folder_id);
CREATE INDEX IF NOT EXISTS idx_library_panels_type ON library_panels(panel_type);

-- Library panel usage tracking (which dashboards use which library panels)
CREATE TABLE IF NOT EXISTS library_panel_usage (
    usage_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    panel_id        UUID NOT NULL REFERENCES library_panels(panel_id) ON DELETE CASCADE,
    dashboard_id    UUID NOT NULL,
    panel_key       TEXT NOT NULL, -- panel key within the dashboard
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(panel_id, dashboard_id, panel_key)
);

CREATE INDEX IF NOT EXISTS idx_library_panel_usage_panel ON library_panel_usage(panel_id);
CREATE INDEX IF NOT EXISTS idx_library_panel_usage_dash ON library_panel_usage(dashboard_id);

-- Data transformation pipeline definitions
CREATE TABLE IF NOT EXISTS data_transforms (
    transform_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    -- Array of transform steps: [{type: "filter", config: {...}}, {type: "aggregate", config: {...}}]
    steps           JSONB NOT NULL DEFAULT '[]',
    input_type      TEXT NOT NULL DEFAULT 'metrics',
    output_type     TEXT NOT NULL DEFAULT 'metrics',
    enabled         BOOLEAN DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_data_transforms_org ON data_transforms(org_id);

-- Insert default root folders
INSERT INTO dashboard_folders (folder_id, org_id, parent_id, name, slug, description)
VALUES
    ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', NULL, 'General', 'general', 'Default folder'),
    ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', NULL, 'Infrastructure', 'infrastructure', 'Infrastructure dashboards'),
    ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000001', NULL, 'Network', 'network', 'Network monitoring dashboards'),
    ('00000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', NULL, 'Applications', 'applications', 'Application dashboards'),
    ('00000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000001', NULL, 'Alerts', 'alerts', 'Alert dashboards')
ON CONFLICT DO NOTHING;
