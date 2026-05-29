-- OT16: Attribute-level search (JSONB GIN indexes) + profiles GA tracker.

-- GIN indexes for attribute-level search across all signal tables
CREATE INDEX IF NOT EXISTS idx_traces_tags_gin ON traces USING gin (tags);
CREATE INDEX IF NOT EXISTS idx_logs_metadata_gin ON logs USING gin (metadata);
CREATE INDEX IF NOT EXISTS idx_otel_profiles_attrs_gin ON otel_profiles USING gin (attributes);

-- Attribute search helper: extract top-k attribute keys with counts
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
            FROM logs, jsonb_object_keys(metadata) AS k(key)
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
