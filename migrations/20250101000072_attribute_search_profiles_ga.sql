-- OT16: Attribute-level search (JSONB GIN indexes) + profiles GA tracker.

-- Create otel_profiles table if it doesn't exist (may have been created outside migrations)
CREATE TABLE IF NOT EXISTS otel_profiles (
    id              UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    profile_id      TEXT NOT NULL,
    service_name    TEXT NOT NULL DEFAULT 'unknown',
    profile_type    TEXT NOT NULL DEFAULT 'cpu',
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_ns     BIGINT NOT NULL DEFAULT 0,
    sample_count    INTEGER,
    attributes      JSONB NOT NULL DEFAULT '{}',
    payload         BYTEA,
    PRIMARY KEY (id, started_at)
);
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
        PERFORM create_hypertable('otel_profiles', 'started_at', if_not_exists => TRUE, migrate_data => TRUE);
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'otel_profiles hypertable creation skipped: %', SQLERRM;
END $$;

-- GIN indexes for attribute-level search across all signal tables
-- (idx_traces_tags_gin may already exist from 000061 — IF NOT EXISTS handles it)
CREATE INDEX IF NOT EXISTS idx_traces_tags_gin ON traces USING gin (tags);
CREATE INDEX IF NOT EXISTS idx_logs_tags_gin ON logs USING gin (tags);
CREATE INDEX IF NOT EXISTS idx_otel_profiles_attrs_gin ON otel_profiles USING gin (attributes);

-- Attribute search helper: extract top-k attribute keys with counts
-- Note: logs uses 'tags' column (not 'metadata')
CREATE OR REPLACE FUNCTION otlp_attribute_keys(signal TEXT, org UUID, lim INT DEFAULT 50)
RETURNS TABLE(key TEXT, count BIGINT) AS $$
BEGIN
    IF signal = 'traces' THEN
        RETURN QUERY
            SELECT k.key, COUNT(*) AS count
            FROM traces, jsonb_object_keys(tags) AS k(key)
            WHERE org_id = org AND started_at > NOW() - INTERVAL '24 hours'
            GROUP BY k.key ORDER BY count DESC LIMIT lim;
    ELSIF signal = 'logs' THEN
        RETURN QUERY
            SELECT k.key, COUNT(*) AS count
            FROM logs, jsonb_object_keys(tags) AS k(key)
            WHERE org_id = org AND timestamp > NOW() - INTERVAL '24 hours'
            GROUP BY k.key ORDER BY count DESC LIMIT lim;
    ELSIF signal = 'profiles' THEN
        RETURN QUERY
            SELECT k.key, COUNT(*) AS count
            FROM otel_profiles, jsonb_object_keys(attributes) AS k(key)
            WHERE org_id = org AND started_at > NOW() - INTERVAL '24 hours'
            GROUP BY k.key ORDER BY count DESC LIMIT lim;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Profiles GA readiness tracker view
CREATE OR REPLACE VIEW otel_profiles_ga_status AS
SELECT
    org_id,
    COUNT(*) AS total_profiles,
    COUNT(DISTINCT service_name) AS unique_services,
    COUNT(DISTINCT profile_id) AS unique_profile_ids,
    MAX(started_at) AS last_profile_at,
    CASE
        WHEN COUNT(*) >= 100 AND COUNT(DISTINCT service_name) >= 1 THEN 'GA_READY'
        WHEN COUNT(*) > 0 THEN 'BETA'
        ELSE 'NO_DATA'
    END AS ga_status
FROM otel_profiles
WHERE started_at > NOW() - INTERVAL '7 days'
GROUP BY org_id;
