#!/bin/bash

# Source port configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true

PASS=0
FAIL=0
BASE="http://127.0.0.1:${ZENTRA_FRONTEND_PORT}"

check() {
    local name="$1"
    local url="$2"
    local method="${3:-GET}"
    local body="$4"
    
    if [ "$method" = "POST" ]; then
        code=$(curl -s -o /tmp/test_out -w "%{http_code}" -X POST -H "Content-Type: application/json" -d "$body" "$url")
    else
        code=$(curl -s -o /tmp/test_out -w "%{http_code}" "$url")
    fi
    
    if [ "$code" -ge 200 ] && [ "$code" -lt 400 ]; then
        echo "  PASS  $name (HTTP $code)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $name (HTTP $code)"
        cat /tmp/test_out | head -c 200
        echo ""
        FAIL=$((FAIL + 1))
    fi
}

echo "============================================"
echo " Zentra Full Test Suite"
echo "============================================"
echo ""

echo "--- Frontend ---"
check "Frontend index" "$BASE/"

echo ""
echo "--- Core API ---"
check "Health" "$BASE/api/health"
check "System Status" "$BASE/api/system/status"
check "System DB" "$BASE/api/system/db"

echo ""
echo "--- Targets ---"
check "List Targets" "$BASE/api/targets"

echo ""
echo "--- Metrics ---"
check "Query Metrics" "$BASE/api/metrics?query=cpu&range=1h"
check "Aggregated Metrics" "$BASE/api/metrics/aggregated?query=cpu"

echo ""
echo "--- Alerts ---"
check "List Alerts" "$BASE/api/alerts"
check "Alert Rules" "$BASE/api/alert-rules"
check "Alert Silences" "$BASE/api/alerts/silences"
check "Alert Templates" "$BASE/api/alerts/templates"

echo ""
echo "--- Logs ---"
check "Search Logs" "$BASE/api/logs?q=error"

echo ""
echo "--- Events ---"
check "Recent Events" "$BASE/api/events?limit=10"
check "Event Stats" "$BASE/api/events/stats"

echo ""
echo "--- Network ---"
check "Network Capture" "$BASE/api/network/capture"
check "Network Connections" "$BASE/api/network/connections"
check "Network Hosts" "$BASE/api/network/hosts"
check "Network Protocols" "$BASE/api/network/protocols"
check "Network Bandwidth" "$BASE/api/network/bandwidth"
check "Deep Traffic Snapshot" "$BASE/api/network/deep/snapshot"
check "Service Breakdown" "$BASE/api/network/deep/services"
check "ICMP Breakdown" "$BASE/api/network/deep/icmp"
check "Threat Status" "$BASE/api/network/deep/threat-status"

echo ""
echo "--- AI / Chatbot ---"
check "AI Insights" "$BASE/api/ai/insights"
check "AI Chat" "$BASE/api/ai/chat" "POST" '{"message":"hello","session_id":"test"}'
check "Chat History" "$BASE/api/ai/chat/history?session_id=test"

echo ""
echo "--- MCP Server ---"
check "MCP Tools List" "$BASE/api/mcp/tools"
check "MCP Tool Call" "$BASE/api/mcp/call" "POST" '{"tool":"server_list_all","params":{}}'
check "MCP Chat (MCP mode)" "$BASE/api/mcp/chat" "POST" '{"message":"Show Oracle tablespace usage","session_id":"test","mode":"mcp"}'
check "MCP Chat (LLM mode)" "$BASE/api/mcp/chat" "POST" '{"message":"What is the system status?","session_id":"test","mode":"llm"}'
check "MCP Chat (API mode)" "$BASE/api/mcp/chat" "POST" '{"message":"How to check alerts?","session_id":"test","mode":"api"}'
check "MCP Oracle RAC" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_rac_status","params":{}}'
check "MCP Oracle DataGuard" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_dataguard_status","params":{}}'
check "MCP Oracle RMAN" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_rman_status","params":{}}'
check "MCP Oracle Wait Events" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_wait_events","params":{}}'
check "MCP Oracle Long Queries" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_long_queries","params":{}}'
check "MCP Oracle PDB/CDB" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_pdb_status","params":{}}'
check "MCP Oracle Connections" "$BASE/api/mcp/call" "POST" '{"tool":"oracle_get_connections","params":{}}'
check "MCP Server CPU" "$BASE/api/mcp/call" "POST" '{"tool":"server_get_cpu","params":{"host":"prod-web-01"}}'
check "MCP Server Memory" "$BASE/api/mcp/call" "POST" '{"tool":"server_get_memory","params":{"host":"prod-web-01"}}'
check "MCP Server Disk" "$BASE/api/mcp/call" "POST" '{"tool":"server_get_disk","params":{"host":"prod-web-01"}}'
check "MCP Network Traffic" "$BASE/api/mcp/call" "POST" '{"tool":"network_get_traffic","params":{}}'
check "MCP Network Threats" "$BASE/api/mcp/call" "POST" '{"tool":"network_get_threats","params":{}}'
check "MCP Storage Arrays" "$BASE/api/mcp/call" "POST" '{"tool":"storage_get_arrays","params":{}}'
check "MCP Storage Forecast" "$BASE/api/mcp/call" "POST" '{"tool":"storage_get_capacity_forecast","params":{}}'
check "MCP Alerts Active" "$BASE/api/mcp/call" "POST" '{"tool":"alerts_get_active","params":{}}'
check "MCP Logs Search" "$BASE/api/mcp/call" "POST" '{"tool":"logs_search","params":{"query":"error"}}'
check "MCP Error Analysis" "$BASE/api/mcp/call" "POST" '{"tool":"logs_error_analysis","params":{}}'
check "MCP Anomalies" "$BASE/api/mcp/call" "POST" '{"tool":"predictions_get_anomalies","params":{}}'
check "MCP System Status" "$BASE/api/mcp/call" "POST" '{"tool":"system_get_status","params":{}}'

echo ""
echo "--- RCA ---"
check "RCA Analyze" "$BASE/api/rca/analyze" "POST" '{"log_content":"ORA-04031: unable to allocate","source":"oracle"}'
check "RCA History" "$BASE/api/rca/history"

echo ""
echo "--- Predictions ---"
check "Forecasts" "$BASE/api/predictions/forecasts"
check "Anomalies" "$BASE/api/predictions/anomalies"
check "Trend Alerts" "$BASE/api/predictions/trend-alerts"
check "Prediction Summary" "$BASE/api/predictions/summary"

echo ""
echo "--- Licenses ---"
check "List Licenses" "$BASE/api/admin/licenses"
check "Compliance" "$BASE/api/admin/licenses/compliance"
check "Expiring" "$BASE/api/admin/licenses/expiring"

echo ""
echo "--- Dashboards ---"
check "List Dashboards" "$BASE/api/dashboards"

echo ""
echo "--- Scrape ---"
check "Scrape Targets" "$BASE/api/scrape/targets"
check "Scrape Health" "$BASE/api/scrape/health"

echo ""
echo "--- Annotations ---"
check "List Annotations" "$BASE/api/annotations"

echo ""
echo "--- Notifications ---"
check "Notification Channels" "$BASE/api/notifications/channels"
check "Notification History" "$BASE/api/notifications/history"

echo ""
echo "--- Log Sources ---"
check "List Log Sources" "$BASE/api/log-sources"

echo ""
echo "--- Search ---"
check "Global Search" "$BASE/api/search?q=cpu"

echo ""
echo "--- Drilldown ---"
check "Drilldown" "$BASE/api/drilldown?type=metric&id=cpu"

echo ""
echo "--- Correlation ---"
check "Logs for Metric" "$BASE/api/correlation/logs-for-metric?metric=cpu&range=1h"
check "Metrics for Log" "$BASE/api/correlation/metrics-for-log?log_id=test"

echo ""
echo "============================================"
echo " RESULTS: $PASS passed, $FAIL failed"
echo " Total: $((PASS + FAIL)) tests"
echo "============================================"
