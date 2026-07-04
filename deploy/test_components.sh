#!/usr/bin/env bash
set -euo pipefail
TOKEN=$(curl -sf -X POST http://localhost:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"Admin@Zentra123"}' || true \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["token"])' 2>/dev/null)
echo "Token: ${TOKEN:0:20}..."
echo ""
echo "=== GET /api/agents/by-dc/pgmig-svsossib/components ==="
curl -si "http://localhost:5006/api/agents/by-dc/pgmig-svsossib/components" \
  -H "Authorization: Bearer $TOKEN" 2>&1
