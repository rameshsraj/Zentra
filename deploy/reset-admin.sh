#!/bin/bash
# Reset admin password to admin123 and run tests
# Pre-computed bcrypt hash for "admin123"
HASH='$2b$12$BZLC/mivy0lQsTTYxYtUfOFTkBRVWvZJj31XJKvslHNSUft3KkCie'
echo "UPDATE users SET password_hash='$HASH', is_locked=false, locked_until=NULL WHERE email='admin@smarsh-nexus.com';" | PGPASSWORD="${PGPASSWORD:?Error: PGPASSWORD not set}" psql -h 127.0.0.1 -p 5434 -U postgres -d zentra
echo "Password reset done"

# Now run test
bash /tmp/test-oracle.sh
