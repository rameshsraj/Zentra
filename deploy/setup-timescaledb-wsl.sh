#!/usr/bin/env bash
# Installs and enables TimescaleDB in the WSL Ubuntu PostgreSQL instance.
# Run once: wsl -d Ubuntu -u root -- bash /path/to/setup-timescaledb-wsl.sh
set -euo pipefail

PG_VERSION=$(pg_lsclusters -h | awk 'NR==1{print $1}')
echo "PostgreSQL version: $PG_VERSION"

# Install package
apt-get install -y "postgresql-${PG_VERSION}-timescaledb" 2>&1 | tail -5

# Add to shared_preload_libraries (idempotent)
CONF="/etc/postgresql/${PG_VERSION}/main/postgresql.conf"
if grep -q "shared_preload_libraries = 'timescaledb'" "$CONF" 2>/dev/null; then
    echo "TimescaleDB already in shared_preload_libraries"
else
    # Comment out any existing shared_preload_libraries line, then append
    sed -i "s/^shared_preload_libraries/#shared_preload_libraries/" "$CONF"
    echo "shared_preload_libraries = 'timescaledb'" >> "$CONF"
    echo "Added shared_preload_libraries = 'timescaledb' to $CONF"
fi

# Restart postgres
pg_ctlcluster "$PG_VERSION" main restart
sleep 3
pg_isready -h 127.0.0.1 -p 5434 && echo "PostgreSQL: ready"

# Enable extension in zentra DB
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d zentra \
    -c "CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;" 2>&1
echo "TimescaleDB extension enabled in 'zentra' database."
