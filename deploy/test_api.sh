#!/usr/bin/env bash
set -euo pipefail
RESP=$(curl -sf -X POST http://localhost:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"Zentra@Admin1"}')
echo "Login response: $RESP" | head -c 200
TOKEN=$(echo "$RESP" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("token","NO_TOKEN"))' 2>/dev/null || echo "PARSE_FAILED")
echo "Token prefix: ${TOKEN:0:30}"
echo ""
echo "=== /api/agents/by-dc/pgmig-svsossib/components ==="
curl -si "http://localhost:5006/api/agents/by-dc/pgmig-svsossib/components" \
  -H "Authorization: Bearer $TOKEN"
