-- Add per-tenant demo_mode flag to org_settings.
-- Existing orgs default to demo=true (safe: no real data sources configured yet).
ALTER TABLE org_settings ADD COLUMN IF NOT EXISTS demo_mode BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE org_settings ADD COLUMN IF NOT EXISTS demo_mode_updated_at TIMESTAMPTZ;

-- Index for fast lookup at request time
CREATE INDEX IF NOT EXISTS idx_org_settings_demo_mode ON org_settings (org_id, demo_mode);
