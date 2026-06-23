#!/usr/bin/env bash
# =============================================================================
# Zentra — Server Setup Script (Binary-Only Deployment)
# =============================================================================
# Run as root on a fresh Debian 12 / Rocky Linux 8 server.
#
# Expects pre-built artifacts at /store/zentra (cloned from GitHub):
#   /store/zentra/bin/zentra-server     — API binary (Linux x86_64)
#   /store/zentra/frontend/dist/        — React SPA static files
#   /store/zentra/migrations/           — SQL files (reference only — embedded in binary)
#   /store/zentra/deploy/nginx.conf     — Nginx reverse proxy config
#
# What this does:
#   1.  Installs runtime dependencies (PostgreSQL 16, TimescaleDB, Nginx)
#   2.  Configures PostgreSQL (database, users, extensions)
#   3.  Creates service user and .env with generated secrets
#   4.  Configures Nginx reverse proxy (frontend :5010 → API :5006)
#   5.  Creates systemd services and starts everything
#   6.  Runs health check
#
# NO build tools are installed — no Rust, no Node.js, no cargo, no npm.
# Migrations are embedded in the zentra-server binary and run on first startup.
#
# Usage:
#   git clone https://github.com/rameshsraj/zentra-release.git /store/zentra
#   cd /store/zentra
#   sudo bash deploy/setup-debian12.sh
# =============================================================================
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────────────────
DEPLOY_DIR="/store/zentra"
SERVICE_USER="zentra"

PG_HOST="127.0.0.1"
PG_PORT="5434"
PG_USER="postgres"
PG_DB="zentra"
PG_DATA_DIR="/store/zentra/pgdata"

API_PORT=5006
FRONTEND_PORT=5010

LOG_FILE="/var/log/zentra-setup.log"

# ─────────────────────────────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────────────────────────────
info()  { echo -e "\e[36m[INFO]\e[0m  $*" | tee -a "${LOG_FILE}"; }
ok()    { echo -e "\e[32m[ OK ]\e[0m  $*" | tee -a "${LOG_FILE}"; }
warn()  { echo -e "\e[33m[WARN]\e[0m  $*" | tee -a "${LOG_FILE}"; }
fail()  { echo -e "\e[31m[FAIL]\e[0m  $*" | tee -a "${LOG_FILE}"; exit 1; }

# ─────────────────────────────────────────────────────────────────────────────
# Pre-flight
# ─────────────────────────────────────────────────────────────────────────────
[ "$(id -u)" -ne 0 ] && fail "Run as root (sudo)"
[ ! -f "${DEPLOY_DIR}/bin/zentra-server" ] && fail "Binary not found at ${DEPLOY_DIR}/bin/zentra-server — clone the repo first."
[ ! -d "${DEPLOY_DIR}/frontend/dist" ] && fail "Frontend not found at ${DEPLOY_DIR}/frontend/dist/"
mkdir -p "$(dirname "${LOG_FILE}")"
echo "=== Zentra Setup $(date) ===" > "${LOG_FILE}"

# Detect server IPs
EXTERNAL_IP=$(curl -sf --max-time 5 http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip -H "Metadata-Flavor: Google" 2>/dev/null || hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")
INTERNAL_IP=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "127.0.0.1")

echo ""
echo "======================================================================"
echo "  Zentra — Server Setup (Binary-Only)"
echo "======================================================================"
echo "  Deploy:     ${DEPLOY_DIR}"
echo "  External:   http://${EXTERNAL_IP}:${FRONTEND_PORT}"
echo "  Internal:   http://${INTERNAL_IP}:${FRONTEND_PORT}"
echo "  PostgreSQL: ${PG_USER}@${PG_HOST}:${PG_PORT}/${PG_DB}"
echo "======================================================================"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# Detect distro (Debian/Ubuntu or RHEL/Rocky)
# ═══════════════════════════════════════════════════════════════════════════
IS_DEBIAN=false
IS_RHEL=false
if [ -f /etc/debian_version ]; then
    IS_DEBIAN=true
elif [ -f /etc/redhat-release ]; then
    IS_RHEL=true
else
    warn "Unknown distro — assuming RHEL-like"
    IS_RHEL=true
fi

# ═══════════════════════════════════════════════════════════════════════════
# STEP 1 — Install runtime dependencies
# ═══════════════════════════════════════════════════════════════════════════
info "Step 1/6: Installing runtime dependencies..."

if [ "${IS_DEBIAN}" = true ]; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq >> "${LOG_FILE}" 2>&1
    apt-get install -y -qq curl ca-certificates gnupg lsb-release nginx >> "${LOG_FILE}" 2>&1

    # PostgreSQL 16
    if ! command -v psql >/dev/null 2>&1; then
        curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc \
            | gpg --dearmor -o /etc/apt/keyrings/pgdg.gpg 2>/dev/null || true
        echo "deb [signed-by=/etc/apt/keyrings/pgdg.gpg] http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" \
            > /etc/apt/sources.list.d/pgdg.list
        apt-get update -qq >> "${LOG_FILE}" 2>&1
        apt-get install -y -qq postgresql-16 postgresql-contrib-16 >> "${LOG_FILE}" 2>&1
    fi

    # TimescaleDB
    if ! dpkg -l | grep -q timescaledb 2>/dev/null; then
        curl -fsSL https://packagecloud.io/timescale/timescaledb/gpgkey \
            | gpg --dearmor -o /etc/apt/keyrings/timescaledb.gpg 2>/dev/null || true
        echo "deb [signed-by=/etc/apt/keyrings/timescaledb.gpg] https://packagecloud.io/timescale/timescaledb/debian/ $(lsb_release -cs) main" \
            > /etc/apt/sources.list.d/timescaledb.list
        apt-get update -qq >> "${LOG_FILE}" 2>&1
        apt-get install -y -qq timescaledb-2-postgresql-16 >> "${LOG_FILE}" 2>&1 || warn "TimescaleDB install failed"
    fi

    # On Debian, postgresql.service is a meta-wrapper (ExecStart=/bin/true).
    # The real cluster service is postgresql@16-main.service.
    PG_SERVICE="postgresql@16-main"
    PG_BIN="/usr/lib/postgresql/16/bin"
    NGINX_SITES="/etc/nginx/sites-available"
    NGINX_ENABLED="/etc/nginx/sites-enabled"
else
    # RHEL / Rocky Linux 8
    dnf install -y dnf-utils curl ca-certificates nginx >> "${LOG_FILE}" 2>&1

    if ! command -v psql >/dev/null 2>&1; then
        dnf install -y https://download.postgresql.org/pub/repos/yum/reporpms/EL-8-x86_64/pgdg-redhat-repo-latest.noarch.rpm >> "${LOG_FILE}" 2>&1
        dnf -qy module disable postgresql >> "${LOG_FILE}" 2>&1 || true
        dnf install -y postgresql16-server postgresql16-contrib >> "${LOG_FILE}" 2>&1
    fi

    # TimescaleDB
    if ! rpm -q timescaledb-2-postgresql-16 >/dev/null 2>&1; then
        cat > /etc/yum.repos.d/timescaledb.repo <<'TSREPO'
[timescaledb]
name=TimescaleDB Repository
baseurl=https://packagecloud.io/timescale/timescaledb/el/8/$basearch
gpgcheck=0
enabled=1
TSREPO
        dnf install -y timescaledb-2-postgresql-16 >> "${LOG_FILE}" 2>&1 || warn "TimescaleDB install failed"
    fi

    PG_SERVICE="postgresql-16"
    PG_BIN="/usr/pgsql-16/bin"
    NGINX_SITES="/etc/nginx/conf.d"
    NGINX_ENABLED=""  # RHEL uses conf.d directly
fi

ok "Runtime dependencies installed"

# ═══════════════════════════════════════════════════════════════════════════
# Initialize PostgreSQL data directory under /store/zentra/pgdata/
# ═══════════════════════════════════════════════════════════════════════════
info "Initializing PostgreSQL data directory at ${PG_DATA_DIR}..."

# Stop PostgreSQL if running (it may have started with default data dir)
systemctl stop "${PG_SERVICE}" >> "${LOG_FILE}" 2>&1 || true
[ "${IS_DEBIAN}" = true ] && pg_ctlcluster 16 main stop >> "${LOG_FILE}" 2>&1 || true

if [ ! -f "${PG_DATA_DIR}/PG_VERSION" ]; then
    mkdir -p "${PG_DATA_DIR}"
    chown postgres:postgres "${PG_DATA_DIR}"
    chmod 700 "${PG_DATA_DIR}"

    # Ensure locale exists (Debian minimal images may not have it)
    if ! locale -a 2>/dev/null | grep -qi 'en_US.utf8'; then
        info "Generating en_US.UTF-8 locale..."
        if [ -f /etc/locale.gen ]; then
            sed -i 's/^# *en_US.UTF-8/en_US.UTF-8/' /etc/locale.gen
            locale-gen >> "${LOG_FILE}" 2>&1 || true
        fi
    fi

    # Determine locale to use
    INIT_LOCALE="en_US.UTF-8"
    if ! locale -a 2>/dev/null | grep -qi 'en_US.utf8'; then
        INIT_LOCALE="C.UTF-8"
        warn "en_US.UTF-8 not available, using C.UTF-8"
    fi

    # Initialize the cluster in our custom directory
    info "Running initdb (locale=${INIT_LOCALE})..."
    sudo -u postgres "${PG_BIN}/initdb" -D "${PG_DATA_DIR}" --encoding=UTF8 --locale="${INIT_LOCALE}" >> "${LOG_FILE}" 2>&1 \
        || fail "initdb failed — check ${LOG_FILE} for details"
    ok "PostgreSQL data directory initialized at ${PG_DATA_DIR}"
else
    ok "PostgreSQL data directory already exists at ${PG_DATA_DIR}"
fi

# Override systemd to use the custom data directory
if [ "${IS_DEBIAN}" = true ]; then
    # Debian uses pg_ctlcluster + /etc/postgresql/16/main/postgresql.conf
    # Patch data_directory to point at our custom location
    DEBIAN_PG_CONF="/etc/postgresql/16/main/postgresql.conf"
    if [ -f "${DEBIAN_PG_CONF}" ]; then
        sed -i "s|^data_directory = .*|data_directory = '${PG_DATA_DIR}'|" "${DEBIAN_PG_CONF}"
        grep -q "^data_directory" "${DEBIAN_PG_CONF}" || \
            echo "data_directory = '${PG_DATA_DIR}'" >> "${DEBIAN_PG_CONF}"
    fi
    # Also ensure unix_socket_directories points to standard location
    sed -i "s|^#*unix_socket_directories = .*|unix_socket_directories = '/var/run/postgresql'|" "${DEBIAN_PG_CONF}" || true
else
    # RHEL: override the systemd service
    mkdir -p /etc/systemd/system/${PG_SERVICE}.service.d
    cat > /etc/systemd/system/${PG_SERVICE}.service.d/override.conf <<SYSOVER
[Service]
Environment=PGDATA=${PG_DATA_DIR}
SYSOVER
    systemctl daemon-reload
fi

# Point conf variables to the custom data dir
PG_CONF="${PG_DATA_DIR}/postgresql.conf"
PG_HBA="${PG_DATA_DIR}/pg_hba.conf"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 2 — Configure PostgreSQL
# ═══════════════════════════════════════════════════════════════════════════
info "Step 2/6: Configuring PostgreSQL..."

# PostgreSQL password
PG_PASS="zantaz"

# Configure postgresql.conf
if [ -f "${PG_CONF}" ]; then
    # Set port
    sed -i "s/^#*port = .*/port = ${PG_PORT}/" "${PG_CONF}"
    # Listen on localhost only
    sed -i "s/^#*listen_addresses = .*/listen_addresses = '127.0.0.1'/" "${PG_CONF}"

    # TimescaleDB + tuning (append if not already present)
    if ! grep -q "timescaledb" "${PG_CONF}" 2>/dev/null; then
        cat >> "${PG_CONF}" <<'PGTUNING'

# Zentra configuration
shared_preload_libraries = 'timescaledb'
timescaledb.telemetry_level = off
max_connections = 200
shared_buffers = 256MB
effective_cache_size = 512MB
work_mem = 4MB
maintenance_work_mem = 64MB
random_page_cost = 1.1
password_encryption = scram-sha-256
log_connections = on
log_disconnections = on
log_statement = 'ddl'
log_timezone = 'UTC'
timezone = 'UTC'
PGTUNING
    fi
fi

# Configure pg_hba.conf
if [ -f "${PG_HBA}" ]; then
    cat > "${PG_HBA}" <<'HBAEOF'
# TYPE  DATABASE  USER      ADDRESS       METHOD
local   all       postgres                peer
local   all       all                     scram-sha-256
host    all       all       127.0.0.1/32  scram-sha-256
host    all       all       ::1/128       scram-sha-256
host    all       all       0.0.0.0/0     reject
host    all       all       ::/0          reject
HBAEOF
fi

# Start PostgreSQL with custom data dir and new config
systemctl enable "${PG_SERVICE}" >> "${LOG_FILE}" 2>&1
systemctl restart "${PG_SERVICE}" >> "${LOG_FILE}" 2>&1

# Wait for PostgreSQL to be ready (up to 30s)
info "Waiting for PostgreSQL to be ready..."
for i in $(seq 1 15); do
    if sudo -u postgres psql -p "${PG_PORT}" -c 'SELECT 1' >/dev/null 2>&1; then
        ok "PostgreSQL is ready (${i}s)"
        break
    fi
    sleep 2
done
sudo -u postgres psql -p "${PG_PORT}" -c 'SELECT 1' >/dev/null 2>&1 \
    || fail "PostgreSQL did not start — check: journalctl -u ${PG_SERVICE} -n 50"

# Set postgres password (cluster is using trust auth initially from initdb)
sudo -u postgres psql -p "${PG_PORT}" -c "ALTER USER postgres PASSWORD '${PG_PASS}';" >> "${LOG_FILE}" 2>&1
export PGPASSWORD="${PG_PASS}"

# Create database
DB_EXISTS=$(sudo -u postgres psql -p "${PG_PORT}" -t -c \
    "SELECT 1 FROM pg_database WHERE datname='${PG_DB}';" 2>/dev/null | tr -d ' ')
if [ "${DB_EXISTS}" != "1" ]; then
    sudo -u postgres psql -p "${PG_PORT}" -c "CREATE DATABASE ${PG_DB} OWNER ${PG_USER};" >> "${LOG_FILE}" 2>&1
    ok "Database '${PG_DB}' created"
else
    ok "Database '${PG_DB}' already exists"
fi

# Enable extensions
sudo -u postgres psql -p "${PG_PORT}" -d "${PG_DB}" <<'EXTSQL' >> "${LOG_FILE}" 2>&1 || true
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
EXTSQL
ok "Extensions enabled (timescaledb, pg_trgm, uuid-ossp)"

# Create restricted app user
sudo -u postgres psql -p "${PG_PORT}" -d "${PG_DB}" <<APPSQL >> "${LOG_FILE}" 2>&1 || true
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'zentra_app') THEN
    CREATE USER zentra_app WITH PASSWORD '${PG_PASS}';
  ELSE
    ALTER USER zentra_app WITH PASSWORD '${PG_PASS}';
  END IF;
END
\$\$;
GRANT CONNECT ON DATABASE ${PG_DB} TO zentra_app;
GRANT USAGE ON SCHEMA public TO zentra_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO zentra_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO zentra_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO zentra_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE ON SEQUENCES TO zentra_app;
GRANT CREATE ON SCHEMA public TO zentra_app;
APPSQL

ok "PostgreSQL configured (port ${PG_PORT}, database ${PG_DB}, user zentra_app)"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 3 — Service user and .env
# ═══════════════════════════════════════════════════════════════════════════
info "Step 3/6: Setting up service user and .env..."

if ! id -u "${SERVICE_USER}" >/dev/null 2>&1; then
    useradd --system --shell /usr/sbin/nologin --home-dir "${DEPLOY_DIR}" "${SERVICE_USER}"
    ok "Service user '${SERVICE_USER}' created"
fi

mkdir -p "${DEPLOY_DIR}"/{logs,.ssh}
chmod 700 "${DEPLOY_DIR}/.ssh"

# Generate secrets
JWT_SECRET=$(openssl rand -hex 32)
MASTER_KEY=$(openssl rand -hex 32)

cat > "${DEPLOY_DIR}/.env" <<ENVEOF
# Zentra Environment — generated by setup-debian12.sh on $(date -u +%Y-%m-%d)
DATABASE_URL=postgresql://zentra_app:${PG_PASS}@${PG_HOST}:${PG_PORT}/${PG_DB}
ZENTRA_JWT_SECRET=${JWT_SECRET}
ZENTRA_MASTER_KEY=${MASTER_KEY}
ZENTRA_ALLOW_INSECURE_JWT=0
ZENTRA_API_PORT=${API_PORT}
ZENTRA_BIND_ADDR=127.0.0.1
ZENTRA_ALLOW_ORIGINS=*
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
RUST_LOG=info
HOME=${DEPLOY_DIR}
ENVEOF

chmod 600 "${DEPLOY_DIR}/.env"
chown -R "${SERVICE_USER}:${SERVICE_USER}" "${DEPLOY_DIR}"
ok ".env written to ${DEPLOY_DIR}/.env"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 4 — Configure Nginx
# ═══════════════════════════════════════════════════════════════════════════
info "Step 4/6: Configuring Nginx..."

# Use the bundled nginx.conf if available, otherwise generate one
NGINX_DEST=""
if [ "${IS_DEBIAN}" = true ]; then
    NGINX_DEST="/etc/nginx/sites-available/zentra"

    if [ -f "${DEPLOY_DIR}/deploy/nginx.conf" ]; then
        cp "${DEPLOY_DIR}/deploy/nginx.conf" "${NGINX_DEST}"
    else
        # Generate minimal config
        cat > "${NGINX_DEST}" <<NGINXEOF
limit_req_zone \$binary_remote_addr zone=login_limit:10m rate=5r/m;
limit_req_zone \$binary_remote_addr zone=api_limit:10m rate=30r/s;

server {
    listen ${FRONTEND_PORT};
    server_name _;
    root ${DEPLOY_DIR}/frontend/dist;
    index index.html;
    server_tokens off;
    client_max_body_size 2m;

    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;

    location / {
        try_files \$uri \$uri/ /index.html;
    }

    location = /api/auth/login {
        limit_req zone=login_limit burst=3 nodelay;
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
    }

    location /api/ {
        limit_req zone=api_limit burst=60 nodelay;
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 60s;
    }

    location /v1/ {
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        client_max_body_size 10m;
    }

    gzip on;
    gzip_types text/plain text/css application/json application/javascript;
    gzip_min_length 1024;
}
NGINXEOF
    fi

    rm -f /etc/nginx/sites-enabled/default
    ln -sf "${NGINX_DEST}" /etc/nginx/sites-enabled/zentra
else
    # RHEL — use conf.d
    NGINX_DEST="/etc/nginx/conf.d/zentra.conf"
    if [ -f "${DEPLOY_DIR}/deploy/nginx.conf" ]; then
        cp "${DEPLOY_DIR}/deploy/nginx.conf" "${NGINX_DEST}"
    fi
    rm -f /etc/nginx/conf.d/default.conf
fi

nginx -t >> "${LOG_FILE}" 2>&1 || fail "Nginx config test failed"
ok "Nginx configured on port ${FRONTEND_PORT}"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 5 — Systemd services
# ═══════════════════════════════════════════════════════════════════════════
info "Step 5/6: Creating systemd services..."

cat > /etc/systemd/system/zentra-api.service <<SVCEOF
[Unit]
Description=Zentra API Server
After=network.target ${PG_SERVICE}.service
Requires=${PG_SERVICE}.service

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_USER}
WorkingDirectory=${DEPLOY_DIR}
EnvironmentFile=${DEPLOY_DIR}/.env
ExecStart=${DEPLOY_DIR}/bin/zentra-server
Restart=always
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=${DEPLOY_DIR}/logs ${DEPLOY_DIR}/.ssh
PrivateTmp=true
StandardOutput=append:${DEPLOY_DIR}/logs/api.log
StandardError=append:${DEPLOY_DIR}/logs/api-error.log
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
SVCEOF

systemctl daemon-reload
systemctl enable zentra-api >> "${LOG_FILE}" 2>&1
systemctl restart zentra-api
ok "zentra-api service started"

systemctl restart nginx
ok "Nginx restarted"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 6 — Health check
# ═══════════════════════════════════════════════════════════════════════════
info "Step 6/6: Running health checks..."

sleep 5

HEALTH_OK=false
for i in $(seq 1 15); do
    if curl -sf "http://127.0.0.1:${API_PORT}/api/health" >/dev/null 2>&1; then
        HEALTH_OK=true
        break
    fi
    printf "."
    sleep 3
done
echo ""

if [ "${HEALTH_OK}" = true ]; then
    ok "API health check PASSED"
else
    warn "API health check failed — check: journalctl -u zentra-api -n 50"
    warn "Or: tail -50 ${DEPLOY_DIR}/logs/api.log"
fi

if curl -sf "http://127.0.0.1:${FRONTEND_PORT}/" >/dev/null 2>&1; then
    ok "Frontend health check PASSED"
else
    warn "Frontend not responding — check: journalctl -u nginx -n 20"
fi

# ═══════════════════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════════════════
API_STATUS=$(systemctl is-active zentra-api 2>/dev/null || echo "unknown")
NGINX_STATUS=$(systemctl is-active nginx 2>/dev/null || echo "unknown")
PG_STATUS=$(systemctl is-active "${PG_SERVICE}" 2>/dev/null || echo "unknown")

echo ""
echo ""
echo "======================================================================"
echo "  Zentra — Setup Complete!"
echo "======================================================================"
echo ""
echo "  WEB UI:     http://${EXTERNAL_IP}:${FRONTEND_PORT}"
echo "  Internal:   http://${INTERNAL_IP}:${FRONTEND_PORT}"
echo "  Login:      admin@Zentra.local / changeme"
echo ""
echo "  SERVICES:"
echo "    postgresql:   ${PG_STATUS}  (port ${PG_PORT})"
echo "    zentra-api:   ${API_STATUS}"
echo "    nginx:        ${NGINX_STATUS}  (port ${FRONTEND_PORT})"
echo ""
echo "  COMMANDS:"
echo "    Status:     systemctl status zentra-api"
echo "    Logs:       journalctl -u zentra-api -f"
echo "    Restart:    systemctl restart zentra-api nginx"
echo ""
echo "  FILES:"
echo "    Config:     ${DEPLOY_DIR}/.env"
echo "    Logs:       ${DEPLOY_DIR}/logs/"
echo "    Binary:     ${DEPLOY_DIR}/bin/zentra-server"
echo "    Frontend:   ${DEPLOY_DIR}/frontend/dist/"
echo "    Setup log:  ${LOG_FILE}"
echo ""
echo "  IMPORTANT: Change the default admin password after first login!"
echo "======================================================================"
echo ""
