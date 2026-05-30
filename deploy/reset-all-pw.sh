#!/bin/bash
cd /store/Zentra
set -a; source .env 2>/dev/null; set +a
export PGPASSWORD=$POSTGRES_PASSWORD

# Generate bcrypt hash for 'changeme'
NEW_HASH=$(python3 -c "
import sys
sys.path.insert(0, '/usr/local/lib64/python3.11/site-packages')
sys.path.insert(0, '/usr/local/lib/python3.11/site-packages')
import bcrypt
print(bcrypt.hashpw(b'changeme', bcrypt.gensalt()).decode())
" 2>/dev/null)

if [ -z "$NEW_HASH" ]; then
  pip3 install --user bcrypt 2>/dev/null
  NEW_HASH=$(python3 -c "
import sys, os
for p in ['/usr/local/lib64/python3.11/site-packages', '/usr/local/lib/python3.11/site-packages', os.path.expanduser('~/.local/lib/python3.11/site-packages')]:
    sys.path.insert(0, p)
import bcrypt
print(bcrypt.hashpw(b'changeme', bcrypt.gensalt()).decode())
" 2>/dev/null)
fi

echo "Hash: $NEW_HASH"

if [ -n "$NEW_HASH" ]; then
  # Make email lookup case-insensitive by lowering all emails
  psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -c "UPDATE users SET email = LOWER(email);"
  # Reset all passwords to 'changeme'
  psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -c "UPDATE users SET password_hash = '$NEW_HASH';"
  echo "All passwords reset to 'changeme'"
fi

echo ""
echo "=== Users ==="
psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -t -c "SELECT email, username FROM users;"

echo ""
echo "=== Test login ==="
curl -s -X POST http://127.0.0.1:5006/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"changeme"}' | python3 -c "import sys,json; d=json.load(sys.stdin); print('LOGIN OK' if d.get('token') else 'FAIL: '+str(d))" 2>/dev/null
echo ""
