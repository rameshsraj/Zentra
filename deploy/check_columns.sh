#!/bin/bash
export PGPASSWORD='zantaz'
PG="psql -h 10.88.11.162 -p 5434 -U postgres -d zentra -t -A"

echo "=== metrics table columns ==="
$PG -c "SELECT column_name, data_type FROM information_schema.columns WHERE table_name='metrics' ORDER BY ordinal_position;"

echo ""
echo "=== scrape_targets columns ==="
$PG -c "SELECT column_name, data_type FROM information_schema.columns WHERE table_name='scrape_targets' ORDER BY ordinal_position;"

echo ""
echo "=== cilium handler query check ==="
$PG -c "SELECT column_name FROM information_schema.columns WHERE table_name='scrape_targets' AND column_name IN ('labels','last_scraped_at','job_name','host','port','component');"
