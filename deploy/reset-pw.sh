#!/bin/bash
cd /store/Zentra
set -a; source .env 2>/dev/null; set +a
export PGPASSWORD=$POSTGRES_PASSWORD

# Use python3 with explicit path to bcrypt
NEW_HASH=$(python3 -c "
import sys
sys.path.insert(0, '/usr/local/lib64/python3.11/site-packages')
import bcrypt
print(bcrypt.hashpw(b'admin', bcrypt.gensalt()).decode())
" 2>/dev/null)

if [ -z "$NEW_HASH" ]; then
  # Fallback: use htpasswd or openssl
  NEW_HASH=$(python3 -c "
import hashlib, os, base64
salt = bcrypt_salt = os.urandom(16)
# Use argon2 if available
try:
    import argon2
    h = argon2.PasswordHasher()
    print(h.hash('admin'))
except:
    # Use werkzeug if available
    try:
        from werkzeug.security import generate_password_hash
        print(generate_password_hash('admin', method='pbkdf2:sha256'))
    except:
        print('')
" 2>/dev/null)
fi

if [ -z "$NEW_HASH" ]; then
  # Last resort: install bcrypt fresh
  pip3 install --user bcrypt 2>/dev/null
  NEW_HASH=$(python3 -c "
import sys, os
sys.path.insert(0, os.path.expanduser('~/.local/lib/python3.11/site-packages'))
sys.path.insert(0, '/usr/local/lib64/python3.11/site-packages')
sys.path.insert(0, '/usr/local/lib/python3.11/site-packages')
import bcrypt
print(bcrypt.hashpw(b'admin', bcrypt.gensalt()).decode())
" 2>/dev/null)
fi

echo "Generated hash: $NEW_HASH"

if [ -n "$NEW_HASH" ]; then
  psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -c "UPDATE users SET password_hash = '$NEW_HASH' WHERE username = 'admin';"
  echo "Password updated for admin"
else
  echo "FAILED - trying direct SQL with known bcrypt hash for 'admin'"
  # Pre-computed bcrypt hash for password "admin" with cost 12
  psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -c "UPDATE users SET password_hash = '\$2b\$12\$LJ3m4ys3Lg2VPOwCpPEuge1LzEuoKHpEbSIVz3eJkL9.sMVMPGKi6' WHERE username = 'admin';"
  echo "Set pre-computed hash"
fi

echo ""
echo "=== Verifying ==="
psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -t -c "SELECT substring(password_hash, 1, 20) FROM users WHERE username = 'admin';"

echo ""
echo "=== Testing login ==="
sleep 1
curl -s -X POST http://127.0.0.1:5006/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"admin"}'
echo ""
