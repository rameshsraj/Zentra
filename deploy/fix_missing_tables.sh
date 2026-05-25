#!/bin/bash
export PGPASSWORD='zantaz'
PG="psql -h 10.88.11.162 -p 5434 -U postgres -d postgres"

echo "=== Creating spans table ==="
$PG <<'SQL'
CREATE TABLE IF NOT EXISTS spans (
    trace_id       text NOT NULL,
    span_id        text NOT NULL,
    parent_span_id text,
    operation_name text NOT NULL,
    service_name   text NOT NULL,
    started_at     timestamptz NOT NULL,
    duration_us    bigint NOT NULL DEFAULT 0,
    status         text NOT NULL DEFAULT 'ok',
    tags           jsonb DEFAULT '{}',
    PRIMARY KEY (trace_id, span_id, started_at)
);
SELECT create_hypertable('spans', 'started_at', if_not_exists => TRUE, migrate_data => TRUE);
CREATE INDEX IF NOT EXISTS idx_spans_service ON spans (service_name, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_spans_trace ON spans (trace_id);
CREATE INDEX IF NOT EXISTS idx_spans_tags ON spans USING gin (tags);
SQL

echo ""
echo "=== Adding last_scraped_at to scrape_targets ==="
$PG -c "ALTER TABLE scrape_targets ADD COLUMN IF NOT EXISTS last_scraped_at timestamptz;"

echo ""
echo "=== Adding semconv columns to spans ==="
$PG <<'SQL'
ALTER TABLE spans ADD COLUMN IF NOT EXISTS http_method text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS http_status_code int;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS http_route text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS db_system text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS db_statement text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS rpc_system text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS rpc_method text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS messaging_system text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS net_peer_name text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS net_peer_port int;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS otel_library_name text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS span_kind text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS deployment_environment text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS k8s_namespace text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS k8s_pod_name text;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS container_id text;
SQL

echo ""
echo "=== Verifying ==="
$PG -t -A -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename IN ('spans','llm_spans','otel_derived_metrics') ORDER BY tablename;"
$PG -t -A -c "SELECT column_name FROM information_schema.columns WHERE table_name='scrape_targets' AND column_name='last_scraped_at';"

echo ""
echo "ALL_FIXES_DONE"
