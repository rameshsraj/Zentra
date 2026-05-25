#!/usr/bin/env bash
# =============================================================================
# Zentra - Docker Entrypoint
# Initializes PostgreSQL + TimescaleDB, then starts supervisord
# =============================================================================
set -e

# ---------------------------------------------------------------------------
# License expiration check (5-day trial)
# ---------------------------------------------------------------------------
INSTALL_DATE_FILE="/opt/zentra/.install_date"
LICENSE_DAYS=5

if [ ! -f "${INSTALL_DATE_FILE}" ]; then
    # First run - record installation date
    date +%s > "${INSTALL_DATE_FILE}"
else
    # Check if license has expired
    INSTALL_EPOCH=$(cat "${INSTALL_DATE_FILE}")
    CURRENT_EPOCH=$(date +%s)
    DAYS_SINCE_INSTALL=$(( (CURRENT_EPOCH - INSTALL_EPOCH) / 86400 ))

    if [ "${DAYS_SINCE_INSTALL}" -ge "${LICENSE_DAYS}" ]; then
        echo "[entrypoint] ERROR: Image corrupted. Please reinstall."
        exit 1
    fi
fi

# ---------------------------------------------------------------------------
# Default environment variables
# ---------------------------------------------------------------------------
export POSTGRES_USER="${POSTGRES_USER:-postgres}"
export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-$(head -c 16 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 24)}"
export POSTGRES_DB="${POSTGRES_DB:-zentra}"
export POSTGRES_PORT="${POSTGRES_PORT:-5434}"
export DATABASE_URL="${DATABASE_URL:-postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}}"
export ZENTRA_API_PORT="${ZENTRA_API_PORT:-5006}"
export ZENTRA_JWT_SECRET="${ZENTRA_JWT_SECRET:-}"
export ZENTRA_ALLOW_INSECURE_JWT="${ZENTRA_ALLOW_INSECURE_JWT:-1}"

# Auto-generate ZENTRA_MASTER_KEY if not provided (required for credential encryption)
if [ -z "${ZENTRA_MASTER_KEY:-}" ]; then
    export ZENTRA_MASTER_KEY=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' | head -c 64)
    echo "[entrypoint] ZENTRA_MASTER_KEY not set - auto-generated for this session."
    echo "[entrypoint] WARNING: Set ZENTRA_MASTER_KEY explicitly for production use."
fi
export ZENTRA_ALLOW_ORIGINS="${ZENTRA_ALLOW_ORIGINS:-*}"
export ZENTRA_ORG_ID="${ZENTRA_ORG_ID:-00000000-0000-0000-0000-000000000001}"

PGDATA="/var/lib/postgresql/data"

echo "=============================================="
echo "  Zentra - Starting up"
echo "  PostgreSQL: ${POSTGRES_USER}@localhost:${POSTGRES_PORT}/${POSTGRES_DB}"
echo "  API Port:   ${ZENTRA_API_PORT}"
echo "=============================================="

# ---------------------------------------------------------------------------
# Initialize PostgreSQL if data directory is empty (first run)
# ---------------------------------------------------------------------------
if [ ! -f "${PGDATA}/PG_VERSION" ]; then
    echo "[entrypoint] Initializing PostgreSQL ${PG_MAJOR} data directory..."
    su - postgres -c "/usr/pgsql-16/bin/initdb -D ${PGDATA} --encoding=UTF8 --locale=en_US.UTF-8"

    # Configure PostgreSQL to listen on the configured port
    sed -i "s/^#port = 5432/port = ${POSTGRES_PORT}/" "${PGDATA}/postgresql.conf"
    sed -i "s/^port = 5432/port = ${POSTGRES_PORT}/" "${PGDATA}/postgresql.conf"

    # Allow local connections with password
    cat > "${PGDATA}/pg_hba.conf" <<EOFHBA
# TYPE  DATABASE  USER      ADDRESS     METHOD
local   all       all                   trust
host    all       all       127.0.0.1/32 md5
host    all       all       ::1/128      md5
host    all       all       0.0.0.0/0    md5
EOFHBA

    # Enable TimescaleDB in shared_preload_libraries
    echo "shared_preload_libraries = 'timescaledb'" >> "${PGDATA}/postgresql.conf"
    echo "timescaledb.telemetry_level = off" >> "${PGDATA}/postgresql.conf"

    # Performance tuning for monitoring workload
    cat >> "${PGDATA}/postgresql.conf" <<EOFCONF
# Zentra performance tuning
max_connections = 200
shared_buffers = 256MB
effective_cache_size = 512MB
work_mem = 4MB
maintenance_work_mem = 64MB
random_page_cost = 1.1
log_timezone = 'UTC'
timezone = 'UTC'
EOFCONF

    echo "[entrypoint] Starting PostgreSQL for initial setup..."
    su - postgres -c "/usr/pgsql-16/bin/pg_ctl -D ${PGDATA} -l /var/log/postgresql/init.log start -w -t 30"

    # Create database and user
    echo "[entrypoint] Creating database '${POSTGRES_DB}'..."
    su - postgres -c "psql -p ${POSTGRES_PORT} -c \"ALTER USER ${POSTGRES_USER} PASSWORD '${POSTGRES_PASSWORD}';\"" || true
    su - postgres -c "psql -p ${POSTGRES_PORT} -c \"SELECT 1 FROM pg_database WHERE datname='${POSTGRES_DB}'\" | grep -q 1" || \
        su - postgres -c "psql -p ${POSTGRES_PORT} -c \"CREATE DATABASE ${POSTGRES_DB} OWNER ${POSTGRES_USER};\""

    # Enable TimescaleDB extension
    echo "[entrypoint] Enabling TimescaleDB extension..."
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB} -c 'CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;'"
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB} -c 'CREATE EXTENSION IF NOT EXISTS pg_trgm;'" || true
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB} -c 'CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\";'" || true

    echo "[entrypoint] Stopping PostgreSQL after init..."
    su - postgres -c "/usr/pgsql-16/bin/pg_ctl -D ${PGDATA} stop -m fast -w"

    echo "[entrypoint] PostgreSQL initialization complete."
else
    echo "[entrypoint] PostgreSQL data directory exists - skipping init."

    # Ensure port is correct in existing config
    sed -i "s/^port = [0-9]*/port = ${POSTGRES_PORT}/" "${PGDATA}/postgresql.conf"
fi

# Ensure correct ownership (may be needed if volume was mounted with wrong perms)
chown -R postgres:postgres "${PGDATA}" /var/run/postgresql /var/log/postgresql

# ---------------------------------------------------------------------------
# Post-migration cleanup: remove seeded demo/target data
# Runs in background AFTER the API starts (migrations happen at API startup)
# ---------------------------------------------------------------------------
if [ "${ZENTRA_CLEAN_SEED_DATA:-1}" = "1" ]; then
    cat > /opt/zentra/cleanup-seeds.sh <<'EOFSCRIPT'
#!/bin/bash
# Wait for API to be healthy (migrations complete)
echo "[cleanup] Waiting for API to be ready..."
for i in $(seq 1 60); do
    if curl -sf http://localhost:5006/api/health >/dev/null 2>&1; then
        echo "[cleanup] API healthy after ${i}s - cleaning seed data..."
        su - postgres -c "psql -p ${POSTGRES_PORT:-5434} -d ${POSTGRES_DB:-zentra} -c \"DELETE FROM targets WHERE target_id IN ('00000000-0000-0000-0000-000000000201','00000000-0000-0000-0000-000000000202');\""
        echo "[cleanup] Seed data cleaned. Only auth rows remain."
        touch /opt/zentra/.seeds-cleaned
        exit 0
    fi
    sleep 1
done
echo "[cleanup] API did not become healthy in 60s - skipping cleanup"
EOFSCRIPT
    chmod +x /opt/zentra/cleanup-seeds.sh
    # Only run cleanup on first boot (before .seeds-cleaned marker exists)
    if [ ! -f /opt/zentra/.seeds-cleaned ]; then
        /opt/zentra/cleanup-seeds.sh &
    fi
fi

# ---------------------------------------------------------------------------
# Generate SSH keys if not present (for monitoring remote servers)
# Keys persist across restarts via the /root/.ssh directory
# ---------------------------------------------------------------------------
if [ ! -f /root/.ssh/id_rsa ]; then
    echo "[entrypoint] Generating SSH key pair for server monitoring..."
    ssh-keygen -t rsa -b 4096 -f /root/.ssh/id_rsa -N "" -C "zentra@$(hostname)" >/dev/null 2>&1
    chmod 600 /root/.ssh/id_rsa
    chmod 644 /root/.ssh/id_rsa.pub
    echo "[entrypoint] SSH key pair generated at /root/.ssh/id_rsa"
else
    echo "[entrypoint] SSH key pair already exists - skipping generation."
fi

# Ensure SSH config allows key-based auth without strict host checking for monitoring
if [ ! -f /root/.ssh/config ]; then
    cat > /root/.ssh/config <<'EOFSSH'
Host *
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
    LogLevel ERROR
    ConnectTimeout 10
EOFSSH
    chmod 600 /root/.ssh/config
fi

echo "[entrypoint] Starting all services via supervisord..."
exec "$@"
