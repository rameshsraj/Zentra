-- Add missing columns to otlp_api_keys required by agent registration (agent_fleet migration 77)
-- These columns are referenced in handlers::agents::register_agent and must exist before
-- an agent can be registered via /api/agents/register.

ALTER TABLE otlp_api_keys ADD COLUMN IF NOT EXISTS key_prefix        TEXT;
ALTER TABLE otlp_api_keys ADD COLUMN IF NOT EXISTS rate_limit_rpm    INTEGER     NOT NULL DEFAULT 10000;
ALTER TABLE otlp_api_keys ADD COLUMN IF NOT EXISTS max_payload_bytes INTEGER     NOT NULL DEFAULT 5242880;
ALTER TABLE otlp_api_keys ADD COLUMN IF NOT EXISTS enabled           BOOLEAN     NOT NULL DEFAULT TRUE;
ALTER TABLE otlp_api_keys ADD COLUMN IF NOT EXISTS updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW();

CREATE INDEX IF NOT EXISTS idx_otlp_api_keys_prefix  ON otlp_api_keys(key_prefix);
CREATE INDEX IF NOT EXISTS idx_otlp_api_keys_enabled ON otlp_api_keys(enabled) WHERE enabled = true;
