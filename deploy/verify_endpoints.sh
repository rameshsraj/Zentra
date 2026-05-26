#!/bin/bash
# Endpoint verification script that bypasses login by crafting a JWT directly.

cd /store/Zentra
set -a; source .env 2>/dev/null; set +a

JWT_SECRET="${JWT_SECRET:-zentra-secret-key}"

# Get admin user info
ADMIN_INFO=$(PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d postgres -t -A -F'|' -c \
  "SELECT u.id, COALESCE(u.org_id, '00000000-0000-0000-0000-000000000001'), u.email, COALESCE(r.name, 'Admin')
   FROM users u
   LEFT JOIN user_roles ur ON ur.user_id = u.id
   LEFT JOIN roles r ON r.role_id = ur.role_id
   WHERE u.email='admin@smarsh-nexus.com' LIMIT 1;")

USER_ID=$(echo "$ADMIN_INFO" | cut -d'|' -f1)
ORG_ID=$(echo "$ADMIN_INFO" | cut -d'|' -f2)
EMAIL=$(echo "$ADMIN_INFO" | cut -d'|' -f3)
ROLE=$(echo "$ADMIN_INFO" | cut -d'|' -f4)

echo "User: $EMAIL  Org: $ORG_ID  Role: $ROLE"

# Generate JWT using python
NOW=$(date +%s)
EXP=$((NOW + 28800))

TOKEN=$(python3.11 -c "
import json, base64, hmac, hashlib

header = base64.urlsafe_b64encode(json.dumps({'alg':'HS256','typ':'JWT'}).encode()).rstrip(b'=').decode()
payload = base64.urlsafe_b64encode(json.dumps({
    'sub': '$USER_ID',
    'org': '$ORG_ID',
    'email': '$EMAIL',
    'role': '$ROLE',
    'exp': $EXP,
    'iat': $NOW,
    'jti': 'verify-$(date +%s)'
}).encode()).rstrip(b'=').decode()
sig_input = f'{header}.{payload}'.encode()
sig = base64.urlsafe_b64encode(hmac.new('$JWT_SECRET'.encode(), sig_input, hashlib.sha256).digest()).rstrip(b'=').decode()
print(f'{header}.{payload}.{sig}')
")

echo "Token generated (${#TOKEN} chars)"

# Verify token works
HEALTH_RESP=$(curl -s http://127.0.0.1:5006/api/system/status -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" --max-time 5 -w "\n%{http_code}" 2>/dev/null)
HEALTH_CODE=$(echo "$HEALTH_RESP" | tail -1)
echo "Auth test /api/system/status: $HEALTH_CODE"

if [ "$HEALTH_CODE" != "200" ]; then
  echo "Auth failed - response: $(echo "$HEALTH_RESP" | head -1)"
  echo "Falling back to unauthenticated testing..."
  AUTH=""
else
  AUTH="Authorization: Bearer $TOKEN"
fi

# Test all endpoints
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
  "/api/auth/me"
  "/api/search?q=cpu"
  "/api/drilldown?metric=cpu"
  "/api/events/stats"
)

echo ""
echo "=========================================="
echo " ENDPOINT VERIFICATION (${#ENDPOINTS[@]} endpoints)"
echo "=========================================="

PASS=0
FAIL=0
AUTH_FAIL=0
ERRORS=""

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
    ERRORS="${ERRORS}\n  404 $EP"
  elif [ "$CODE" -eq 500 ]; then
    echo "ERR  500  $EP"
    FAIL=$((FAIL+1))
    ERRORS="${ERRORS}\n  500 $EP"
  else
    echo "???  $CODE  $EP"
    FAIL=$((FAIL+1))
    ERRORS="${ERRORS}\n  $CODE $EP"
  fi
done

echo ""
echo "=========================================="
echo " RESULTS: $PASS OK, $AUTH_FAIL auth-blocked, $FAIL failed"
echo " Total: ${#ENDPOINTS[@]} endpoints"
if [ -n "$ERRORS" ]; then
  echo ""
  echo " Issues:"
  echo -e "$ERRORS"
fi
echo "=========================================="
