#!/bin/bash
export PGPASSWORD='zantaz'
PG="psql -h 10.88.11.162 -p 5434 -U postgres"

echo "=== Check which databases exist ==="
$PG -d postgres -t -A -c "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname;"

echo ""
echo "=== Check spans in postgres DB ==="
$PG -d postgres -t -A -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename='spans';"

echo ""
echo "=== Check spans in zentra DB (if exists) ==="
$PG -d zentra -t -A -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename='spans';" 2>&1 || echo "(zentra db doesn't exist)"

echo ""
echo "=== Check .env for DATABASE_URL ==="
grep -i database /store/Zentra/.env 2>/dev/null || echo "No DATABASE_URL found"
grep -i tsdb /store/Zentra/.env 2>/dev/null || echo "No TSDB_URL found"

echo ""
echo "=== Check what zentra-server is connecting to ==="
cat /proc/$(pgrep -f zentra-server | head -1)/environ 2>/dev/null | tr '\0' '\n' | grep -i database || echo "Could not read env"
cat /proc/$(pgrep -f zentra-server | head -1)/environ 2>/dev/null | tr '\0' '\n' | grep -i tsdb || echo "No TSDB env var"
