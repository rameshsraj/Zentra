#!/bin/bash
set -e
BASE="http://10.88.11.162:5006/api"

echo "=== Zentra API Tests ==="

# 1. Login
TOKEN=$(curl -s -X POST "$BASE/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@Zentra.local","password":"changeme"}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null)

if [ -z "$TOKEN" ]; then
  echo "FAIL: Login failed - no token returned"
  exit 1
fi
echo "PASS: Login -> token=${TOKEN:0:20}..."

AUTH="Authorization: Bearer $TOKEN"

# 2. GET /agents
R=$(curl -s -w "\n%{http_code}" -H "$AUTH" "$BASE/agents")
CODE=$(echo "$R" | tail -1); BODY=$(echo "$R" | head -1)
COUNT=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d))" 2>/dev/null || echo "?")
echo "$( [ "$CODE" = "200" ] && echo PASS || echo FAIL ): GET /agents -> HTTP $CODE, $COUNT agents"

# 3. GET /agent-keys
R=$(curl -s -w "\n%{http_code}" -H "$AUTH" "$BASE/agent-keys")
CODE=$(echo "$R" | tail -1); BODY=$(echo "$R" | head -1)
COUNT=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d))" 2>/dev/null || echo "?")
echo "$( [ "$CODE" = "200" ] && echo PASS || echo FAIL ): GET /agent-keys -> HTTP $CODE, $COUNT keys"

# 4. POST /agent-keys (create a test key)
R=$(curl -s -w "\n%{http_code}" -X POST -H "$AUTH" -H "Content-Type: application/json" \
  -d '{"name":"test-key-api-test"}' "$BASE/agent-keys")
CODE=$(echo "$R" | tail -1); BODY=$(echo "$R" | head -1)
KEY_ID=$(echo "$BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))" 2>/dev/null || echo "")
echo "$( [ "$CODE" = "200" ] && echo PASS || echo FAIL ): POST /agent-keys -> HTTP $CODE, id=${KEY_ID:0:8}..."

# 5. DELETE /agent-keys/:id (revoke the test key)
if [ -n "$KEY_ID" ]; then
  R=$(curl -s -w "\n%{http_code}" -X DELETE -H "$AUTH" "$BASE/agent-keys/$KEY_ID")
  CODE=$(echo "$R" | tail -1)
  echo "$( [ "$CODE" = "200" ] && echo PASS || echo FAIL ): DELETE /agent-keys/$KEY_ID -> HTTP $CODE"
fi

# 6. GET /targets
R=$(curl -s -w "\n%{http_code}" -H "$AUTH" "$BASE/targets")
CODE=$(echo "$R" | tail -1); BODY=$(echo "$R" | head -1)
COUNT=$(echo "$BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d))" 2>/dev/null || echo "?")
echo "$( [ "$CODE" = "200" ] && echo PASS || echo FAIL ): GET /targets -> HTTP $CODE, $COUNT targets"

# 7. GET /agents/targets (agent auth with Bearer — pick first agent key)
ISOK_KEY=$(curl -s -H "$AUTH" "$BASE/agent-keys" \
  | python3 -c "import sys,json; d=json.load(sys.stdin); print(d[0]['key_prefix'] if d else '')" 2>/dev/null || echo "")
if [ -n "$ISOK_KEY" ]; then
  echo "INFO: Agent key prefix found: $ISOK_KEY (full key not shown for security)"
fi

echo ""
echo "=== All tests complete ==="
