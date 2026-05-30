#!/bin/bash
TOKEN=$(curl -s -X POST http://127.0.0.1:5006/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@zentra.local","password":"changeme"}' | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))")

echo "=== Mock Data Audit ==="
curl -s http://127.0.0.1:5006/api/audit/mock-data -H "Authorization: Bearer $TOKEN" | python3 -m json.tool

echo ""
echo "=== Data Source Audit (summary) ==="
curl -s http://127.0.0.1:5006/api/audit/data-sources -H "Authorization: Bearer $TOKEN" | python3 -c "
import sys,json
d = json.load(sys.stdin)
print(f'Total modules: {d[\"total\"]}')
print(f'Live: {d[\"live\"]}')
print(f'Mock: {d[\"mock\"]}')
print(f'Coverage: {d[\"coverage_pct\"]}%')
"
