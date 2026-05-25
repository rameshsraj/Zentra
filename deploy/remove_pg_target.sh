#!/bin/bash
export PGPASSWORD='zantaz'
PG="psql -h 10.88.11.162 -p 5434 -U postgres -d zentra"

echo "=== Before: database targets ==="
$PG -t -A -c "SELECT target_id, name, target_subtype FROM targets WHERE target_type='database';"

echo ""
echo "=== Removing non-Oracle database targets ==="
$PG -c "DELETE FROM targets WHERE target_type='database' AND target_subtype != 'oracle';"

echo ""
echo "=== After: database targets ==="
$PG -t -A -c "SELECT target_id, name, target_subtype FROM targets WHERE target_type='database';"

echo ""
echo "DONE"
