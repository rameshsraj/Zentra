-- OT05: Migration hygiene — canonical traces base schema.
-- Ensures a fresh database can bootstrap the traces table with all required columns.
-- Idempotent — safe to re-run on existing databases.

CREATE TABLE IF NOT EXISTS traces (
    trace_id        TEXT NOT NULL,
    span_id         TEXT NOT NULL,
    parent_span_id  TEXT,
    operation_name  TEXT NOT NULL DEFAULT 'unknown',
    service_name    TEXT NOT NULL DEFAULT 'unknown',
    started_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_us     BIGINT NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'ok',
    tags            JSONB NOT NULL DEFAULT '{}',
    org_id          UUID,
    PRIMARY KEY (span_id, started_at)
);

-- Generated columns for semconv-based filtering (OT05/OT09)
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='traces' AND column_name='http_method') THEN
        ALTER TABLE traces ADD COLUMN http_method TEXT GENERATED ALWAYS AS (tags->>'http.method') STORED;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='traces' AND column_name='db_system') THEN
        ALTER TABLE traces ADD COLUMN db_system TEXT GENERATED ALWAYS AS (tags->>'db.system') STORED;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='traces' AND column_name='gen_ai_system') THEN
        ALTER TABLE traces ADD COLUMN gen_ai_system TEXT GENERATED ALWAYS AS (tags->>'gen_ai.system') STORED;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='traces' AND column_name='duration_ms') THEN
        ALTER TABLE traces ADD COLUMN duration_ms DOUBLE PRECISION GENERATED ALWAYS AS (duration_us::double precision / 1000.0) STORED;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='traces' AND column_name='attributes') THEN
        ALTER TABLE traces ADD COLUMN attributes JSONB NOT NULL DEFAULT '{}';
    END IF;
END $$;

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_traces_org_started ON traces (org_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_service ON traces (service_name, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_trace_id ON traces (trace_id);
CREATE INDEX IF NOT EXISTS idx_traces_tags_gin ON traces USING gin (tags);

-- TimescaleDB hypertable (safe if extension not available)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
        PERFORM create_hypertable('traces', 'started_at', if_not_exists => TRUE, migrate_data => TRUE);
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'TimescaleDB hypertable creation skipped: %', SQLERRM;
END $$;

-- Backward-compatible "spans" view
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_name = 'spans' AND table_type = 'BASE TABLE'
    ) THEN
        EXECUTE 'CREATE OR REPLACE VIEW spans AS SELECT * FROM traces';
    END IF;
END $$;

-- OT11: Span events and span links tables
CREATE TABLE IF NOT EXISTS span_events (
    id              UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    trace_id        TEXT NOT NULL,
    span_id         TEXT NOT NULL,
    event_name      TEXT NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    attributes      JSONB NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS span_links (
    id              UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    trace_id        TEXT NOT NULL,
    span_id         TEXT NOT NULL,
    linked_trace_id TEXT NOT NULL,
    linked_span_id  TEXT NOT NULL,
    attributes      JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_span_events_span ON span_events (trace_id, span_id);
CREATE INDEX IF NOT EXISTS idx_span_links_span ON span_links (trace_id, span_id);

-- otlp_ingest_audit table (OT04) — ensure it exists for fresh DBs
CREATE TABLE IF NOT EXISTS otlp_ingest_audit (
    id              UUID DEFAULT gen_random_uuid() PRIMARY KEY,
    org_id          UUID NOT NULL,
    signal          TEXT NOT NULL,
    endpoint        TEXT NOT NULL,
    status_code     INT NOT NULL DEFAULT 200,
    span_count      INT,
    payload_bytes   INT,
    remote_addr     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_otlp_audit_org ON otlp_ingest_audit (org_id, created_at DESC);

-- Materialized view for RED metrics (OT06)
CREATE MATERIALIZED VIEW IF NOT EXISTS span_red_metrics_1m AS
SELECT
    service_name,
    time_bucket('1 minute', started_at) AS bucket,
    COUNT(*) AS request_count,
    COUNT(*) FILTER (WHERE status = 'error') AS error_count,
    AVG(duration_us::double precision / 1000.0) AS avg_ms,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY duration_us::double precision / 1000.0) AS p50_ms,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY duration_us::double precision / 1000.0) AS p95_ms,
    PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY duration_us::double precision / 1000.0) AS p99_ms
FROM traces
WHERE started_at > NOW() - INTERVAL '24 hours'
GROUP BY service_name, bucket;

CREATE UNIQUE INDEX IF NOT EXISTS span_red_metrics_1m_uniq
    ON span_red_metrics_1m (service_name, bucket)
    WHERE service_name IS NOT NULL;
