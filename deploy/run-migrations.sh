#!/bin/bash
set -e
export PGPASSWORD="${PGPASSWORD:?Error: PGPASSWORD not set}"
DB_HOST="${POSTGRES_HOST:-10.88.11.162}"
DB_PORT="${POSTGRES_PORT:-5434}"
DB_USER="${POSTGRES_USER:-postgres}"
DB_NAME="zentra"
SQL_DIR=/store/Zentra/deploy/sql

for f in 001_schema.sql 002_indexes.sql 003_license_management.sql 004_predictions.sql 005_scrape_targets.sql 006_annotations.sql 007_dashboards.sql; do
    echo "Applying: $f"
    psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -f "$SQL_DIR/$f" 2>&1 | tail -3
done

echo "ALL_MIGRATIONS_APPLIED"
