#!/usr/bin/env bash
TOKEN=$(curl -s -X POST http://localhost:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"changeme"}' \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])' 2>/dev/null)
echo "token: ${TOKEN:0:40}..."
echo ""
curl -si "http://localhost:5006/api/agents/by-dc/pgmig-svsossib/components" \
  -H "Authorization: Bearer $TOKEN" 2>&1
