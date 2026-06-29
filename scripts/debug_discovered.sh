#!/usr/bin/env bash
BASE="http://localhost:5006"

TOKEN=$(curl -s -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"changeme"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin).get("token",""))')

AGENT=$(curl -s -X POST "$BASE/api/agents/register" \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"datacenter_name":"debug-dc"}')

AGENT_KEY=$(echo "$AGENT" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("api_key",""))')
AGENT_ID=$(echo "$AGENT" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("agent_id",""))')

echo "AGENT_ID: $AGENT_ID"
echo "AGENT_KEY prefix: ${AGENT_KEY:0:15}..."

# Test heartbeat first (same auth path) to verify key works
echo ""
echo "--- Heartbeat test ---"
curl -s -o /dev/null -w "Heartbeat HTTP: %{http_code}\n" \
  -X POST "$BASE/api/agents/heartbeat" \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $AGENT_KEY" \
  -d '{"agent_version":"1.0","active_targets":0,"os_name":"linux","dc_name":"debug-dc"}'

# Test discovered POST
echo ""
echo "--- Discovered POST test ---"
curl -s -w "\nHTTP: %{http_code}\n" \
  -X POST "$BASE/api/agents/$AGENT_ID/discovered" \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $AGENT_KEY" \
  -d '{"hosts":[{"ip_address":"10.0.0.1","open_ports":[22],"os_hint":"linux"}]}'

# Check DB directly
echo ""
echo "--- DB check ---"
psql 'postgresql://postgres:zantaz@10.88.11.162:5434/zentra' -c \
  "SELECT ip_address, opted_in, ignored FROM discovered_hosts WHERE agent_id = '$AGENT_ID';"

# Cleanup
curl -s -o /dev/null -X DELETE "$BASE/api/agents/$AGENT_ID" -H "Authorization: Bearer $TOKEN"
echo "Cleaned up agent"
