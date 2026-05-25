-- SkyWalking P05: Topology Drill-through + Service Hierarchy
ALTER TABLE targets ADD COLUMN IF NOT EXISTS parent_target_id UUID REFERENCES targets(target_id) ON DELETE SET NULL;
ALTER TABLE targets ADD COLUMN IF NOT EXISTS hierarchy_depth  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE targets ADD COLUMN IF NOT EXISTS hierarchy_path   TEXT;
CREATE INDEX IF NOT EXISTS idx_targets_parent ON targets(parent_target_id);
