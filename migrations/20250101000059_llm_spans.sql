-- SkyWalking P08: LLM Dashboard from OTLP GenAI Spans
CREATE TABLE IF NOT EXISTS llm_spans (
    id            UUID NOT NULL DEFAULT gen_random_uuid(),
    org_id        UUID NOT NULL,
    trace_id      TEXT NOT NULL,
    span_id       TEXT NOT NULL,
    started_at    TIMESTAMPTZ NOT NULL,
    duration_ms   DOUBLE PRECISION NOT NULL,
    gen_ai_system TEXT,
    model         TEXT,
    operation     TEXT,
    input_tokens  INTEGER,
    output_tokens INTEGER,
    finish_reason TEXT,
    service_name  TEXT,
    is_error      BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (id, started_at)
);
SELECT create_hypertable('llm_spans', 'started_at', if_not_exists => TRUE);
CREATE INDEX IF NOT EXISTS idx_llm_spans_org_time ON llm_spans(org_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_llm_spans_model    ON llm_spans(model, started_at DESC);
DO $$ BEGIN
    PERFORM add_retention_policy('llm_spans', INTERVAL '90 days', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Retention policy skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;
