#!/bin/bash
# Cleanup all Zentra objects accidentally created in the 'postgres' database.
# These should only exist in the 'Zentra' database.

set -e
export PGPASSWORD="${PGPASSWORD:?Error: PGPASSWORD not set}"
DB_HOST="${POSTGRES_HOST:-10.88.11.162}"
DB_PORT="${POSTGRES_PORT:-5434}"
DB_USER="${POSTGRES_USER:-postgres}"
DB_NAME=postgres

run_sql() {
    echo "  $1"
    psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "$1" 2>&1 | grep -v "^$" | head -3
}

echo "============================================"
echo " Cleaning Zentra objects from 'postgres' DB"
echo "============================================"
echo ""

echo "--- Dropping Views ---"
run_sql "DROP VIEW IF EXISTS dashboard_summary CASCADE;"
run_sql "DROP VIEW IF EXISTS recent_dashboards CASCADE;"
run_sql "DROP VIEW IF EXISTS scrape_target_status CASCADE;"
run_sql "DROP VIEW IF EXISTS scrape_health_summary CASCADE;"
run_sql "DROP VIEW IF EXISTS active_anomalies CASCADE;"
run_sql "DROP VIEW IF EXISTS active_forecasts CASCADE;"
run_sql "DROP VIEW IF EXISTS prediction_summary CASCADE;"
run_sql "DROP VIEW IF EXISTS license_compliance_summary CASCADE;"
run_sql "DROP VIEW IF EXISTS licenses_expiring_soon CASCADE;"

echo ""
echo "--- Dropping Materialized Views ---"
run_sql "DROP MATERIALIZED VIEW IF EXISTS metrics_1m CASCADE;"
run_sql "DROP MATERIALIZED VIEW IF EXISTS metrics_1h CASCADE;"
run_sql "DROP MATERIALIZED VIEW IF EXISTS log_severity_1h CASCADE;"

echo ""
echo "--- Dropping Tables (007 dashboards) ---"
run_sql "DROP TABLE IF EXISTS dashboard_exports CASCADE;"
run_sql "DROP TABLE IF EXISTS dashboard_panels CASCADE;"
run_sql "DROP TABLE IF EXISTS dashboard_variables CASCADE;"
run_sql "DROP TABLE IF EXISTS dashboards CASCADE;"

echo ""
echo "--- Dropping Tables (006 annotations) ---"
run_sql "DROP TABLE IF EXISTS annotations CASCADE;"
run_sql "DROP TABLE IF EXISTS dashboard_configs CASCADE;"

echo ""
echo "--- Dropping Tables (005 scrape) ---"
run_sql "DROP TABLE IF EXISTS scrape_recording_rules CASCADE;"
run_sql "DROP TABLE IF EXISTS scrape_results CASCADE;"
run_sql "DROP TABLE IF EXISTS scrape_targets CASCADE;"

echo ""
echo "--- Dropping Tables (004 predictions) ---"
run_sql "DROP TABLE IF EXISTS recording_rules CASCADE;"
run_sql "DROP TABLE IF EXISTS trend_alerts CASCADE;"
run_sql "DROP TABLE IF EXISTS anomaly_events CASCADE;"
run_sql "DROP TABLE IF EXISTS predictions CASCADE;"
run_sql "DROP TABLE IF EXISTS metric_baselines CASCADE;"

echo ""
echo "--- Dropping Tables (003 licenses) ---"
run_sql "DROP TABLE IF EXISTS license_audit_log CASCADE;"
run_sql "DROP TABLE IF EXISTS license_assignments CASCADE;"
run_sql "DROP TABLE IF EXISTS license_seats CASCADE;"
run_sql "DROP TABLE IF EXISTS licenses CASCADE;"

echo ""
echo "--- Dropping Tables (001 schema) ---"
run_sql "DROP TABLE IF EXISTS log_entries CASCADE;"
run_sql "DROP TABLE IF EXISTS metrics CASCADE;"
run_sql "DROP TABLE IF EXISTS alert_history CASCADE;"
run_sql "DROP TABLE IF EXISTS alert_rules CASCADE;"
run_sql "DROP TABLE IF EXISTS targets CASCADE;"

echo ""
echo "--- Dropping Extensions ---"
run_sql "DROP EXTENSION IF EXISTS pg_trgm CASCADE;"
echo "(NOT dropping timescaledb from postgres DB — may be system-wide)"

echo ""
echo "============================================"
echo " Cleanup complete!"
echo "============================================"

echo ""
echo "--- Verifying: remaining Zentra tables in postgres DB ---"
psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename;" 2>&1
