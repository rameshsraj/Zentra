-- OT02: Dead letter queue for failed OTLP ingest batches.
-- Stores the original payload bytes and error for forensics.

CREATE TABLE IF NOT EXISTS otlp_dead_letter (
    id              UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    org_id          UUID NOT NULL,
    signal          TEXT NOT NULL,  -- traces, metrics, logs, profiles
    payload         BYTEA NOT NULL,
    error_message   TEXT NOT NULL,
    retry_count     INT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_otlp_dlq_org_created
    ON otlp_dead_letter (org_id, created_at DESC);

-- Retention: auto-drop rows older than 30 days
CREATE OR REPLACE FUNCTION cleanup_otlp_dlq() RETURNS void AS $$
BEGIN
    DELETE FROM otlp_dead_letter WHERE created_at < NOW() - INTERVAL '30 days';
END;
$$ LANGUAGE plpgsql;
