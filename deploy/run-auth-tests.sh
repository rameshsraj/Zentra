#!/bin/bash
# Authenticated full test suite for Zentra
BASE="http://127.0.0.1:5006"
PASS=0
FAIL=0

# Get JWT token
echo "--- Authenticating ---"
LOGIN_RESP=$(curl -s -X POST "$BASE/api/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@zentra.local","password":"changeme"}' 2>/dev/null)

echo "  Login response: $LOGIN_RESP"

TOKEN=$(echo "$LOGIN_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null || echo "")

if [ -z "$TOKEN" ]; then
  echo "WARNING: Could not get auth token. Running unauthenticated tests only."
else
  echo "  Got JWT token: ${TOKEN:0:20}..."
fi

AUTH="-H Authorization:\ Bearer\ $TOKEN"

check() {
    local name="$1"
    local url="$2"
    local method="${3:-GET}"
    local body="${4:-}"

    if [ "$method" = "POST" ]; then
        code=$(curl -s -o /tmp/test_out -w "%{http_code}" -X POST \
          -H "Content-Type: application/json" \
          -H "Authorization: Bearer $TOKEN" \
          -d "$body" "$url")
    else
        code=$(curl -s -o /tmp/test_out -w "%{http_code}" \
          -H "Authorization: Bearer $TOKEN" "$url")
    fi

    if [ "$code" -ge 200 ] && [ "$code" -lt 400 ]; then
        echo "  PASS  $name (HTTP $code)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $name (HTTP $code)"
        head -c 200 /tmp/test_out 2>/dev/null
        echo ""
        FAIL=$((FAIL + 1))
    fi
}

echo ""
echo "============================================"
echo " Zentra Authenticated Test Suite"
echo " $(date)"
echo "============================================"

echo ""
echo "--- Core ---"
check "Health" "$BASE/api/health"
check "System Status" "$BASE/api/system/status"
check "System DB" "$BASE/api/system/db"
check "Pool Status" "$BASE/api/system/pool-status"

echo ""
echo "--- Targets ---"
check "List Targets" "$BASE/api/targets"

echo ""
echo "--- Metrics ---"
check "Query Metrics" "$BASE/api/metrics?metric_name=cpu&last_minutes=60"
check "Aggregated Metrics" "$BASE/api/metrics/aggregated?metric_name=cpu&last_minutes=60"

echo ""
echo "--- Alerts ---"
check "List Alerts" "$BASE/api/alerts"
check "Alert Rules" "$BASE/api/alert-rules"

echo ""
echo "--- Logs ---"
check "Search Logs" "$BASE/api/logs?q=error"

echo ""
echo "--- Events ---"
check "Recent Events" "$BASE/api/events?limit=10"

echo ""
echo "--- Network ---"
check "Network Connections" "$BASE/api/network/connections"
check "Network Hosts" "$BASE/api/network/hosts"
check "Network Bandwidth" "$BASE/api/network/bandwidth"

echo ""
echo "--- AI ---"
check "AI Insights" "$BASE/api/ai/insights"

echo ""
echo "--- Predictions ---"
check "Forecasts" "$BASE/api/predictions/forecasts"
check "Anomalies" "$BASE/api/predictions/anomalies"

echo ""
echo "--- Dashboards ---"
check "List Dashboards" "$BASE/api/dashboards"

echo ""
echo "--- Scrape ---"
check "Scrape Targets" "$BASE/api/scrape/targets"
check "Scrape Health" "$BASE/api/scrape/health"

echo ""
echo "--- Notifications ---"
check "Notification Channels" "$BASE/api/notifications/channels"

echo ""
echo "--- Search ---"
check "Global Search" "$BASE/api/search?q=cpu"

echo ""
echo "--- OTel Endpoints ---"
check "OTel Status" "$BASE/api/otel/status"
check "OTel Stats" "$BASE/api/otel/stats"
check "OTel Stats Timeseries" "$BASE/api/otel/stats/timeseries"
check "OTel Resources" "$BASE/api/otel/resources"
check "OTel Attributes" "$BASE/api/otel/attributes"
check "OTel SemConv" "$BASE/api/otel/semconv"
check "OTel Pipeline" "$BASE/api/otel/pipeline"
check "OTel Span Metrics" "$BASE/api/otel/span-metrics"
check "OTel Alert Rules" "$BASE/api/otel/alert-rules"
check "OTel GenAI" "$BASE/api/otel/genai"
check "OTel API Keys" "$BASE/api/otel/api-keys"
check "OTel Ingest Audit" "$BASE/api/otel/ingest-audit"
check "OTel Trace Aggregates" "$BASE/api/otel/trace-aggregates"
check "OTel Connectors" "$BASE/api/otel/connectors"
check "OTel Derived Metrics" "$BASE/api/otel/derived-metrics"
check "OTel Collector Config" "$BASE/api/otel/collector-config"
check "OTel SDK Snippets" "$BASE/api/otel/sdk-snippets"

echo ""
echo "--- OTLP Ingest ---"
check "OTLP Traces Ingest" "$BASE/v1/traces" "POST" '{"resourceSpans":[]}'
check "OTLP Metrics Ingest" "$BASE/v1/metrics" "POST" '{"resourceMetrics":[]}'
check "OTLP Logs Ingest" "$BASE/v1/logs" "POST" '{"resourceLogs":[]}'

echo ""
echo "--- Frontend ---"
check "Frontend Index" "http://127.0.0.1:5010/"

echo ""
echo "============================================"
echo " RESULTS: $PASS passed, $FAIL failed"
echo " Total: $((PASS + FAIL)) tests"
if [ $FAIL -gt 0 ]; then
    echo " STATUS: SOME FAILURES"
else
    echo " STATUS: ALL PASSED"
fi
echo "============================================"
