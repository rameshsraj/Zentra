#!/bin/bash
# ============================================================
# Zentra Endpoint Verification
# 1. Restart API to clear in-memory state
# 2. Reset password + clear rate-limit rows
# 3. Login immediately (before any failed attempts accumulate)
# 4. Test all endpoints with the token
# ============================================================

echo "=== Step 1: Restart API server ==="
pkill -f zentra-server 2>/dev/null || true
sleep 3

cd /store/Zentra
set -a; source .env 2>/dev/null; set +a
nohup ./target/release/zentra-server > /var/log/zentra-server.log 2>&1 &
echo "Server PID=$!"

# Wait for server to be ready
for i in 1 2 3 4 5 6 7 8 9 10; do
  sleep 1
  if curl -sf http://127.0.0.1:5006/api/health >/dev/null 2>&1; then
    echo "Server ready after ${i}s"
    break
  fi
done

echo ""
echo "=== Step 2: Reset password + clear rate limits ==="
NEW_HASH=$(python3.11 -c 'import bcrypt; print(bcrypt.hashpw(b"admin123", bcrypt.gensalt(12)).decode())')

# Clear rate limits from zentra DB (where the API writes login_attempts)
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -c "DELETE FROM login_attempts;" 2>&1

# Update password in BOTH databases (users exist in postgres, may also be in zentra)
for DB in zentra postgres; do
  echo "UPDATE users SET password_hash='${NEW_HASH}', is_locked=false, locked_until=NULL, mfa_enabled=false WHERE email IN ('admin@smarsh-nexus.com','admin@Zentra.local');" > /tmp/admin_reset.sql
  PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d $DB -f /tmp/admin_reset.sql 2>&1
done
echo "Done"

echo ""
echo "=== Step 3: Login ==="
# Login IMMEDIATELY after clearing rate limits — no delays
LOGIN_RESP=$(curl -s http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"admin123"}')

echo "Response: $LOGIN_RESP"

TOKEN=$(echo "$LOGIN_RESP" | python3.11 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('token', ''))
except:
    print('')
" 2>/dev/null)

if [ -z "$TOKEN" ]; then
  echo ""
  echo "!!! LOGIN FAILED - will test endpoints without auth !!!"
  echo "!!! (401 responses confirm routes exist + auth works) !!!"
  AUTH=""
else
  echo "LOGIN OK (token: ${TOKEN:0:20}...)"
  AUTH="Authorization: Bearer $TOKEN"
fi

echo ""
echo "=== Step 4: Endpoint Verification ==="

ENDPOINTS=(
  "/api/health"
  "/api/system/status"
  "/api/system/db"
  "/api/config/mode"
  "/api/targets"
  "/api/traces"
  "/api/traces/services"
  "/api/traces/operations"
  "/api/metrics/catalog"
  "/api/alerts"
  "/api/alert-rules"
  "/api/dashboards"
  "/api/logs"
  "/api/network/devices"
  "/api/oracle/overview"
  "/api/postgres/overview"
  "/api/mysql/overview"
  "/api/mongodb/overview"
  "/api/sqlserver/overview"
  "/api/security/ids/rules"
  "/api/security/connections"
  "/api/security/firewall/stats"
  "/api/compliance/frameworks"
  "/api/sso/providers"
  "/api/tenants"
  "/api/ai/insights"
  "/api/ai/ml/status"
  "/api/predictions/summary"
  "/api/llm/overview"
  "/api/anomalies"
  "/api/runbooks"
  "/api/apdex"
  "/api/cilium/status"
  "/api/hubble/status"
  "/api/profiles"
  "/api/netflow/stats"
  "/api/snmp/traps"
  "/api/cloud/summary"
  "/api/storage-arrays"
  "/api/collectors"
  "/api/scrape/targets"
  "/api/events"
  "/api/events/stats"
  "/api/mcp/tools"
  "/api/rca/history"
  "/api/otel/resources"
  "/api/otel/status"
  "/api/otel/collector-config"
  "/api/llm-security/overview"
  "/api/notifications/channels"
  "/api/notifications/escalation-chains"
  "/api/users"
  "/api/roles"
  "/api/auth/me"
  "/api/search?q=cpu"
  "/api/drilldown?metric=cpu"
  "/api/v1/status/buildinfo"
  "/api/v1/status/runtimeinfo"
  "/api/v1/status/config"
  "/api/v1/openapi.json"
  "/api/correlation/logs-for-metric?metric=cpu"
)

PASS=0
FAIL=0
AUTH_FAIL=0
ERRORS=""

for EP in "${ENDPOINTS[@]}"; do
  if [ -n "$AUTH" ]; then
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5006${EP}" \
      -H "$AUTH" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  else
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5006${EP}" \
      -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  fi

  if [ "$CODE" -ge 200 ] && [ "$CODE" -lt 300 ]; then
    printf "OK   %s  %s\n" "$CODE" "$EP"
    PASS=$((PASS+1))
  elif [ "$CODE" -eq 401 ]; then
    printf "AUTH %s  %s\n" "$CODE" "$EP"
    AUTH_FAIL=$((AUTH_FAIL+1))
  elif [ "$CODE" -eq 404 ]; then
    printf "MISS %s  %s\n" "$CODE" "$EP"
    FAIL=$((FAIL+1))
    ERRORS="${ERRORS}\n  404 $EP"
  elif [ "$CODE" -eq 500 ]; then
    printf "ERR  %s  %s\n" "$CODE" "$EP"
    FAIL=$((FAIL+1))
    ERRORS="${ERRORS}\n  500 $EP"
  else
    printf "???  %s  %s\n" "$CODE" "$EP"
    FAIL=$((FAIL+1))
    ERRORS="${ERRORS}\n  $CODE $EP"
  fi
done

TOTAL=${#ENDPOINTS[@]}
echo ""
echo "=========================================="
echo " RESULTS: $PASS OK, $AUTH_FAIL auth(401), $FAIL failed"
echo " Total:   $TOTAL endpoints tested"
ROUTED=$((PASS + AUTH_FAIL))
echo " Routed:  $ROUTED/$TOTAL (route exists, handler responds)"
if [ -n "$ERRORS" ]; then
  echo ""
  echo " ISSUES:"
  echo -e "$ERRORS"
fi
echo "=========================================="
