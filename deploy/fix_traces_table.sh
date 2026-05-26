#!/bin/bash
set -e

# Check what tables exist in zentra vs postgres
echo "=== Tables in zentra DB ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -t -c "SELECT tablename FROM pg_tables WHERE schemaname='public' ORDER BY tablename;" 2>&1

echo ""
echo "=== Check if traces exists in postgres DB ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d postgres -t -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename='traces';" 2>&1

echo ""
echo "=== Run migrations against zentra DB ==="
cd /store/Zentra/deploy/sql
for f in 001_schema.sql 002_indexes.sql; do
  echo "Applying $f to zentra..."
  PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -f "$f" 2>&1 | tail -3
done

echo ""
echo "=== Verify traces table now exists ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -t -c "SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename='traces';" 2>&1
