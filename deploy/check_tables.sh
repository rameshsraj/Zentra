#!/bin/bash
export PGPASSWORD='zantaz'
PG="psql -h 10.88.11.162 -p 5434 -U postgres -d postgres -t -A"

echo "=== Checking tables ==="
$PG -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename IN ('spans','llm_spans','otel_derived_metrics','derived_metrics','scrape_targets','apdex_config','profiles','otel_profiles','hubble_l7_flows','otlp_api_keys') ORDER BY tablename;"

echo ""
echo "=== Checking scrape_targets columns ==="
$PG -c "SELECT column_name FROM information_schema.columns WHERE table_name='scrape_targets' ORDER BY ordinal_position;"

echo ""
echo "=== Checking llm_spans ==="
$PG -c "SELECT column_name FROM information_schema.columns WHERE table_name='llm_spans' ORDER BY ordinal_position;" 2>&1 || echo "llm_spans does not exist"

echo ""
echo "=== Checking derived_metrics ==="
$PG -c "SELECT column_name FROM information_schema.columns WHERE table_name='derived_metrics' ORDER BY ordinal_position;" 2>&1 || echo "derived_metrics does not exist"
