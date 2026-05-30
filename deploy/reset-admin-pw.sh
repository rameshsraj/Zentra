#!/bin/bash
cd /store/Zentra
set -a; source .env 2>/dev/null; set +a
export PGPASSWORD=$POSTGRES_PASSWORD

echo "=== Current admin password hash ==="
psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -t -c "SELECT password_hash FROM users WHERE username = 'admin' LIMIT 1;"

echo ""
echo "=== Checking hash format ==="
HASH=$(psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -t -c "SELECT password_hash FROM users WHERE username = 'admin' LIMIT 1;" | tr -d ' ')
echo "Hash starts with: ${HASH:0:10}"

echo ""
echo "=== Resetting admin password to 'admin' using bcrypt ==="
# Generate bcrypt hash for 'admin' using python3
NEW_HASH=$(python3 -c "
import hashlib, base64, os
# Try bcrypt first
try:
    import bcrypt
    h = bcrypt.hashpw(b'admin', bcrypt.gensalt()).decode()
    print(h)
except ImportError:
    # Fallback: use argon2 or simple hash
    try:
        from passlib.hash import bcrypt as pb
        print(pb.hash('admin'))
    except ImportError:
        # Last resort: check what format the existing hash uses
        print('NO_BCRYPT_AVAILABLE')
")

echo "New hash: $NEW_HASH"

if [ "$NEW_HASH" = "NO_BCRYPT_AVAILABLE" ]; then
    echo "Installing bcrypt..."
    pip3 install bcrypt 2>/dev/null || pip install bcrypt 2>/dev/null
    NEW_HASH=$(python3 -c "import bcrypt; print(bcrypt.hashpw(b'admin', bcrypt.gensalt()).decode())")
    echo "New hash after install: $NEW_HASH"
fi

if [ -n "$NEW_HASH" ] && [ "$NEW_HASH" != "NO_BCRYPT_AVAILABLE" ]; then
    psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -c "UPDATE users SET password_hash = '$NEW_HASH' WHERE username = 'admin';"
    echo "Admin password reset to 'admin'"
else
    echo "FAILED to generate hash"
fi

echo ""
echo "=== Testing login ==="
curl -s -X POST http://127.0.0.1:5006/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"admin"}'
echo ""
