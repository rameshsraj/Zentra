#!/bin/bash
# Endpoint verification script for the local WSL deployment.

cd /store/Zentra
set -euo pipefail

BASE="${BASE:-http://127.0.0.1:5006}"
LOGIN_EMAIL="${LOGIN_EMAIL:-admin@Zentra.local}"
LOGIN_PASSWORD="${LOGIN_PASSWORD:-changeme}"

LOGIN=$(curl -s -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$LOGIN_EMAIL\",\"password\":\"$LOGIN_PASSWORD\"}")
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("token",""))' 2>/dev/null || true)

if [ -z "$TOKEN" ]; then
  echo "Auth failed for $LOGIN_EMAIL - response: $LOGIN"
  exit 1
fi

AUTH="Authorization: Bearer $TOKEN"
echo "Authenticated as $LOGIN_EMAIL"

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
  "/api/oracle/overview"
  "/api/oracle/connections"
  "/api/oracle/metrics"
  "/api/oracle/scrape/status"
  "/api/oracle/txeventq"
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
  "/api/otel/status"
  "/api/otel/pipeline"
  "/api/llm-security/overview"
  "/api/notifications/channels"
  "/api/notifications/escalation-chains"
  "/api/users"
  "/api/roles"
  "/api/v1/status/buildinfo"
  "/api/v1/status/runtimeinfo"
  "/api/v1/status/config"
  "/api/v1/status/tsdb"
  "/api/auth/me"
  "/api/search?q=cpu"
  "/api/drilldown?level=global"
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
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}${EP}" -H "$AUTH" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
  else
    CODE=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}${EP}" -H "Content-Type: application/json" --max-time 5 2>/dev/null)
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
if [ "$AUTH_FAIL" -ne 0 ] || [ "$FAIL" -ne 0 ]; then
  exit 1
fi
echo " RESULTS: $PASS OK, $AUTH_FAIL auth-blocked, $FAIL failed"
echo " Total: ${#ENDPOINTS[@]} endpoints"
if [ -n "$ERRORS" ]; then
  echo ""
  echo " Issues:"
  echo -e "$ERRORS"
fi
echo "=========================================="
