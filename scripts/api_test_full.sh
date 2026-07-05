#!/usr/bin/env bash
# Full API test — old endpoints + new discovered-hosts endpoints
set -euo pipefail
BASE="http://localhost:5006"

pass() { echo "PASS: $1"; }
fail() { echo "FAIL: $1"; }
check() { [ "$1" = "$2" ] && pass "$3 -> $1" || fail "$3 -> got $1 (expected $2)"; }

echo "=== AUTH ==="
LOGIN=$(curl -s -X POST "$BASE/api/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"changeme"}')
TOKEN=$(echo "$LOGIN" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("token",""))' 2>/dev/null)
[ -n "$TOKEN" ] && pass "POST /api/auth/login" || { fail "POST /api/auth/login -> $LOGIN"; exit 1; }
AUTH="Authorization: Bearer $TOKEN"

echo ""
echo "=== AGENT FLEET ==="
R=$(curl -s -o /dev/null -w '%{http_code}' -H "$AUTH" "$BASE/api/agents")
check "$R" "200" "GET /api/agents"

AGENT=$(curl -s -X POST "$BASE/api/agents/register" \
  -H 'Content-Type: application/json' -H "$AUTH" \
  -d '{"datacenter_name":"test-dc","region_label":"US-East"}')
AGENT_ID=$(echo "$AGENT" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("agent_id",""))' 2>/dev/null)
AGENT_KEY=$(echo "$AGENT" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("api_key",""))' 2>/dev/null)
[ -n "$AGENT_ID" ] && pass "POST /api/agents/register -> agent_id=${AGENT_ID:0:8}..." || fail "POST /api/agents/register -> $AGENT"

echo ""
echo "=== HEARTBEAT ==="
R=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/agents/heartbeat" \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $AGENT_KEY" \
  -d '{"agent_version":"1.0.0","active_targets":2,"os_name":"linux","dc_name":"test-dc"}')
check "$R" "200" "POST /api/agents/heartbeat"

echo ""
echo "=== COMPONENT CONFIRMATION -> SUBTYPE-SPECIFIC AGENT TARGETS ==="
COMP_RESP=$(curl -s -X POST "$BASE/api/agents/by-dc/test-dc/components" \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $AGENT_KEY" \
  -d '{"components":[
    {"component_type":"linux","component_name":"Linux Host","address":"10.88.11.50","port":22},
    {"component_type":"postgresql","component_name":"PostgreSQL","address":"10.88.11.50:5432","port":5432},
    {"component_type":"nginx","component_name":"Nginx","address":"10.88.11.50:80","port":80},
    {"component_type":"redis","component_name":"Redis","address":"10.88.11.50:6379","port":6379}
  ]}')
UPSERTED=$(echo "$COMP_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("upserted",""))' 2>/dev/null)
check "$UPSERTED" "4" "POST /api/agents/by-dc/:dc/components (4 concrete component types)"

COMP_LIST=$(curl -s -H "$AUTH" "$BASE/api/agents/by-dc/test-dc/components")
COMP_IDS=$(echo "$COMP_LIST" | python3 -c 'import sys,json; data=json.load(sys.stdin); print(",".join(c["id"] for c in data.get("components",[]) if c.get("component_type") in {"linux","postgresql","nginx","redis"}))' 2>/dev/null)
if [ -n "$COMP_IDS" ]; then
  pass "GET /api/agents/by-dc/:dc/components -> selected component ids"
else
  fail "GET /api/agents/by-dc/:dc/components -> $COMP_LIST"
fi

CONFIRM_PAYLOAD=$(COMP_IDS="$COMP_IDS" python3 - <<'PY'
import json, os
ids=[x for x in os.environ["COMP_IDS"].split(",") if x]
print(json.dumps({"datacenter_name":"test-dc","component_ids":ids}))
PY
)
CONFIRM_RESP=$(curl -s -X POST "$BASE/api/agents/components/confirm" \
  -H 'Content-Type: application/json' -H "$AUTH" \
  -d "$CONFIRM_PAYLOAD")
CREATED=$(echo "$CONFIRM_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("created",""))' 2>/dev/null)
check "$CREATED" "4" "POST /api/agents/components/confirm -> creates subtype-specific targets"

AGENT_TARGETS=$(curl -s -H "Authorization: Bearer $AGENT_KEY" "$BASE/api/agents/targets")
SUBTYPE_CHECK=$(echo "$AGENT_TARGETS" | python3 -c 'import sys,json; data=json.load(sys.stdin); sub={t.get("target_subtype") for t in data}; hosts_ok=all(":" not in t.get("host","") for t in data); print("ok" if {"linux","postgresql","nginx","redis"}.issubset(sub) and hosts_ok else f"bad:{sorted(sub)}:hosts_ok={hosts_ok}")' 2>/dev/null)
check "$SUBTYPE_CHECK" "ok" "GET /api/agents/targets returns assigned concrete subtypes and host without port"

echo ""
echo "=== DISCOVERED HOSTS (new) ==="
R=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/api/agents/$AGENT_ID/discovered" \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $AGENT_KEY" \
  -d '{"hosts":[{"ip_address":"10.88.11.50","hostname":"db01","open_ports":[22,5432],"os_hint":"linux"},{"ip_address":"10.88.11.51","hostname":"web01","open_ports":[22,80,443],"os_hint":"linux"}]}')
check "$R" "200" "POST /api/agents/:id/discovered (agent auth)"

DISC=$(curl -s -H "$AUTH" "$BASE/api/agents/$AGENT_ID/discovered")
DISC_COUNT=$(echo "$DISC" | python3 -c 'import sys,json; print(len(json.load(sys.stdin)))' 2>/dev/null)
check "$DISC_COUNT" "2" "GET /api/agents/:id/discovered (user auth, host count)"

HOST_ID=$(echo "$DISC" | python3 -c 'import sys,json; print(json.load(sys.stdin)[0]["id"])' 2>/dev/null)
R=$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/agents/$AGENT_ID/discovered/$HOST_ID" \
  -H 'Content-Type: application/json' -H "$AUTH" \
  -d '{"opted_in":true}')
check "$R" "200" "PATCH /api/agents/:id/discovered/:host_id (opted_in=true)"

HOST_ID2=$(echo "$DISC" | python3 -c 'import sys,json; print(json.load(sys.stdin)[1]["id"])' 2>/dev/null)
R=$(curl -s -o /dev/null -w '%{http_code}' -X PATCH "$BASE/api/agents/$AGENT_ID/discovered/$HOST_ID2" \
  -H 'Content-Type: application/json' -H "$AUTH" \
  -d '{"ignored":true}')
check "$R" "200" "PATCH /api/agents/:id/discovered/:host_id (ignored=true)"

echo ""
echo "=== AGENT KEYS ==="
R=$(curl -s -o /dev/null -w '%{http_code}' -H "$AUTH" "$BASE/api/agent-keys")
check "$R" "200" "GET /api/agent-keys"

KEY_RESP=$(curl -s -X POST "$BASE/api/agent-keys" \
  -H 'Content-Type: application/json' -H "$AUTH" \
  -d '{"name":"test-key"}')
KEY_ID=$(echo "$KEY_RESP" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])' 2>/dev/null)
[ -n "$KEY_ID" ] && pass "POST /api/agent-keys -> id=${KEY_ID:0:8}..." || fail "POST /api/agent-keys -> $KEY_RESP"

R=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE -H "$AUTH" "$BASE/api/agent-keys/$KEY_ID")
check "$R" "200" "DELETE /api/agent-keys/:id"

echo ""
echo "=== TARGETS ==="
R=$(curl -s -o /dev/null -w '%{http_code}' -H "$AUTH" "$BASE/api/targets")
check "$R" "200" "GET /api/targets"

TGT=$(curl -s -X POST "$BASE/api/targets" \
  -H 'Content-Type: application/json' -H "$AUTH" \
  -d "{\"name\":\"test-server\",\"target_subtype\":\"linux\",\"host\":\"10.88.11.50\",\"port\":22,\"environment\":\"test\",\"agent_id\":\"$AGENT_ID\"}")
TGT_ID=$(echo "$TGT" | python3 -c 'import sys,json; print(json.load(sys.stdin).get("target_id",""))' 2>/dev/null)
[ -n "$TGT_ID" ] && pass "POST /api/targets (agent-aware, SSH skipped) -> target_id=${TGT_ID:0:8}..." || fail "POST /api/targets -> $TGT"

R=$(curl -s -o /dev/null -w '%{http_code}' -H "$AUTH" "$BASE/api/targets/$TGT_ID")
check "$R" "200" "GET /api/targets/:id"

echo ""
echo "=== CLEANUP ==="
R=$(curl -s -o /dev/null -w '%{http_code}' -X DELETE -H "$AUTH" "$BASE/api/agents/$AGENT_ID")
check "$R" "200" "DELETE /api/agents/:id (deregister)"

echo ""
echo "=== ALL TESTS COMPLETE ==="
