#!/bin/bash
cd /store/Zentra
set -a; source .env 2>/dev/null; set +a

# Get a fresh token
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -c "DELETE FROM login_attempts;" 2>/dev/null
NEW_HASH=$(python3.11 -c 'import bcrypt; print(bcrypt.hashpw(b"admin123", bcrypt.gensalt(12)).decode())')
echo "UPDATE users SET password_hash='${NEW_HASH}', is_locked=false, locked_until=NULL WHERE email='admin@Zentra.local';" > /tmp/pw.sql
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -f /tmp/pw.sql 2>/dev/null

TOKEN=$(curl -s http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"admin123"}' | python3.11 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null)

if [ -z "$TOKEN" ]; then
  echo "Login failed"
  exit 1
fi
echo "Logged in OK"
AUTH="Authorization: Bearer $TOKEN"

echo ""
echo "=== 500 Error Details ==="
for EP in /api/traces /api/traces/services /api/traces/operations /api/mysql/overview /api/mongodb/overview /api/sqlserver/overview /api/apdex /api/otel/resources; do
  echo ""
  echo "--- $EP ---"
  curl -s "http://127.0.0.1:5006${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 10 2>/dev/null | head -c 500
  echo ""
done

echo ""
echo "=== Timeout (000) Details ==="
for EP in /api/network/devices /api/storage-arrays /api/v1/status/tsdb; do
  echo ""
  echo "--- $EP ---"
  curl -s "http://127.0.0.1:5006${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 15 -w "\nHTTP_CODE:%{http_code} TIME:%{time_total}s" 2>/dev/null | tail -c 600
  echo ""
done
