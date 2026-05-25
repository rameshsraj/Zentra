#!/bin/bash
# Quick test of individual endpoints with verbose output
TOKEN=$(curl -sf -X POST http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' -d @/tmp/login.json \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')

CID=$(curl -sf -X POST http://127.0.0.1:5006/api/oracle/connections \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $TOKEN" \
  -d @/tmp/oraconn.json \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["id"])')

echo "TOKEN=${TOKEN:0:20}..."
echo "CID=$CID"

for ep in wait-classes top-queries running-queries sysstat sysmetrics pdb rman rac performance-history; do
  echo ""
  echo "=== $ep ==="
  curl -sf "http://127.0.0.1:5006/api/oracle/${ep}?connection_id=${CID}" \
    -H "Authorization: Bearer $TOKEN" 2>&1 | python3 -m json.tool 2>/dev/null || echo "(raw output above)"
done
