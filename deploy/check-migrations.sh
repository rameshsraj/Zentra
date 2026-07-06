#!/usr/bin/env bash
# Check which migration files use TimescaleDB commercial APIs without exception handlers
# Run: wsl -d Ubuntu -u root -- bash /mnt/c/GitHub/Zentra/deploy/check-migrations.sh
cd /store/Zentra/migrations 2>/dev/null || { echo "No migrations dir"; exit 1; }

echo "=== Migrations using add_compression_policy / add_retention_policy ==="
for f in $(grep -l 'add_compression_policy\|add_retention_policy' *.sql 2>/dev/null); do
    if grep -q 'EXCEPTION' "$f"; then
        echo "  OK (guarded): $f"
    else
        echo "  UNGUARDED:   $f  <-- WILL FAIL on apache license"
    fi
done

echo ""
echo "=== Running all pending migrations with verbose output ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra \
    -tAc "SELECT filename FROM zentra_schema_migrations ORDER BY filename" 2>/dev/null | head -30

echo ""
echo "=== Attempting all unapplied migrations (no ON_ERROR_STOP) ==="
for f in *.sql; do
    applied=$(PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra \
        -tAc "SELECT 1 FROM zentra_schema_migrations WHERE filename='$f'" 2>/dev/null | tr -d '[:space:]')
    if [[ "$applied" == "1" ]]; then
        echo "  SKIP (applied): $f"
        continue
    fi
    echo "  TEST: $f"
    result=$(PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra \
        -v ON_ERROR_STOP=1 -f "$f" 2>&1)
    rc=$?
    if [[ $rc -ne 0 ]]; then
        echo "  FAIL (rc=$rc): $f"
        echo "$result" | tail -5 | sed 's/^/    /'
    else
        echo "  PASS: $f"
    fi
done
