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
# D08: License expiration check (3-day trial, signed + tamper-resistant)
# After expiry the container still starts but in degraded mode (Dashboard only).
# ---------------------------------------------------------------------------
LICENSE_DAYS=3
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
        echo "[entrypoint] WARNING: Trial license expired ${days_expired} day(s) ago."
        echo "[entrypoint] Running in DEGRADED mode — Dashboard view only, all features disabled."
        echo "[entrypoint] Contact the Zentra team for a production license."
        export ZENTRA_TRIAL_EXPIRED=1
    else
        local days_left=$(( (stored_expiry - current_epoch) / 86400 ))
        echo "[entrypoint] License valid — ${days_left} day(s) remaining."
        if [ "${days_left}" -le 1 ]; then
            echo "[entrypoint] WARNING: License expires in less than 24 hours!"
        fi
        export ZENTRA_TRIAL_EXPIRED=0
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
    export ZENTRA_TRIAL_EXPIRED=0
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
# zentra_app has DML + CREATE (for migrations) but no superuser/DDL-on-system privileges
export DATABASE_URL="${DATABASE_URL:-postgresql://zentra_app:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT}/${POSTGRES_DB}}"

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

    # D04: Create restricted application user (least-privilege)
    echo "[entrypoint] Creating restricted 'zentra_app' database user..."
    su - postgres -c "psql -p ${POSTGRES_PORT} -d ${POSTGRES_DB}" <<EOFSQL
DO \$\$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'zentra_app') THEN
        CREATE USER zentra_app WITH PASSWORD '${POSTGRES_PASSWORD}';
    ELSE
        ALTER USER zentra_app WITH PASSWORD '${POSTGRES_PASSWORD}';
    END IF;
END
\$\$;
GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO zentra_app;
GRANT USAGE ON SCHEMA public TO zentra_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO zentra_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO zentra_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO zentra_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE ON SEQUENCES TO zentra_app;
GRANT CREATE ON SCHEMA public TO zentra_app;
EOFSQL

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
# Ensure zentra user (non-root runtime) has a writable ~/.ssh directory
# ---------------------------------------------------------------------------
if [ ! -d /opt/zentra/.ssh ]; then
    mkdir -p /opt/zentra/.ssh
    chown zentra:zentra /opt/zentra/.ssh
    chmod 700 /opt/zentra/.ssh
    echo "[entrypoint] Created /opt/zentra/.ssh for zentra user"
fi

# ---------------------------------------------------------------------------
# Generate SSH keys for BOTH root (docker exec fallback) and zentra user
# zentra-server runs as the 'zentra' user, so it needs keys under /opt/zentra/.ssh
# ---------------------------------------------------------------------------

# Root keys (used by docker exec and deploy-all.ps1 fallback)
if [ ! -f /root/.ssh/id_rsa ]; then
    echo "[entrypoint] Generating SSH key pair for root..."
    ssh-keygen -t rsa -b 4096 -f /root/.ssh/id_rsa -N "" -C "zentra@$(hostname)" >/dev/null 2>&1
    chmod 600 /root/.ssh/id_rsa
    chmod 644 /root/.ssh/id_rsa.pub
    echo "[entrypoint] Root SSH key pair generated at /root/.ssh/id_rsa"
else
    echo "[entrypoint] Root SSH key pair already exists — skipping generation."
fi

# zentra user keys (used by zentra-server process running as non-root)
if [ ! -f /opt/zentra/.ssh/id_rsa ]; then
    echo "[entrypoint] Generating SSH key pair for zentra user..."
    ssh-keygen -t rsa -b 4096 -f /opt/zentra/.ssh/id_rsa -N "" -C "zentra@$(hostname)" >/dev/null 2>&1
    chown zentra:zentra /opt/zentra/.ssh/id_rsa /opt/zentra/.ssh/id_rsa.pub
    chmod 600 /opt/zentra/.ssh/id_rsa
    chmod 644 /opt/zentra/.ssh/id_rsa.pub
    echo "[entrypoint] zentra user SSH key pair generated at /opt/zentra/.ssh/id_rsa"
else
    echo "[entrypoint] zentra user SSH key pair already exists — skipping generation."
fi

# Ensure SSH config for root
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

# Ensure SSH config for zentra user
if [ ! -f /opt/zentra/.ssh/config ]; then
    cat > /opt/zentra/.ssh/config <<'EOFSSH'
Host *
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
    LogLevel ERROR
    ConnectTimeout 10
EOFSSH
    chown zentra:zentra /opt/zentra/.ssh/config
    chmod 600 /opt/zentra/.ssh/config
fi

# ---------------------------------------------------------------------------
# D09: Self-lockdown — harden the container BEFORE starting services
# After this block: no shells, no editors, no package managers, and all
# config/license files are immutable (even root cannot modify).
# Preserved: sshpass, ssh, scp, curl (needed for monitoring + healthcheck)
# ---------------------------------------------------------------------------
if [ "${ZENTRA_SKIP_LOCKDOWN:-0}" != "1" ]; then
    echo "[entrypoint] Phase: SELF-LOCKDOWN — securing container..."

    # 1. Make license files immutable (even root cannot modify without chattr -i)
    for lf in "${LICENSE_FILE}" "${FALLBACK_LICENSE}"; do
        if [ -f "${lf}" ]; then
            chattr +i "${lf}" 2>/dev/null || true
        fi
    done

    # 2. Make configuration and integrity files immutable
    for cf in \
        /etc/supervisord.conf \
        /etc/nginx/conf.d/zentra.conf \
        /etc/nginx/nginx.conf \
        /opt/zentra/.integrity \
        /opt/zentra/.manifest-hash \
        /opt/zentra/.build-info \
        /usr/local/bin/zentra-server \
        /usr/local/bin/docker-entrypoint.sh \
    ; do
        if [ -f "${cf}" ]; then
            chattr +i "${cf}" 2>/dev/null || true
        fi
    done

    # 3. Make all SQL migrations immutable
    find /opt/zentra/migrations -name '*.sql' -exec chattr +i {} \; 2>/dev/null || true

    # 4. Make frontend dist immutable
    find /opt/zentra/frontend/dist -type f -exec chattr +i {} \; 2>/dev/null || true

    # 5. Remove package managers (prevent installing tools at runtime)
    rm -f /usr/bin/dnf /usr/bin/dnf-3 /usr/bin/yum \
          /usr/bin/yum-config-manager /usr/bin/rpm \
          /usr/bin/rpm2cpio /usr/bin/rpmkeys 2>/dev/null || true
    rm -rf /etc/yum.repos.d /var/cache/dnf 2>/dev/null || true

    # 6. Remove editors and scripting languages
    #    NOTE: python3 is kept because supervisord depends on it.
    #    We remove perl/lua/ruby and all editors.
    rm -f /usr/bin/vi /usr/bin/vim /usr/bin/nano /usr/bin/ed \
          /usr/bin/perl /usr/bin/lua /usr/bin/ruby 2>/dev/null || true

    # 7. Remove dangerous file-manipulation utilities
    rm -f /usr/bin/wget /usr/bin/ftp /usr/bin/telnet \
          /usr/bin/gcc /usr/bin/cc /usr/bin/make \
          /usr/bin/gdb /usr/bin/strace /usr/bin/ltrace \
          /usr/bin/dd /usr/bin/strings 2>/dev/null || true

    # 8. Remove chattr itself (so immutable bits can NEVER be reversed)
    rm -f /usr/bin/chattr /usr/bin/lsattr 2>/dev/null || true

    echo "[entrypoint] Config and license files locked (immutable)."
    echo "[entrypoint] Package managers, editors, scripting languages removed."

    # 9. Remove all shells AFTER exec hands off to supervisord.
    #    We cannot remove bash while we're running in it, so we spawn a
    #    background task that waits for supervisord to start, then removes
    #    all shell binaries. After this, 'docker exec' becomes useless.
    (
        sleep 5
        rm -f /bin/bash /bin/sh /usr/bin/bash /usr/bin/sh \
              /usr/bin/csh /usr/bin/tcsh /usr/bin/zsh \
              /usr/bin/ksh /usr/bin/fish /usr/bin/dash 2>/dev/null
        # Remove the cleanup script itself
        rm -f /opt/zentra/cleanup-seeds.sh 2>/dev/null
        echo "[lockdown] All shells removed. Container is now locked down."
    ) &

    echo "[entrypoint] LOCKDOWN COMPLETE — container will be fully sealed in 5s."
fi

echo "[entrypoint] Starting all services via supervisord..."
exec "$@"
