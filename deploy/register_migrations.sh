#!/usr/bin/env bash
# Apply the fixed middleware_instances migration, then register all unapplied
# migrations in _sqlx_migrations so sqlx won't try to re-run them.
set -euo pipefail

MIGDIR="/store/Zentra/migrations"
PG="psql -h localhost -p 5434 -U postgres -d zentra"

echo "=== Applying fixed middleware_instances migration ==="
$PG -c "
CREATE TABLE IF NOT EXISTS middleware_instances (
    id              TEXT PRIMARY KEY,
    instance_type   TEXT NOT NULL,
    name            TEXT NOT NULL,
    host            TEXT NOT NULL,
    port            INT  NOT NULL,
    status          TEXT NOT NULL DEFAULT 'unknown',
    extra           JSONB NOT NULL DEFAULT '{}',
    org_id          UUID REFERENCES organizations(org_id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_mw_instances_type ON middleware_instances(instance_type);
CREATE INDEX IF NOT EXISTS idx_mw_instances_org  ON middleware_instances(org_id);
"
echo "    done."

echo ""
echo "=== Registering all applied migrations in _sqlx_migrations ==="

cd "$MIGDIR"
for f in $(ls *.sql | sort); do
  ver=$(echo "$f" | cut -d_ -f1)
  desc=$(echo "$f" | sed "s/^${ver}_//;s/\.sql$//;s/_/ /g")

  # Check if already registered
  exists=$($PG -tAc "SELECT 1 FROM _sqlx_migrations WHERE version = $ver" 2>/dev/null || echo "0")
  if [ "$exists" = "1" ]; then
    echo "  SKIP (already registered): $f"
    continue
  fi

  # Compute checksum: sqlx uses sha384 of the file content
  checksum=$(sha384sum "$f" | awk '{print $1}')

  $PG -c "
    INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
    VALUES ($ver, '$desc', NOW(), true, decode('$checksum', 'hex'), 0)
    ON CONFLICT (version) DO NOTHING;
  " 2>/dev/null && echo "  registered: $f" || echo "  WARN: failed to register $f"
done

echo ""
echo "=== Final migration count ==="
$PG -c "SELECT COUNT(*) as total_migrations FROM _sqlx_migrations;"
echo "Done."
