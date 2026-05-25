#!/bin/bash
set -e
export PGPASSWORD='zantaz'
PG="psql -h 10.88.11.162 -p 5434 -U postgres -d zentra"

echo "=== Step 1: Run all missing migrations on ZENTRA db ==="

# Run each migration file against the zentra database
MDIR="/store/Zentra/migrations"
for f in \
  20250101000055_entity_dashboard_scope.sql \
  20250101000056_apdex_config.sql \
  20250101000057_target_hierarchy.sql \
  20250101000058_hubble_l7_flows.sql \
  20250101000059_llm_spans.sql \
  20250101000060_profiles.sql \
  20250101000063_derived_metrics.sql \
  20250101000064_otel_alert_rules.sql \
  20250101000065_otlp_api_keys.sql
do
  echo "--- Applying: $f ---"
  $PG -f "$MDIR/$f" 2>&1 || true
done

echo ""
echo "=== Step 2: Create spans table ==="
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
echo "=== Step 3: Add semconv columns to spans ==="
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
echo "=== Step 4: Fix scrape_targets columns ==="
$PG <<'SQL'
ALTER TABLE scrape_targets ADD COLUMN IF NOT EXISTS last_scraped_at timestamptz;
ALTER TABLE scrape_targets ADD COLUMN IF NOT EXISTS labels jsonb DEFAULT '{}';
SQL

echo ""
echo "=== Step 5: Create hubble_l7_flows if missing ==="
$PG <<'SQL'
CREATE TABLE IF NOT EXISTS hubble_l7_flows (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        uuid NOT NULL,
    src_identity  text,
    dst_identity  text,
    src_namespace text,
    dst_namespace text,
    protocol      text,
    l7_type       text,
    http_method   text,
    http_url      text,
    http_code     int,
    dns_query     text,
    dns_rcode     text,
    kafka_topic   text,
    latency_ns    bigint,
    verdict       text,
    ts            timestamptz DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_hubble_l7_ts ON hubble_l7_flows (ts DESC);
SQL

echo ""
echo "=== Step 6: Verify all tables ==="
$PG -t -A -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename IN ('spans','llm_spans','otel_derived_metrics','derived_metrics','hubble_l7_flows','otlp_api_keys','apdex_config','profiles','otel_profiles','scrape_targets') ORDER BY tablename;"

echo ""
echo "=== Step 7: Verify scrape_targets has last_scraped_at ==="
$PG -t -A -c "SELECT column_name FROM information_schema.columns WHERE table_name='scrape_targets' AND column_name IN ('last_scraped_at','labels') ORDER BY column_name;"

echo ""
echo "ALL_FIXES_APPLIED"
