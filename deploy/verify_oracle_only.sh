#!/bin/bash
BASE="http://127.0.0.1:5006/api"
LOGIN=$(curl -sf -X POST "$BASE/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@Zentra.local","password":"changeme"}')
TOKEN=$(echo "$LOGIN" | grep -o '"token":"[^"]*"' | head -1 | cut -d'"' -f4)

echo "=== Database targets ==="
curl -sf -H "Authorization: Bearer $TOKEN" "$BASE/targets" | python3 -c "
import sys, json
targets = json.load(sys.stdin)
db = [t for t in targets if t.get('target_type') == 'database']
for t in db:
    print(f\"  {t['name']} | {t.get('target_subtype','?')} | {t.get('host','?')}:{t.get('port','?')} | {t.get('status','?')}\")
print(f'Total DB targets: {len(db)}')
non_oracle = [t for t in db if t.get('target_subtype','') != 'oracle']
if non_oracle:
    print(f'WARNING: {len(non_oracle)} non-Oracle DB targets still exist!')
    for t in non_oracle:
        print(f'  -> {t[\"name\"]} ({t.get(\"target_subtype\",\"?\")})')
else:
    print('OK: All database targets are Oracle only')
"

echo ""
echo "=== Frontend serving ==="
curl -sf -o /dev/null -w 'HTTP %{http_code}' http://127.0.0.1:5010/
echo ""
echo "=== API health ==="
curl -sf http://127.0.0.1:5006/api/health
echo ""
