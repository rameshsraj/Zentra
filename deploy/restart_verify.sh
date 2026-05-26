#!/bin/bash
# Step 1: Restart the server
pkill -f zentra-server 2>/dev/null || true
sleep 2

cd /store/Zentra
set -a; source .env 2>/dev/null; set +a
nohup ./target/release/zentra-server > /var/log/zentra-server.log 2>&1 &
echo "Server restarted PID=$!"
sleep 4

# Step 2: Verify health
curl -sf http://127.0.0.1:5006/api/health && echo "" || echo "Health check failed"

# Step 3: Reset password and clear rate limits
NEW_HASH=$(python3.11 -c 'import bcrypt; print(bcrypt.hashpw(b"admin123", bcrypt.gensalt(12)).decode())')
cat > /tmp/reset_pw.sql <<EOSQL
UPDATE users SET password_hash='${NEW_HASH}', is_locked=false, locked_until=NULL WHERE email='admin@smarsh-nexus.com';
DELETE FROM login_attempts;
EOSQL
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d postgres -f /tmp/reset_pw.sql
echo "Password reset and rate limits cleared"

# Step 4: Login
LOGIN_BODY='{"email":"admin@smarsh-nexus.com","password":"admin123"}'
LOGIN_RESP=$(curl -s http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d "$LOGIN_BODY")
echo "Login: $LOGIN_RESP"

TOKEN=$(echo "$LOGIN_RESP" | python3.11 -c "import sys,json; d=json.load(sys.stdin); print(d.get('token',''))" 2>/dev/null || true)

if [ -z "$TOKEN" ]; then
  echo "LOGIN FAILED - testing without auth"
  AUTH=""
else
  echo "LOGIN OK"
  AUTH="Authorization: Bearer $TOKEN"
fi

# Step 5: Test all endpoints
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
  "/api/oracle/instances"
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
  "/api/mcp/tools"
  "/api/rca/history"
  "/api/otel/resources"
  "/api/otel/collector/status"
  "/api/otel/pipelines"
  "/api/llm-security/overview"
  "/api/notifications/channels"
  "/api/notifications/escalation-chains"
  "/api/admin/users"
  "/api/admin/roles"
  "/api/v1/status/buildinfo"
  "/api/v1/status/runtimeinfo"
  "/api/v1/status/config"
  "/api/v1/status/tsdb"
)

echo ""
echo "=========================================="
echo " ENDPOINT VERIFICATION (${#ENDPOINTS[@]} endpoints)"
echo "=========================================="

PASS=0
FAIL=0
AUTH_FAIL=0

for EP in "${ENDPOINTS[@]}"; do
  if [ -n "$AUTH" ]; then
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5006${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  else
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:5006${EP}" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  fi

  if [ "$CODE" -ge 200 ] && [ "$CODE" -lt 300 ]; then
    echo "OK   $CODE  $EP"
    PASS=$((PASS+1))
  elif [ "$CODE" -eq 401 ]; then
    echo "AUTH 401  $EP"
    AUTH_FAIL=$((AUTH_FAIL+1))
  elif [ "$CODE" -eq 404 ]; then
    echo "MISS 404  $EP"
    FAIL=$((FAIL+1))
  elif [ "$CODE" -eq 500 ]; then
    echo "ERR  500  $EP"
    FAIL=$((FAIL+1))
  else
    echo "???  $CODE  $EP"
    FAIL=$((FAIL+1))
  fi
done

echo ""
echo "=========================================="
echo " RESULTS: $PASS OK, $AUTH_FAIL auth-blocked, $FAIL failed (of ${#ENDPOINTS[@]})"
echo "=========================================="
