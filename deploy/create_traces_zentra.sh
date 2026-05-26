#!/bin/bash
set -e

echo "=== Creating traces table in zentra DB ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra <<'SQL'

-- Traces table matching what zentra-api handlers expect
CREATE TABLE IF NOT EXISTS traces (
    id              uuid        NOT NULL DEFAULT gen_random_uuid(),
    org_id          uuid        NOT NULL,
    trace_id        text        NOT NULL,
    span_id         text        NOT NULL,
    parent_span_id  text,
    service_name    text        NOT NULL,
    operation_name  text        NOT NULL,
    started_at      timestamptz NOT NULL,
    ended_at        timestamptz NOT NULL,
    duration_us     bigint      DEFAULT 0,
    duration_ms     double precision GENERATED ALWAYS AS (duration_us / 1000.0) STORED,
    status          text        NOT NULL DEFAULT 'ok',
    status_message  text,
    kind            text,
    tags            jsonb       NOT NULL DEFAULT '{}',
    events          jsonb       NOT NULL DEFAULT '[]',
    links           jsonb       NOT NULL DEFAULT '[]',
    attributes      jsonb       NOT NULL DEFAULT '{}',
    resource_attributes jsonb   NOT NULL DEFAULT '{}',
    http_method     text,
    http_status_code integer,
    db_system       text,
    rpc_system      text,
    messaging_system text,
    gen_ai_system   text,
    PRIMARY KEY (id)
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_traces_org_id ON traces (org_id);
CREATE INDEX IF NOT EXISTS idx_traces_trace_id ON traces (trace_id);
CREATE INDEX IF NOT EXISTS idx_traces_service ON traces (org_id, service_name);
CREATE INDEX IF NOT EXISTS idx_traces_started ON traces (org_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_traces_status ON traces (org_id, status);
CREATE INDEX IF NOT EXISTS idx_traces_duration ON traces (org_id, duration_us);
CREATE INDEX IF NOT EXISTS idx_traces_parent ON traces (parent_span_id);
CREATE INDEX IF NOT EXISTS idx_traces_tags ON traces USING gin (tags);

-- Convert to TimescaleDB hypertable if extension is available
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
        PERFORM create_hypertable('traces', 'started_at', if_not_exists => TRUE, migrate_data => TRUE);
        RAISE NOTICE 'traces converted to hypertable';
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'TimescaleDB hypertable creation skipped: %', SQLERRM;
END;
$$;

SQL

echo "traces table created"

echo ""
echo "=== Verify ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -c "\d traces" 2>&1

echo ""
echo "=== Re-test traces endpoints ==="
# Get token
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -c "DELETE FROM login_attempts;" 2>/dev/null
NEW_HASH=$(python3.11 -c 'import bcrypt; print(bcrypt.hashpw(b"admin123", bcrypt.gensalt(12)).decode())')
echo "UPDATE users SET password_hash='${NEW_HASH}', is_locked=false, locked_until=NULL WHERE email='admin@Zentra.local';" > /tmp/pw.sql
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -f /tmp/pw.sql 2>/dev/null

TOKEN=$(curl -s http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"admin123"}' | python3.11 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null)

AUTH="Authorization: Bearer $TOKEN"

for EP in /api/traces /api/traces/services /api/traces/operations /api/apdex /api/otel/resources; do
  CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5006${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  BODY=$(curl -s "http://127.0.0.1:5006${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 5 2>/dev/null | head -c 200)
  echo "$CODE  $EP  $BODY"
done
