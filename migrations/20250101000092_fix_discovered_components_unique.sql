-- Fix the unique constraint on discovered_components so that NULL ports
-- don't result in duplicate rows for the same component type (e.g. Linux Host).
-- Replace the partial unique index with one that treats NULL port as -1.

-- Remove old duplicate rows first (keep the most recently seen one)
DELETE FROM discovered_components a
USING discovered_components b
WHERE a.agent_id = b.agent_id
  AND a.component_type = b.component_type
  AND a.port IS NULL
  AND b.port IS NULL
  AND a.first_seen < b.first_seen;

-- Drop old constraint
ALTER TABLE discovered_components
    DROP CONSTRAINT IF EXISTS uq_discovered_comp_agent_type_port;

-- New constraint using COALESCE so NULL port is treated as -1 (unique per agent+type+port)
CREATE UNIQUE INDEX IF NOT EXISTS uq_disc_comp_agent_type_port
    ON discovered_components (agent_id, component_type, COALESCE(port, -1));
