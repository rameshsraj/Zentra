#!/usr/bin/env bash
set -euo pipefail

DEPLOY_PATH="/store/Zentra"
PGDATA_PATH="/store/pgdata"
BACKEND_PORT="5006"
FRONTEND_PORT="5010"
POSTGRES_PORT="5434"
POSTGRES_USER="postgres"
POSTGRES_PASSWORD="zantaz"
POSTGRES_DB="zentra"
APP_USER="zentra"
APP_PASSWORD="zentra"
RUNTIME_DB_URL="postgresql://${POSTGRES_USER}:${POSTGRES_PASSWORD}@127.0.0.1:${POSTGRES_PORT}/${POSTGRES_DB}"

EXTERNAL_IP="$(curl -sf --max-time 5 \
    -H "Metadata-Flavor: Google" \
    "http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip" \
    2>/dev/null || true)"
if [[ -z "${EXTERNAL_IP}" ]]; then
    EXTERNAL_IP="$(curl -sf --max-time 5 https://ifconfig.me 2>/dev/null || true)"
fi
if [[ -z "${EXTERNAL_IP}" ]]; then
    EXTERNAL_IP="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
fi
ALLOW_ORIGIN=""
if [[ -n "${EXTERNAL_IP}" ]]; then
    ALLOW_ORIGIN="http://${EXTERNAL_IP}:${FRONTEND_PORT}"
fi

if [[ "${EUID}" -ne 0 ]]; then
    echo "This script must be run as root (or via sudo)." >&2
    exit 1
fi

echo "========== Zentra Full Server Setup =========="

export DEBIAN_FRONTEND=noninteractive

wait_for_apt_locks() {
    local tries=120
    local delay=5
    while (( tries > 0 )); do
        if sudo fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1 \
            || sudo fuser /var/lib/dpkg/lock >/dev/null 2>&1 \
            || sudo fuser /var/lib/apt/lists/lock >/dev/null 2>&1; then
            echo "apt/dpkg lock detected, waiting... (${tries} tries left)"
            sleep "${delay}"
            tries=$((tries - 1))
            continue
        fi
        return 0
    done

    echo "Timed out waiting for apt/dpkg lock release" >&2
    return 1
}

ensure_timescaledb_installed() {
    # Detect the installed PostgreSQL major version
    local pg_ver
    pg_ver="$(psql --version 2>/dev/null | grep -oP '(?<=PostgreSQL )\d+' | head -n1 || true)"
    if [[ -z "${pg_ver}" ]]; then
        pg_ver="$(ls /etc/postgresql 2>/dev/null | sort -V | tail -n1 || true)"
    fi
    if [[ -z "${pg_ver}" ]]; then
        echo "Cannot detect PostgreSQL version for TimescaleDB install" >&2
        exit 1
    fi
    echo "Detected PostgreSQL version: ${pg_ver}"

    if [[ -f "/usr/share/postgresql/${pg_ver}/extension/timescaledb.control" ]]; then
        echo "TimescaleDB already installed for PostgreSQL ${pg_ver}."
        return 0
    fi

    echo "Installing TimescaleDB extension for PostgreSQL ${pg_ver}..."
    if ! grep -Rqs "packagecloud.io/timescale/timescaledb" /etc/apt/sources.list /etc/apt/sources.list.d 2>/dev/null; then
        curl -fsSL https://packagecloud.io/install/repositories/timescale/timescaledb/script.deb.sh | bash
    fi

    wait_for_apt_locks
    apt-get update -y
    wait_for_apt_locks
    if ! apt-get install -y "timescaledb-2-postgresql-${pg_ver}" timescaledb-tools; then
        echo "timescaledb-2-postgresql-${pg_ver} failed; trying postgresql-${pg_ver}-timescaledb fallback..."
        wait_for_apt_locks
        apt-get install -y "postgresql-${pg_ver}-timescaledb" || {
            echo "TimescaleDB package install failed. Ensure the OS is supported by the TimescaleDB repository." >&2
            exit 1
        }
    fi
}

ensure_store_path_ready() {
    mkdir -p /store

    if grep -Eq '^[[:space:]]*LABEL=Zentra[[:space:]]+/store[[:space:]]+ext4' /etc/fstab; then
        if [[ -e "/dev/disk/by-label/Zentra" ]]; then
            mount /store >/dev/null 2>&1 || true
        else
            echo "Detected stale /etc/fstab entry for LABEL=Zentra with no matching device."
            echo "Disabling that mount entry and continuing with local /store directory."
            sed -i -E 's|^([[:space:]]*LABEL=Zentra[[:space:]]+/store[[:space:]]+ext4[[:space:]].*)$|# \1|' /etc/fstab
            systemctl daemon-reload
            systemctl reset-failed store.mount dev-disk-by\\x2dlabel-Zentra.device >/dev/null 2>&1 || true
        fi
    fi
}

echo "[1/10] Installing required system packages..."
wait_for_apt_locks
dpkg --configure -a || true
apt-get update -y
wait_for_apt_locks
apt-get install -y \
    ca-certificates \
    curl \
    gnupg \
    lsb-release \
    build-essential \
    pkg-config \
    libssl-dev \
    libpq-dev \
    jq \
    python3 \
    python3-pip \
    postgresql \
    postgresql-contrib \
    nodejs \
    npm \
    nginx

if ! command -v cargo >/dev/null 2>&1; then
    echo "Installing Rust compiler (cargo/rustc)..."
    apt-get install -y rustc cargo
fi

ensure_timescaledb_installed

echo "[2/10] Ensuring service account and directories exist..."
ensure_store_path_ready
if ! id -u "${APP_USER}" >/dev/null 2>&1; then
    useradd --system --shell /usr/sbin/nologin --home-dir "${DEPLOY_PATH}" --no-create-home "${APP_USER}"
fi

mkdir -p "${DEPLOY_PATH}/bin" "${DEPLOY_PATH}/build" "${DEPLOY_PATH}/frontend/dist" "${DEPLOY_PATH}/migrations" "${DEPLOY_PATH}/logs"
mkdir -p "${PGDATA_PATH}"

echo "[3/10] Resolving PostgreSQL config path..."
PG_CONF_DIR="$(ls -d /etc/postgresql/*/main 2>/dev/null | head -n 1 || true)"
if [[ -z "${PG_CONF_DIR}" ]]; then
    echo "PostgreSQL configuration directory not found under /etc/postgresql/*/main" >&2
    exit 1
fi
PG_CONF_FILE="${PG_CONF_DIR}/postgresql.conf"
PG_HBA_FILE="${PG_CONF_DIR}/pg_hba.conf"

echo "[4/10] Configuring PostgreSQL on port ${POSTGRES_PORT}..."
sed -i -E "s/^#?[[:space:]]*port[[:space:]]*=[[:space:]]*[0-9]+/port = ${POSTGRES_PORT}/" "${PG_CONF_FILE}"
sed -i -E "s/^#?[[:space:]]*listen_addresses[[:space:]]*=.*/listen_addresses = '127.0.0.1'/" "${PG_CONF_FILE}"
if grep -Eq "^#?[[:space:]]*shared_preload_libraries[[:space:]]*=" "${PG_CONF_FILE}"; then
    sed -i -E "s|^#?[[:space:]]*shared_preload_libraries[[:space:]]*=.*|shared_preload_libraries = 'timescaledb'|" "${PG_CONF_FILE}"
else
    echo "shared_preload_libraries = 'timescaledb'" >> "${PG_CONF_FILE}"
fi

if ! grep -q "127.0.0.1/32" "${PG_HBA_FILE}"; then
    cat >> "${PG_HBA_FILE}" <<'EOF'
host    all             all             127.0.0.1/32            scram-sha-256
EOF
fi

systemctl enable postgresql
# Detect installed PG version for pg_ctlcluster
_PG_VER="$(ls /etc/postgresql 2>/dev/null | sort -V | tail -n1 || true)"
if command -v pg_ctlcluster >/dev/null 2>&1 && [[ -n "${_PG_VER}" ]]; then
    pg_ctlcluster "${_PG_VER}" main restart
else
    systemctl restart postgresql
fi

echo "[5/10] Configuring database and credentials..."
sudo -u postgres psql -v ON_ERROR_STOP=1 <<SQL
ALTER USER ${POSTGRES_USER} WITH PASSWORD '${POSTGRES_PASSWORD}';
SQL

if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='${POSTGRES_DB}'" | grep -q 1; then
    sudo -u postgres createdb "${POSTGRES_DB}"
fi

echo "[6/10] Writing runtime environment (.env)..."
cat > "${DEPLOY_PATH}/.env" <<EOF
DATABASE_URL=${RUNTIME_DB_URL}
ZENTRA_API_PORT=${BACKEND_PORT}
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=change_me_to_a_very_long_secret_at_least_32_chars
ZENTRA_MASTER_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
RUST_LOG=info
ZENTRA_APP_USER=${APP_USER}
ZENTRA_APP_PASSWORD=${APP_PASSWORD}
ZENTRA_ALLOW_ORIGINS=${ALLOW_ORIGIN}
EOF
chmod 640 "${DEPLOY_PATH}/.env"

echo "[7/10] Deploying binaries and frontend assets..."
if [[ -f "${DEPLOY_PATH}/build/zentra-server" ]]; then
    install -m 755 "${DEPLOY_PATH}/build/zentra-server" "${DEPLOY_PATH}/bin/zentra-server"
elif [[ -f "${DEPLOY_PATH}/bin/zentra-server" ]]; then
    chmod 755 "${DEPLOY_PATH}/bin/zentra-server"
else
    echo "Missing zentra-server binary. Expected ${DEPLOY_PATH}/build/zentra-server" >&2
    exit 1
fi

if [[ -d "${DEPLOY_PATH}/build/frontend-dist" ]]; then
    rm -rf "${DEPLOY_PATH}/frontend/dist"
    mkdir -p "${DEPLOY_PATH}/frontend/dist"
    cp -a "${DEPLOY_PATH}/build/frontend-dist/." "${DEPLOY_PATH}/frontend/dist/"
fi

chown -R "${APP_USER}:${APP_USER}" "${DEPLOY_PATH}"

echo "[8/10] Applying SQL migrations..."
shopt -s nullglob
migration_files=("${DEPLOY_PATH}/migrations/"*.sql)
if [[ "${#migration_files[@]}" -eq 0 ]]; then
    echo "No migration files found in ${DEPLOY_PATH}/migrations" >&2
    exit 1
fi

PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 \
    -h 127.0.0.1 \
    -p "${POSTGRES_PORT}" \
    -U "${POSTGRES_USER}" \
    -d "${POSTGRES_DB}" <<'SQL'
CREATE TABLE IF NOT EXISTS zentra_schema_migrations (
    filename TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SQL

migration_count_in_db="$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -tAc \
    "SELECT COUNT(*) FROM zentra_schema_migrations;" \
    -h 127.0.0.1 -p "${POSTGRES_PORT}" -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" | tr -d '[:space:]')"

if [[ "${migration_count_in_db}" == "0" ]]; then
    legacy_schema_present="$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -tAc \
        "SELECT CASE WHEN to_regclass('public.targets') IS NOT NULL AND to_regclass('public.metrics') IS NOT NULL AND to_regclass('public.logs') IS NOT NULL AND to_regclass('public.users') IS NOT NULL THEN 1 ELSE 0 END;" \
        -h 127.0.0.1 -p "${POSTGRES_PORT}" -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" | tr -d '[:space:]')"

    if [[ "${legacy_schema_present}" == "1" ]]; then
        echo "Detected pre-existing schema without migration ledger; seeding migration ledger for idempotent reruns."
        for m in "${migration_files[@]}"; do
            migration_name="$(basename "${m}")"
            PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 \
                -h 127.0.0.1 \
                -p "${POSTGRES_PORT}" \
                -U "${POSTGRES_USER}" \
                -d "${POSTGRES_DB}" \
                -c "INSERT INTO zentra_schema_migrations(filename) VALUES ('${migration_name}') ON CONFLICT (filename) DO NOTHING;" >/dev/null
        done
    fi
fi

for m in "${migration_files[@]}"; do
    migration_name="$(basename "${m}")"
    already_applied="$(PGPASSWORD="${POSTGRES_PASSWORD}" psql -tAc \
        "SELECT 1 FROM zentra_schema_migrations WHERE filename='${migration_name}'" \
        -h 127.0.0.1 -p "${POSTGRES_PORT}" -U "${POSTGRES_USER}" -d "${POSTGRES_DB}" | tr -d '[:space:]')"

    if [[ "${already_applied}" == "1" ]]; then
        echo "Skipping migration (already applied): ${migration_name}"
        continue
    fi

    echo "Applying migration: ${migration_name}"
    PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 \
        -h 127.0.0.1 \
        -p "${POSTGRES_PORT}" \
        -U "${POSTGRES_USER}" \
        -d "${POSTGRES_DB}" \
        -f "${m}" >/dev/null

    PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 \
        -h 127.0.0.1 \
        -p "${POSTGRES_PORT}" \
        -U "${POSTGRES_USER}" \
        -d "${POSTGRES_DB}" \
        -c "INSERT INTO zentra_schema_migrations(filename) VALUES ('${migration_name}') ON CONFLICT (filename) DO NOTHING;" >/dev/null
done

echo "[9/10] Writing and enabling systemd services + nginx..."
cat > /etc/systemd/system/zentra-api.service <<EOF
[Unit]
Description=Zentra API Server
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=${APP_USER}
Group=${APP_USER}
WorkingDirectory=${DEPLOY_PATH}
EnvironmentFile=${DEPLOY_PATH}/.env
ExecStart=${DEPLOY_PATH}/bin/zentra-server
Restart=always
RestartSec=5
StandardOutput=append:${DEPLOY_PATH}/logs/api.log
StandardError=append:${DEPLOY_PATH}/logs/api.log

[Install]
WantedBy=multi-user.target
EOF

# --- nginx config ---
NGINX_CONF="/etc/nginx/sites-available/zentra"
cat > "${NGINX_CONF}" <<NGINXEOF
server {
    listen ${FRONTEND_PORT};
    server_name _ ${EXTERNAL_IP};

    root ${DEPLOY_PATH}/frontend/dist;
    index index.html;

    # Proxy all API and WebSocket traffic to the Rust backend
    location /api/ {
        proxy_pass         http://127.0.0.1:${BACKEND_PORT};
        proxy_http_version 1.1;
        proxy_set_header   Host              \$host;
        proxy_set_header   X-Real-IP         \$remote_addr;
        proxy_set_header   X-Forwarded-For   \$proxy_add_x_forwarded_for;
        proxy_set_header   Upgrade           \$http_upgrade;
        proxy_set_header   Connection        "upgrade";
        proxy_read_timeout 300s;
        proxy_send_timeout 300s;
    }

    # gRPC/OTLP endpoint
    location /otlp/ {
        proxy_pass         http://127.0.0.1:${BACKEND_PORT};
        proxy_http_version 1.1;
        proxy_set_header   Host              \$host;
        proxy_set_header   X-Real-IP         \$remote_addr;
        proxy_set_header   X-Forwarded-For   \$proxy_add_x_forwarded_for;
    }

    # SPA fallback — send all other requests to index.html
    location / {
        try_files \$uri \$uri/ /index.html;
    }

    access_log ${DEPLOY_PATH}/logs/nginx-access.log;
    error_log  ${DEPLOY_PATH}/logs/nginx-error.log;
}
NGINXEOF

ln -sf "${NGINX_CONF}" /etc/nginx/sites-enabled/zentra
rm -f /etc/nginx/sites-enabled/default

# Validate nginx config before reloading
nginx -t

systemctl daemon-reload
systemctl enable zentra-api nginx
systemctl restart zentra-api
systemctl reload-or-restart nginx

echo "[10/10] Health checks and summary..."
API_HEALTH="unreachable"
for _i in 1 2 3 4 5 6 7 8 9 10 11 12; do
    if curl -fsS "http://127.0.0.1:${BACKEND_PORT}/api/health" >/dev/null 2>&1; then
        API_HEALTH="ok"
        break
    fi
    sleep 5
done

FE_HEALTH="unreachable"
for _i in 1 2 3 4 5 6; do
    if curl -fsS "http://127.0.0.1:${FRONTEND_PORT}" >/dev/null 2>&1; then
        FE_HEALTH="ok"
        break
    fi
    sleep 5
done

echo ""
echo "========== Setup Summary =========="
echo "Deploy path        : ${DEPLOY_PATH}"
echo "PG data path       : ${PGDATA_PATH}"
echo "Postgres port      : ${POSTGRES_PORT}"
echo "Database URL       : ${RUNTIME_DB_URL}"
echo "Migrations applied : ${#migration_files[@]}"
echo "Node version       : $(node -v 2>/dev/null || echo 'not found')"
echo "Rust version       : $(rustc --version 2>/dev/null || echo 'not found')"
echo "API service        : $(systemctl is-active zentra-api)"
echo "Nginx service      : $(systemctl is-active nginx)"
echo "API health         : ${API_HEALTH}"
echo "Frontend (nginx)   : ${FE_HEALTH}"
echo "Frontend URL       : http://${EXTERNAL_IP}:${FRONTEND_PORT}"
echo "=================================="
