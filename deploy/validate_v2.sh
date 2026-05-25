#!/bin/bash
BASE="http://127.0.0.1:5006/api"

# Login
LOGIN=$(curl -sf -X POST "$BASE/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"changeme"}')
TOKEN=$(echo "$LOGIN" | grep -o '"token":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$TOKEN" ]; then
  echo "FATAL: Login failed"
  exit 1
fi
echo "Logged in OK (token: ${TOKEN:0:15}...)"
echo ""

check() {
  local label="$1"
  local url="$2"
  local code
  code=$(curl -sf -o /dev/null -w '%{http_code}' -H "Authorization: Bearer $TOKEN" "$url")
  if [ "$code" = "200" ]; then
    printf "  ✅  %-55s %s\n" "$label" "$code"
  elif [ "$code" = "404" ]; then
    printf "  ❌  %-55s %s\n" "$label" "$code"
  else
    printf "  ⚠️  %-55s %s\n" "$label" "$code"
  fi
}

echo "=== CORE ==="
check "GET /health"                              "$BASE/health"
check "GET /targets"                             "$BASE/targets"

echo ""
echo "=== AUTH ==="
check "GET /auth/me"                             "$BASE/auth/me"
check "GET /auth/sessions"                       "$BASE/auth/sessions"
check "GET /users"                               "$BASE/users"

echo ""
echo "=== ALERTS & EVENTS ==="
check "GET /alerts"                              "$BASE/alerts"
check "GET /alert-rules"                         "$BASE/alert-rules"
check "GET /events"                              "$BASE/events"
check "GET /events/stats"                        "$BASE/events/stats"

echo ""
echo "=== METRICS ==="
check "GET /metrics?name=cpu&from=2025-01-01"    "$BASE/metrics?name=cpu&from=2025-01-01T00:00:00Z&to=2026-01-01T00:00:00Z"
check "GET /metrics/aggregated"                  "$BASE/metrics/aggregated?name=cpu&from=2025-01-01T00:00:00Z&to=2026-01-01T00:00:00Z&step=1h"
check "GET /predictions/summary"                 "$BASE/predictions/summary"

echo ""
echo "=== DASHBOARDS ==="
check "GET /dashboards"                          "$BASE/dashboards"

echo ""
echo "=== NETWORK (frontend API client paths) ==="
check "GET /network/stats"                       "$BASE/network/stats"
check "GET /network/capture"                     "$BASE/network/capture"
check "GET /network/adapters"                    "$BASE/network/adapters"
check "GET /network/programs"                    "$BASE/network/programs"
check "GET /network/hosts"                       "$BASE/network/hosts"
check "GET /network/interfaces"                  "$BASE/network/interfaces"
check "GET /network/protocols"                   "$BASE/network/protocols"
check "GET /network/containers"                  "$BASE/network/containers"
check "GET /network/connections"                 "$BASE/network/connections"
check "GET /network/connections/paginated"       "$BASE/network/connections/paginated"
check "GET /network/blacklist"                   "$BASE/network/blacklist"
check "GET /network/devices"                     "$BASE/network/devices"
check "GET /network/traffic/chart?range=1h"      "$BASE/network/traffic/chart?range=1h"
check "GET /network/traffic/protocols"           "$BASE/network/traffic/protocols"
check "GET /network/deep/countries"              "$BASE/network/deep/countries"
check "GET /network/notification-rules"          "$BASE/network/notification-rules"
check "GET /network/notification-log"            "$BASE/network/notification-log"
check "GET /network/deep/services"               "$BASE/network/deep/services"
check "GET /network/socket-stats"                "$BASE/network/socket-stats"
check "GET /network/protocol-stats"              "$BASE/network/protocol-stats"
check "GET /network/topology"                    "$BASE/network/topology"

echo ""
echo "=== OTEL ==="
check "GET /otel/status"                         "$BASE/otel/status"
check "GET /otel/stats"                          "$BASE/otel/stats"
check "GET /otel/stats/timeseries"               "$BASE/otel/stats/timeseries?signal=traces&window=1h&step=5m"
check "GET /otel/resources"                      "$BASE/otel/resources"
check "GET /otel/connectors"                     "$BASE/otel/connectors"
check "GET /otel/derived-metrics"                "$BASE/otel/derived-metrics"
check "GET /otel/pipeline"                       "$BASE/otel/pipeline"
check "GET /otel/sdk-snippets"                   "$BASE/otel/sdk-snippets?language=python&framework=flask"

echo ""
echo "=== LLM / AI ==="
check "GET /llm/overview"                        "$BASE/llm/overview"
check "GET /llm/models"                          "$BASE/llm/models"
check "GET /llm/cost-trend"                      "$BASE/llm/cost-trend"
check "GET /llm/latency-distribution"            "$BASE/llm/latency-distribution"
check "GET /ai/anomalies"                        "$BASE/ai/anomalies"
check "GET /ai/ml/status"                        "$BASE/ai/ml/status"

echo ""
echo "=== LOGS ==="
check "GET /logs?level=error&limit=10"           "$BASE/logs?level=error&limit=10"
check "GET /log-sources"                         "$BASE/log-sources"
check "GET /logs/query?limit=10"                 "$BASE/logs/query?limit=10"
check "GET /logs/streams"                        "$BASE/logs/streams"
check "GET /logs/volume"                         "$BASE/logs/volume"

echo ""
echo "=== CILIUM ==="
check "GET /cilium/status"                       "$BASE/cilium/status"
check "GET /cilium/metrics/summary"              "$BASE/cilium/metrics/summary"
check "GET /hubble/status"                       "$BASE/hubble/status"
check "GET /hubble/flows"                        "$BASE/hubble/flows"

echo ""
echo "=== ORACLE ==="
check "GET /oracle/overview"                     "$BASE/oracle/overview"
check "GET /oracle/connections"                  "$BASE/oracle/connections"
check "GET /oracle/sysmetrics"                   "$BASE/oracle/sysmetrics"
check "GET /oracle/tablespaces"                  "$BASE/oracle/tablespaces"

echo ""
echo "=== MIDDLEWARE ==="
check "GET /middleware/dashboard"                "$BASE/middleware/dashboard"
check "GET /middleware/nginx"                    "$BASE/middleware/nginx"
check "GET /middleware/redis"                    "$BASE/middleware/redis"

echo ""
echo "=== SECURITY ==="
check "GET /security/ids/rules"                  "$BASE/security/ids/rules"
check "GET /security/ids/alerts"                 "$BASE/security/ids/alerts"
check "GET /security/ids/stats"                  "$BASE/security/ids/stats"
check "GET /security/firewall/rules"             "$BASE/security/firewall/rules"
check "GET /security/firewall/stats"             "$BASE/security/firewall/stats"
check "GET /security/connections"                "$BASE/security/connections"
check "GET /security/dns"                        "$BASE/security/dns"

echo ""
echo "=== INFRA ==="
check "GET /healthchecks"                        "$BASE/healthchecks"
check "GET /healthchecks/summary"                "$BASE/healthchecks/summary"
check "GET /runbooks"                            "$BASE/runbooks"
check "GET /anomalies"                           "$BASE/anomalies"
check "GET /deployments"                         "$BASE/deployments"

echo ""
echo "=== DONE ==="
