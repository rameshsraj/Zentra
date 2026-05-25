#!/bin/bash
API=http://127.0.0.1:5006
CT="Content-Type: application/json"

# Step 1: Login (login.json must already be at /tmp/login.json)
RESP=$(curl -sf -X POST $API/api/auth/login -H "$CT" -d @/tmp/login.json)
TOKEN=$(echo "$RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))")
echo "TOKEN=${TOKEN:0:20}..."
if [ -z "$TOKEN" ]; then echo "LOGIN FAILED: $RESP"; exit 1; fi

# Step 2: Create XEPDB1 connection (oraconn.json must already be at /tmp/oraconn.json)
CONN=$(curl -sf -X POST $API/api/oracle/connections -H "$CT" -H "Authorization: Bearer $TOKEN" -d @/tmp/oraconn.json)
echo "CONN=$CONN"
CID=$(echo "$CONN" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))")
echo "CID=$CID"
if [ -z "$CID" ]; then echo "CREATE FAILED"; exit 1; fi

# Step 3: Test connection
echo ""
echo "=== Test Connection ==="
curl -sf -X POST "$API/api/oracle/connections/$CID/test" -H "Authorization: Bearer $TOKEN" | python3 -m json.tool

# Step 4: Test all data endpoints
QS="connection_id=$CID"
for EP in overview tablespaces wait-classes top-queries running-queries cache-hit-ratio pdb sysmetrics sysstat dataguard rman listeners rac performance-history; do
  echo ""
  echo "=== $EP ==="
  curl -sf "$API/api/oracle/$EP?$QS" -H "Authorization: Bearer $TOKEN" 2>/dev/null | head -c 500
  echo ""
done

echo ""
echo "=== ALL TESTS COMPLETE ==="
