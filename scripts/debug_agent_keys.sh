#!/bin/bash
BASE="http://10.88.11.162:5006/api"
TOKEN=$(curl -s -X POST "$BASE/auth/login" \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@Zentra.local","password":"changeme"}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null)

echo "Token: ${TOKEN:0:20}..."

echo "--- GET /agent-keys response ---"
curl -s -H "Authorization: Bearer $TOKEN" "$BASE/agent-keys"
echo ""

echo "--- POST /agent-keys response ---"
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"name":"test-debug"}' "$BASE/agent-keys"
echo ""
