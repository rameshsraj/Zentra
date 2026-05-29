#!/usr/bin/env bash
# =============================================================================
# Zentra - Docker Entrypoint (Hardened)
# Tamper verification → License check → PostgreSQL init → supervisord
# =============================================================================
set -e

# ---------------------------------------------------------------------------
# D07: Tamper protection — verify integrity of critical files
# ---------------------------------------------------------------------------
INTEGRITY_FILE="/opt/zentra/.integrity"
MANIFEST_HASH_FILE="/opt/zentra/.manifest-hash"

if [ -f "${MANIFEST_HASH_FILE}" ] && [ -f "${INTEGRITY_FILE}" ]; then
    EXPECTED_HASH=$(cat "${MANIFEST_HASH_FILE}")
    ACTUAL_HASH=$(sha256sum "${INTEGRITY_FILE}" | cut -d' ' -f1)
    if [ "${EXPECTED_HASH}" != "${ACTUAL_HASH}" ]; then
        echo "[entrypoint] FATAL: Integrity manifest has been modified."
        echo "[entrypoint] Application cannot start. Please use an official image."
        exit 1
    fi
    if ! sha256sum -c "${INTEGRITY_FILE}" --quiet 2>/dev/null; then
        echo "[entrypoint] FATAL: Integrity verification failed — files may be tampered."
        echo "[entrypoint] Application cannot start. Please use an official image."
        exit 1
    fi
    echo "[entrypoint] Integrity verification PASSED."
elif [ -f "${INTEGRITY_FILE}" ]; then
    if ! sha256sum -c "${INTEGRITY_FILE}" --quiet 2>/dev/null; then
        echo "[entrypoint] FATAL: Integrity verification failed."
        exit 1
    fi
fi

# ---------------------------------------------------------------------------
# D08: License expiration check (5-day trial, signed + tamper-resistant)
# ---------------------------------------------------------------------------
LICENSE_DAYS=5
LICENSE_FILE="/var/lib/postgresql/data/.zentra-license"
BUILD_HASH=$(sha256sum /usr/local/bin/zentra-server | cut -d' ' -f1)

generate_license() {
    local install_epoch=$(date +%s)
    local expiry_epoch=$((install_epoch + LICENSE_DAYS * 86400))
    local hostname_val=$(hostname)
    local payload="${install_epoch}:${expiry_epoch}:${hostname_val}"
    local sig=$(echo -n "${payload}:${BUILD_HASH}" | sha256sum | cut -d' ' -f1)
    echo "${payload}:${sig}"
}

verify_license() {
    local license_data="$1"
    local stored_install=$(echo "$license_data" | cut -d: -f1)
    local stored_expiry=$(echo "$license_data" | cut -d: -f2)
    local stored_host=$(echo "$license_data" | cut -d: -f3)
    local stored_sig=$(echo "$license_data" | cut -d: -f4)

    # Verify signature
    local expected_sig=$(echo -n "${stored_install}:${stored_expiry}:${stored_host}:${BUILD_HASH}" | sha256sum | cut -d' ' -f1)
    if [ "${stored_sig}" != "${expected_sig}" ]; then
        echo "[entrypoint] ERROR: License file corrupted or tampered. Please reinstall."
        exit 1
    fi

    # Check expiry
    local current_epoch=$(date +%s)
    if [ "${current_epoch}" -ge "${stored_expiry}" ]; then
        local days_expired=$(( (current_epoch - stored_expiry) / 86400 ))
        echo "[entrypoint] ERROR: License expired ${days_expired} day(s) ago."
        echo "[entrypoint] Contact the Zentra team for a new license or image."
        exit 1
    fi

    local days_left=$(( (stored_expiry - current_epoch) / 86400 ))
    echo "[entrypoint] License valid — ${days_left} day(s) remaining."
    if [ "${days_left}" -le 1 ]; then
        echo "[entrypoint] WARNING: License expires in less than 24 hours!"
    fi
}

# The license file is stored inside the PG data volume so it survives
# container recreation but cannot be bypassed by deleting overlay layers.
# On first boot (before PG init), fall back to /opt/zentra/.license
FALLBACK_LICENSE="/opt/zentra/.license"

if [ -f "${LICENSE_FILE}" ]; then
    verify_license "$(cat "${LICENSE_FILE}")"
elif [ -f "${FALLBACK_LICENSE}" ]; then
    verify_license "$(cat "${FALLBACK_LICENSE}")"
else
    # First run — generate license token
    LICENSE_TOKEN=$(generate_license)
    echo "${LICENSE_TOKEN}" > "${FALLBACK_LICENSE}"
    chmod 400 "${FALLBACK_LICENSE}"
    echo "[entrypoint] License activated — ${LICENSE_DAYS}-day trial started."
fi

# ---------------------------------------------------------------------------
# Default environment variables
# ---------------------------------------------------------------------------
export POSTGRES_USER="${POSTGRES_USER:-postgres}"
export POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-$(head -c 16 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 24)}"
export POSTGRES_DB="${POSTGRES_DB:-zentra}"
export POSTGRES_PORT="${POSTGRES_PORT:-5434}"
export ZENTRA_API_PORT="${ZENTRA_API_PORT:-5006}"

# D02: Auto-generate JWT secret if not provided
if [ -z "${ZENTRA_JWT_SECRET:-}" ]; then
    export ZENTRA_JWT_SECRET=$(head -c 32 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 48)
    echo "[entrypoint] ZENTRA_JWT_SECRET not set — auto-generated for this session."
fi
export ZENTRA_ALLOW_INSECURE_JWT="${ZENTRA_ALLOW_INSECURE_JWT:-1}"

# Auto-generate ZENTRA_MASTER_KEY if not provided (required for credential encryption)
if [ -z "${ZENTRA_MASTER_KEY:-}" ]; then
    export ZENTRA_MASTER_KEY=$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' | head -c 64)
    echo "[entrypoint] ZENTRA_MASTER_KEY not set — auto-generated for this session."
fi
export ZENTRA_ALLOW_ORIGINS="${ZENTRA_ALLOW_ORIGINS:-*}"
export ZENTRA_ORG_ID="${ZENTRA_ORG_ID:-00000000-0000-0000-0000-000000000001}"

# D05: Force API server to bind to localhost only (not externally reachable)
export ZENTRA_BIND_ADDR="${ZENTRA_BIND_ADDR:-127.0.0.1}"

# D04: Use restricted app user for DATABASE_URL (not superuser)
export DATABASE_URL="${DATABASE_URL:-postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT}/${POSTGRES_DB}}"

PGDATA="/var/lib/postgresql/data"

echo "=============================================="
echo "  Zentra — Starting up"
echo "  PostgreSQL: ${POSTGRES_USER}@127.0.0.1:${POSTGRES_PORT}/${POSTGRES_DB}"
echo "  API Port:   ${ZENTRA_BIND_ADDR}:${ZENTRA_API_PORT}"
echo "=============================================="

# ---------------------------------------------------------------------------
# Initialize PostgreSQL if data directory is empty (first run)
# ---------------------------------------------------------------------------
if [ ! -f "${PGDATA}/PG_VERSION" ]; then
    echo "[entrypoint] Initializing PostgreSQL ${PG_MAJOR} data directory..."
    su - postgres -c "/usr/pgsql-16/bin/initdb -D ${PGDATA} --encoding=UTF8 --locale=en_US.UTF-8"

    # Configure PostgreSQL to listen on localhost only (D05)
    sed -i "s/^#port = 5432/port = ${POSTGRES_PORT}/" "${PGDATA}/postgresql.conf"
    sed -i "s/^port = 5432/port = ${POSTGRES_PORT}/" "${PGDATA}/postgresql.conf"
    sed -i "s/^#listen_addresses = 'localhost'/listen_addresses = '127.0.0.1'/" "${PGDATA}/postgresql.conf"

    # Allow local connections only — no external DB access (D04, D05)
    cat > "${PGDATA}/pg_hba.conf" <<EOFHBA
# TYPE  DATABASE  USER      ADDRESS       METHOD
# No external DB access — PostgreSQL is internal-only
local   all       all                     trust
host    all       all       127.0.0.1/32  md5
host    all       all       ::1/128       md5
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

    # Enable extensions
    echo "[entrypoint] Enabling TimescaleDB extension..."
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB} -c 'CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;'"
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB} -c 'CREATE EXTENSION IF NOT EXISTS pg_trgm;'" || true
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB} -c 'CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\";'" || true

    echo "[entrypoint] Stopping PostgreSQL after init..."
    su - postgres -c "/usr/pgsql-16/bin/pg_ctl -D ${PGDATA} stop -m fast -w"

    echo "[entrypoint] PostgreSQL initialization complete."
else
    echo "[entrypoint] PostgreSQL data directory exists — skipping init."

    # Ensure port and listen address are correct in existing config
    sed -i "s/^port = [0-9]*/port = ${POSTGRES_PORT}/" "${PGDATA}/postgresql.conf"
fi

# Ensure correct ownership (may be needed if volume was mounted with wrong perms)
chown -R postgres:postgres "${PGDATA}" /var/run/postgresql /var/log/postgresql

# D08: Copy license to PG volume (persists across container recreations)
if [ -f "${FALLBACK_LICENSE}" ] && [ ! -f "${LICENSE_FILE}" ]; then
    cp "${FALLBACK_LICENSE}" "${LICENSE_FILE}"
    chmod 400 "${LICENSE_FILE}"
    chown postgres:postgres "${LICENSE_FILE}"
    echo "[entrypoint] License persisted to data volume."
fi

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
        echo "[cleanup] API healthy after ${i}s — cleaning seed data..."
        su - postgres -c "psql -p ${POSTGRES_PORT:-5434} -d ${POSTGRES_DB:-zentra} -c \"DELETE FROM targets WHERE target_id IN ('00000000-0000-0000-0000-000000000201','00000000-0000-0000-0000-000000000202');\""
        echo "[cleanup] Seed data cleaned. Only auth rows remain."
        touch /opt/zentra/.seeds-cleaned
        exit 0
    fi
    sleep 1
done
echo "[cleanup] API did not become healthy in 60s — skipping cleanup"
EOFSCRIPT
    chmod +x /opt/zentra/cleanup-seeds.sh
    # Only run cleanup on first boot (before .seeds-cleaned marker exists)
    if [ ! -f /opt/zentra/.seeds-cleaned ]; then
        /opt/zentra/cleanup-seeds.sh &
    fi
fi

# ---------------------------------------------------------------------------
# Generate SSH keys if not present (for monitoring remote servers)
# ---------------------------------------------------------------------------
if [ ! -f /root/.ssh/id_rsa ]; then
    echo "[entrypoint] Generating SSH key pair for server monitoring..."
    ssh-keygen -t rsa -b 4096 -f /root/.ssh/id_rsa -N "" -C "zentra@$(hostname)" >/dev/null 2>&1
    chmod 600 /root/.ssh/id_rsa
    chmod 644 /root/.ssh/id_rsa.pub
    echo "[entrypoint] SSH key pair generated at /root/.ssh/id_rsa"
else
    echo "[entrypoint] SSH key pair already exists — skipping generation."
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
