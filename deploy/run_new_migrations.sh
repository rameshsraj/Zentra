#!/bin/bash
set -e
export PGPASSWORD='zantaz'
PGHOST="10.88.11.162"
PGPORT="5434"
PGUSER="postgres"
PGDB="postgres"
MDIR="/store/Zentra/migrations"

for f in \
  20250101000055_entity_dashboard_scope.sql \
  20250101000056_apdex_config.sql \
  20250101000057_target_hierarchy.sql \
  20250101000058_hubble_l7_flows.sql \
  20250101000059_llm_spans.sql \
  20250101000060_profiles.sql \
  20250101000062_semconv_columns.sql \
  20250101000063_derived_metrics.sql \
  20250101000064_otel_alert_rules.sql \
  20250101000065_otlp_api_keys.sql
do
  echo "=== Applying: $f ==="
  psql -h "$PGHOST" -p "$PGPORT" -U "$PGUSER" -d "$PGDB" -f "$MDIR/$f" 2>&1 || true
done

echo "ALL_MIGRATIONS_DONE"
