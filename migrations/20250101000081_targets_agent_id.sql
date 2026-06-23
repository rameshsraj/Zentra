-- Add agent_id to targets so servers monitored via an agent skip SSH validation
ALTER TABLE targets ADD COLUMN IF NOT EXISTS agent_id UUID REFERENCES agent_fleet(agent_id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_targets_agent_id ON targets(agent_id) WHERE agent_id IS NOT NULL;
