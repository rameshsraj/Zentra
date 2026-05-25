#!/bin/bash
# Zentra Endpoint Validation Script
BASE="http://127.0.0.1:5006/api"

# Step 1: Login
echo "=== LOGGING IN ==="
LOGIN=$(curl -sf -X POST "$BASE/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"changeme"}')

if [ -z "$LOGIN" ]; then
  echo "Login with admin@Zentra.local failed, trying ramesh@smarsh.com..."
  LOGIN=$(curl -sf -X POST "$BASE/auth/login" \
    -H 'Content-Type: application/json' \
    -d '{"email":"ramesh@smarsh.com","password":"changeme"}')
fi

if [ -z "$LOGIN" ]; then
  echo "FAILED: Could not login. Trying without auth..."
  TOKEN=""
else
  TOKEN=$(echo "$LOGIN" | grep -o '"token":"[^"]*"' | head -1 | cut -d'"' -f4)
  echo "TOKEN: ${TOKEN:0:20}..."
fi

AUTH=""
if [ -n "$TOKEN" ]; then
  AUTH="-H Authorization:\ Bearer\ $TOKEN"
fi

# Helper function
check() {
  local label="$1"
  local url="$2"
  local code
  if [ -n "$TOKEN" ]; then
    code=$(curl -sf -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" "$url")
  else
    code=$(curl -sf -o /dev/null -w '%{http_code}' "$url")
  fi
  printf "%-50s %s\n" "$label" "$code"
}

check_post() {
  local label="$1"
  local url="$2"
  local data="$3"
  local code
  if [ -n "$TOKEN" ]; then
    code=$(curl -sf -o /dev/null -w '%{http_code}' -X POST -H "Content-Type: application/json" -H "Authorization: Bearer $TOKEN" -d "$data" "$url")
  else
    code=$(curl -sf -o /dev/null -w '%{http_code}' -X POST -H "Content-Type: application/json" -d "$data" "$url")
  fi
  printf "%-50s %s\n" "$label" "$code"
}

echo ""
echo "=== CORE ENDPOINTS ==="
check "GET /api/health"                          "$BASE/health"
check "GET /api/status"                          "$BASE/status"
check "GET /api/targets"                         "$BASE/targets"

echo ""
echo "=== ALERTS & EVENTS ==="
check "GET /api/alerts"                          "$BASE/alerts"
check "GET /api/alert-rules"                     "$BASE/alert-rules"
check "GET /api/events"                          "$BASE/events"
check "GET /api/events/stats"                    "$BASE/events/stats"

echo ""
echo "=== METRICS & SERVERS ==="
check "GET /api/metrics/latest"                  "$BASE/metrics/latest"
check "GET /api/servers/stats"                   "$BASE/servers/stats"

echo ""
echo "=== DASHBOARDS ==="
check "GET /api/dashboards"                      "$BASE/dashboards"

echo ""
echo "=== PREDICTIONS ==="
check "GET /api/predictions/summary"             "$BASE/predictions/summary"

echo ""
echo "=== NETWORK ENDPOINTS ==="
check "GET /api/network/stats"                   "$BASE/network/stats"
check "GET /api/network/capture"                 "$BASE/network/capture"
check "GET /api/network/adapters"                "$BASE/network/adapters"
check "GET /api/network/programs"                "$BASE/network/programs"
check "GET /api/network/hosts"                   "$BASE/network/hosts"
check "GET /api/network/interfaces"              "$BASE/network/interfaces"
check "GET /api/network/protocols"               "$BASE/network/protocols"
check "GET /api/network/sockets"                 "$BASE/network/sockets"
check "GET /api/network/containers"              "$BASE/network/containers"
check "GET /api/network/services"                "$BASE/network/services"
check "GET /api/network/countries"               "$BASE/network/countries"
check "GET /api/network/connections"             "$BASE/network/connections"
check "GET /api/network/blacklist"               "$BASE/network/blacklist"
check "GET /api/network/notifications"           "$BASE/network/notifications"
check "GET /api/network/devices"                 "$BASE/network/devices"
check "GET /api/network/traffic-chart?range=1h"  "$BASE/network/traffic-chart?range=1h"

echo ""
echo "=== OTEL ENDPOINTS ==="
check "GET /api/otel/status"                     "$BASE/otel/status"
check "GET /api/otel/stats"                      "$BASE/otel/stats"
check "GET /api/otel/stats/timeseries?signal=traces&window=1h&step=5m" "$BASE/otel/stats/timeseries?signal=traces&window=1h&step=5m"
check "GET /api/otel/resources"                  "$BASE/otel/resources"
check "GET /api/otel/connectors"                 "$BASE/otel/connectors"
check "GET /api/otel/derived-metrics"            "$BASE/otel/derived-metrics"
check "GET /api/otel/pipeline"                   "$BASE/otel/pipeline"
check "GET /api/otel/sdk-snippets?language=python&framework=flask" "$BASE/otel/sdk-snippets?language=python&framework=flask"

echo ""
echo "=== LLM / AI ENDPOINTS ==="
check "GET /api/llm/overview"                    "$BASE/llm/overview"
check "GET /api/llm/models"                      "$BASE/llm/models"
check "GET /api/llm/cost-trend"                  "$BASE/llm/cost-trend"
check "GET /api/llm/latency-distribution"        "$BASE/llm/latency-distribution"

echo ""
echo "=== LOG ENDPOINTS ==="
check "GET /api/logs/search"                     "$BASE/logs/search"
check "GET /api/logs/sources"                    "$BASE/logs/sources"
check "GET /api/logs/stats"                      "$BASE/logs/stats"

echo ""
echo "=== AUTH ENDPOINTS ==="
check "GET /api/auth/me"                         "$BASE/auth/me"
check "GET /api/auth/sessions"                   "$BASE/auth/sessions"
check "GET /api/users"                           "$BASE/users"

echo ""
echo "=== CILIUM ==="
check "GET /api/cilium/status"                   "$BASE/cilium/status"
check "GET /api/cilium/flows"                    "$BASE/cilium/flows"
check "GET /api/cilium/endpoints"                "$BASE/cilium/endpoints"

echo ""
echo "=== ORACLE ==="
check "GET /api/oracle/instances"                "$BASE/oracle/instances"

echo ""
echo "=== MIDDLEWARE ==="
check "GET /api/middleware/services"              "$BASE/middleware/services"

echo ""
echo "=== SECURITY ==="
check "GET /api/security/threats"                "$BASE/security/threats"
check "GET /api/security/firewall-rules"         "$BASE/security/firewall-rules"

echo ""
echo "=== DONE ==="
