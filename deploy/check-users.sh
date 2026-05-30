#!/bin/bash
cd /store/Zentra
set -a; source .env 2>/dev/null; set +a
export PGPASSWORD=$POSTGRES_PASSWORD
psql -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d zentra -t -c "SELECT email, username FROM users LIMIT 10;"
