#!/bin/bash
set -e

# Reset admin password to "admin123" (bcrypt hash)
HASH='$2b$12$h5TOGtazR2aK6Vt4xxw4YecKU4LR0nJ1a3z7uM9LCSx6EcXqpjJcS'
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d postgres -c "UPDATE users SET password_hash='${HASH}', is_locked=false, locked_until=NULL WHERE email='admin@smarsh-nexus.com';"
echo "Password reset done"

# Login and get token
LOGIN_BODY='{"email":"admin@smarsh-nexus.com","password":"admin123"}'
LOGIN_RESP=$(curl -s http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d "$LOGIN_BODY")
echo "Login response: $LOGIN_RESP"

TOKEN=$(echo "$LOGIN_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('token',''))" 2>/dev/null)

if [ -z "$TOKEN" ]; then
  echo "LOGIN FAILED — could not extract token"
  exit 1
fi

echo "LOGIN OK — token acquired"
AUTH="Authorization: Bearer $TOKEN"

# Test endpoints
ENDPOINTS=(
  "GET /api/health"
  "GET /api/system/status"
  "GET /api/system/db"
  "GET /api/config/mode"
  "GET /api/targets"
  "GET /api/traces"
  "GET /api/traces/services"
  "GET /api/traces/operations"
  "GET /api/metrics/catalog"
  "GET /api/alerts"
  "GET /api/alert-rules"
  "GET /api/dashboards"
  "GET /api/logs"
  "GET /api/network/devices"
  "GET /api/oracle/instances"
  "GET /api/postgres/overview"
  "GET /api/mysql/overview"
  "GET /api/mongodb/overview"
  "GET /api/sqlserver/overview"
  "GET /api/security/ids/rules"
  "GET /api/security/connections"
  "GET /api/security/firewall/stats"
  "GET /api/compliance/frameworks"
  "GET /api/sso/providers"
  "GET /api/tenants"
  "GET /api/ai/insights"
  "GET /api/ai/ml/status"
  "GET /api/predictions/summary"
  "GET /api/llm/overview"
  "GET /api/anomalies"
  "GET /api/runbooks"
  "GET /api/apdex"
  "GET /api/cilium/status"
  "GET /api/hubble/status"
  "GET /api/profiles"
  "GET /api/netflow/stats"
  "GET /api/snmp/traps"
  "GET /api/cloud/summary"
  "GET /api/storage-arrays"
  "GET /api/collectors"
  "GET /api/scrape/targets"
  "GET /api/events"
  "GET /api/search?q=test"
  "GET /api/mcp/tools"
  "GET /api/rca/history"
  "GET /api/otel/resources"
  "GET /api/otel/collector/status"
  "GET /api/otel/pipelines"
  "GET /api/query/traceql/validate?expr={status=error}"
  "GET /tempo/api/search?q={status=error}"
  "GET /api/llm-security/overview"
  "GET /api/notifications/channels"
  "GET /api/notifications/escalation-chains"
  "GET /api/admin/users"
  "GET /api/admin/roles"
  "GET /api/v1/status/buildinfo"
  "GET /api/v1/status/runtimeinfo"
)

echo ""
echo "=========================================="
echo " ENDPOINT VERIFICATION (${#ENDPOINTS[@]} endpoints)"
echo "=========================================="

PASS=0
FAIL=0
ERRORS=""

for entry in "${ENDPOINTS[@]}"; do
  METHOD=$(echo "$entry" | awk '{print $1}')
  EP=$(echo "$entry" | awk '{print $2}')
  CODE=$(curl -s -o /dev/null -w "%{http_code}" -X "$METHOD" "http://127.0.0.1:5006${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  if [ "$CODE" -ge 200 ] && [ "$CODE" -lt 300 ]; then
    echo "OK   $CODE  $EP"
    PASS=$((PASS+1))
  elif [ "$CODE" -eq 404 ]; then
    echo "MISS 404  $EP"
    FAIL=$((FAIL+1))
    ERRORS="$ERRORS\n  404 $EP"
  else
    echo "ERR  $CODE  $EP"
    FAIL=$((FAIL+1))
    ERRORS="$ERRORS\n  $CODE $EP"
  fi
done

echo ""
echo "=========================================="
echo " RESULTS: $PASS passed, $FAIL failed (of ${#ENDPOINTS[@]})"
if [ -n "$ERRORS" ]; then
  echo " Failed endpoints:"
  echo -e "$ERRORS"
fi
echo "=========================================="
