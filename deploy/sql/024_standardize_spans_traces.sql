-- P07 (DB-SCHEMA): Standardize spans vs traces table naming.
--
-- Canonical schema:
--   traces  — one row per span (trace_id, span_id, parent, service, operation, duration, status, tags)
--   spans   — ALIAS view pointing to traces for backward-compatible queries
--   span_red_metrics_1m — materialized view aggregating from traces
--   active_alerts — table for background worker firing alerts (P02)
--
-- This migration is idempotent — safe to re-run.

-- 1. Ensure traces table exists with canonical columns
--    (created by 001_schema.sql / OTLP handlers; this adds any missing columns)
DO $$
BEGIN
    -- Add org_id if missing (some early migrations omitted it)
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'traces' AND column_name = 'org_id'
    ) THEN
        ALTER TABLE traces ADD COLUMN org_id UUID;
    END IF;

    -- Add http_method for span-metrics grouping
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'traces' AND column_name = 'http_method'
    ) THEN
        ALTER TABLE traces ADD COLUMN http_method TEXT
            GENERATED ALWAYS AS (tags->>'http.method') STORED;
    END IF;

    -- Add db_system for span-metrics grouping
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'traces' AND column_name = 'db_system'
    ) THEN
        ALTER TABLE traces ADD COLUMN db_system TEXT
            GENERATED ALWAYS AS (tags->>'db.system') STORED;
    END IF;

    -- Add gen_ai_system for GenAI trace detection
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'traces' AND column_name = 'gen_ai_system'
    ) THEN
        ALTER TABLE traces ADD COLUMN gen_ai_system TEXT
            GENERATED ALWAYS AS (tags->>'gen_ai.system') STORED;
    END IF;

    -- Add duration_ms (derived from duration_us) for RED metric queries
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'traces' AND column_name = 'duration_ms'
    ) THEN
        ALTER TABLE traces ADD COLUMN duration_ms DOUBLE PRECISION
            GENERATED ALWAYS AS (duration_us::double precision / 1000.0) STORED;
    END IF;
END $$;

-- 2. Create a backward-compatible "spans" view if the spans table doesn't exist
--    as a real table. If it does exist as a table, skip (don't drop user data).
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'spans' AND table_type = 'BASE TABLE'
    ) THEN
        EXECUTE 'CREATE OR REPLACE VIEW spans AS SELECT * FROM traces';
    END IF;
END $$;

-- 3. Ensure unique index for REFRESH MATERIALIZED VIEW CONCURRENTLY
CREATE UNIQUE INDEX IF NOT EXISTS span_red_metrics_1m_uniq
    ON span_red_metrics_1m (service_name, bucket)
    WHERE service_name IS NOT NULL;

-- 4. Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_traces_org_started ON traces (org_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_service ON traces (service_name, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_trace_id ON traces (trace_id);

-- 5. active_alerts table for background worker (P02 BKG-JOBS)
CREATE TABLE IF NOT EXISTS active_alerts (
    id              UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    org_id          UUID NOT NULL,
    rule_name       TEXT NOT NULL,
    metric          TEXT NOT NULL,
    current_value   DOUBLE PRECISION NOT NULL DEFAULT 0,
    threshold       DOUBLE PRECISION NOT NULL DEFAULT 0,
    severity        TEXT NOT NULL DEFAULT 'warning',
    fired_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at     TIMESTAMPTZ,
    UNIQUE (org_id, rule_name)
);

CREATE INDEX IF NOT EXISTS idx_active_alerts_org ON active_alerts (org_id, fired_at DESC);
