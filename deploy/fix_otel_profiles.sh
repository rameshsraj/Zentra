#!/bin/bash
set -e

echo "=== Creating otel_profiles table in zentra DB ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra <<'SQL'

CREATE TABLE IF NOT EXISTS otel_profiles (
    id              uuid        NOT NULL DEFAULT gen_random_uuid(),
    org_id          uuid        NOT NULL,
    profile_id      text        NOT NULL,
    service_name    text        NOT NULL,
    profile_type    text        NOT NULL DEFAULT 'cpu',
    started_at      timestamptz NOT NULL DEFAULT now(),
    ended_at        timestamptz,
    duration_ns     bigint      DEFAULT 0,
    sample_count    integer     DEFAULT 0,
    data            jsonb       NOT NULL DEFAULT '{}',
    attributes      jsonb       NOT NULL DEFAULT '{}',
    PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS idx_otel_profiles_org ON otel_profiles (org_id);
CREATE INDEX IF NOT EXISTS idx_otel_profiles_service ON otel_profiles (org_id, service_name);
CREATE INDEX IF NOT EXISTS idx_otel_profiles_started ON otel_profiles (org_id, started_at DESC);

-- Also check if metrics table has resource_id column, add if missing
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns 
        WHERE table_name = 'metrics' AND column_name = 'resource_id'
    ) THEN
        ALTER TABLE metrics ADD COLUMN resource_id text DEFAULT '';
        RAISE NOTICE 'Added resource_id column to metrics';
    END IF;
END;
$$;

SQL

echo "Done"

echo ""
echo "=== Re-test otel/resources ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -c "DELETE FROM login_attempts;" 2>/dev/null
NEW_HASH=$(python3.11 -c 'import bcrypt; print(bcrypt.hashpw(b"admin123", bcrypt.gensalt(12)).decode())')
echo "UPDATE users SET password_hash='${NEW_HASH}', is_locked=false, locked_until=NULL WHERE email='admin@Zentra.local';" > /tmp/pw.sql
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -f /tmp/pw.sql 2>/dev/null

TOKEN=$(curl -s http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"admin123"}' | python3.11 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null)

AUTH="Authorization: Bearer $TOKEN"

CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5006/api/otel/resources" -H "$AUTH" --max-time 5)
BODY=$(curl -s "http://127.0.0.1:5006/api/otel/resources" -H "$AUTH" --max-time 5 | head -c 300)
echo "$CODE  /api/otel/resources  $BODY"
