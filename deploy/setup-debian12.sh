#!/usr/bin/env bash
# =============================================================================
# Zentra — Complete Server Setup Script (Debian 12 Bookworm)
# =============================================================================
# Run as root on a fresh Debian 12 server with PostgreSQL already installed.
#
# What this does:
#   1.  Installs system dependencies (build tools, libssl, libpq, clang, etc.)
#   2.  Installs Node.js 20.x
#   3.  Installs Rust stable toolchain + sqlx-cli
#   4.  Configures PostgreSQL (zentra DB, TimescaleDB, extensions)
#   5.  Creates deploy directory, service user, .env
#   6.  Builds the Rust backend (zentra-server)
#   7.  Builds the React frontend (npm ci + vite build)
#   8.  Runs database migrations
#   9.  Configures Nginx reverse proxy (frontend :5010 → API :5006)
#  10.  Creates systemd services and starts everything
#  11.  Runs health check
#
# Usage:
#   chmod +x /home/rameshsraj/projects/Zentra/deploy/setup-debian12.sh
#   sudo bash /home/rameshsraj/projects/Zentra/deploy/setup-debian12.sh
# =============================================================================
set -euo pipefail

# ─────────────────────────────────────────────────────────────────────────────
# Configuration — edit these if your setup differs
# ─────────────────────────────────────────────────────────────────────────────
PROJECT_DIR="/home/rameshsraj/projects/Zentra"
DEPLOY_DIR="/opt/zentra"
SERVICE_USER="zentra"

EXTERNAL_IP="34.68.222.94"
INTERNAL_IP="10.128.0.2"

PG_HOST="127.0.0.1"
PG_PORT="5432"
PG_USER="postgres"
PG_PASS="zantaz"
PG_DB="zentra"

API_PORT=5006
FRONTEND_PORT=5010
AGENT_PORT=5001

BUILD_DIR="/mnt/data/zentra-build"

NODE_MAJOR=20
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
[ ! -d "${PROJECT_DIR}" ] && fail "Project not found at ${PROJECT_DIR}"
mkdir -p "$(dirname "${LOG_FILE}")"
echo "=== Zentra Setup $(date) ===" > "${LOG_FILE}"

echo ""
echo "======================================================================"
echo "  Zentra — Full Setup (Debian 12 Bookworm)"
echo "======================================================================"
echo "  Source:     ${PROJECT_DIR}"
echo "  Deploy:     ${DEPLOY_DIR}"
echo "  External:   http://${EXTERNAL_IP}:${FRONTEND_PORT}"
echo "  PostgreSQL: ${PG_USER}@${PG_HOST}:${PG_PORT}/${PG_DB}"
echo "======================================================================"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# STEP 1 — System packages
# ═══════════════════════════════════════════════════════════════════════════
info "Step 1/11: Installing system packages..."

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq >> "${LOG_FILE}" 2>&1
apt-get install -y -qq \
    build-essential pkg-config libssl-dev libpq-dev \
    curl wget git unzip ca-certificates gnupg lsb-release \
    libaio1 libclang-dev clang \
    nginx \
    python3 sudo \
    >> "${LOG_FILE}" 2>&1

ok "System packages installed"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 2 — Node.js 20.x
# ═══════════════════════════════════════════════════════════════════════════
info "Step 2/11: Installing Node.js ${NODE_MAJOR}.x..."

if command -v node >/dev/null 2>&1 && node --version | grep -q "v${NODE_MAJOR}"; then
    ok "Node.js $(node --version) already present"
else
    mkdir -p /etc/apt/keyrings
    curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
        | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg 2>/dev/null || true
    echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_${NODE_MAJOR}.x nodistro main" \
        > /etc/apt/sources.list.d/nodesource.list
    apt-get update -qq >> "${LOG_FILE}" 2>&1
    apt-get install -y -qq nodejs >> "${LOG_FILE}" 2>&1
    ok "Node.js $(node --version) installed"
fi

# ═══════════════════════════════════════════════════════════════════════════
# STEP 3 — Rust toolchain
# ═══════════════════════════════════════════════════════════════════════════
info "Step 3/11: Installing Rust toolchain..."

if command -v rustc >/dev/null 2>&1; then
    ok "Rust $(rustc --version | awk '{print $2}') already present"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable >> "${LOG_FILE}" 2>&1
    ok "Rust installed"
fi

# Make cargo available for rest of script
export PATH="${HOME}/.cargo/bin:${PATH}"
# shellcheck disable=SC1091
source "${HOME}/.cargo/env" 2>/dev/null || true

if ! command -v sqlx >/dev/null 2>&1; then
    info "Installing sqlx-cli (for migrations)..."
    cargo install sqlx-cli --no-default-features --features postgres >> "${LOG_FILE}" 2>&1
    ok "sqlx-cli installed"
fi

# ═══════════════════════════════════════════════════════════════════════════
# STEP 4 — PostgreSQL: database, TimescaleDB, extensions
# ═══════════════════════════════════════════════════════════════════════════
info "Step 4/11: Configuring PostgreSQL..."

export PGPASSWORD="${PG_PASS}"

# Test connection
if ! psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -c "SELECT 1;" >/dev/null 2>&1; then
    fail "Cannot connect to PostgreSQL at ${PG_HOST}:${PG_PORT}"
fi
ok "PostgreSQL connection verified"

# Install TimescaleDB package if extension is not available
TSDB_AVAIL=$(psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -d postgres -t -c \
    "SELECT count(*) FROM pg_available_extensions WHERE name='timescaledb';" 2>/dev/null | tr -d ' ')
if [ "${TSDB_AVAIL:-0}" = "0" ]; then
    info "Installing TimescaleDB..."
    PG_MAJOR=$(psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -t -c "SHOW server_version;" 2>/dev/null | awk -F. '{print $1}' | tr -d ' ')
    [ -z "${PG_MAJOR}" ] && PG_MAJOR=16

    # Add TimescaleDB repo
    curl -fsSL https://packagecloud.io/timescale/timescaledb/gpgkey \
        | gpg --dearmor -o /etc/apt/keyrings/timescaledb.gpg 2>/dev/null || true
    echo "deb [signed-by=/etc/apt/keyrings/timescaledb.gpg] https://packagecloud.io/timescale/timescaledb/debian/ $(lsb_release -cs) main" \
        > /etc/apt/sources.list.d/timescaledb.list
    apt-get update -qq >> "${LOG_FILE}" 2>&1 || true
    apt-get install -y -qq "timescaledb-2-postgresql-${PG_MAJOR}" >> "${LOG_FILE}" 2>&1 || {
        warn "TimescaleDB package install failed — continuing without it"
    }

    # Add to shared_preload_libraries and restart
    PG_CONF=$(psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -t -c "SHOW config_file;" 2>/dev/null | tr -d ' ')
    if [ -n "${PG_CONF}" ] && [ -f "${PG_CONF}" ]; then
        if ! grep -q "timescaledb" "${PG_CONF}" 2>/dev/null; then
            echo "shared_preload_libraries = 'timescaledb'" >> "${PG_CONF}"
            echo "timescaledb.telemetry_level = off" >> "${PG_CONF}"
            systemctl restart postgresql >> "${LOG_FILE}" 2>&1 || true
            sleep 3
            info "PostgreSQL restarted with TimescaleDB"
        fi
    fi
fi

# Create database
DB_EXISTS=$(psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -t -c \
    "SELECT 1 FROM pg_database WHERE datname='${PG_DB}';" 2>/dev/null | tr -d ' ')
if [ "${DB_EXISTS}" != "1" ]; then
    psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -c \
        "CREATE DATABASE ${PG_DB} OWNER ${PG_USER};" >> "${LOG_FILE}" 2>&1
    ok "Database '${PG_DB}' created"
else
    ok "Database '${PG_DB}' already exists"
fi

# Enable extensions
psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" <<'EOSQL' >> "${LOG_FILE}" 2>&1 || true
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
EOSQL
ok "Extensions enabled (timescaledb, pg_trgm, uuid-ossp)"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 5 — Deploy directory, service user, .env
# ═══════════════════════════════════════════════════════════════════════════
info "Step 5/11: Setting up deploy directory and .env..."

if ! id -u "${SERVICE_USER}" >/dev/null 2>&1; then
    useradd --system --shell /usr/sbin/nologin --home-dir "${DEPLOY_DIR}" --create-home "${SERVICE_USER}"
    ok "Service user '${SERVICE_USER}' created"
fi

mkdir -p "${DEPLOY_DIR}"/{bin,logs,migrations,frontend/dist,backups}

# Generate secure secrets
JWT_SECRET=$(openssl rand -base64 48 | tr -dc 'a-zA-Z0-9' | head -c 64)
MASTER_KEY=$(openssl rand -hex 32)

cat > "${DEPLOY_DIR}/.env" <<ENVEOF
# ─── Zentra Environment (generated by setup-debian12.sh) ───
POSTGRES_HOST=${PG_HOST}
POSTGRES_PORT=${PG_PORT}
POSTGRES_USER=${PG_USER}
POSTGRES_PASSWORD=${PG_PASS}
POSTGRES_DATABASE=${PG_DB}
DATABASE_URL=postgresql://${PG_USER}:${PG_PASS}@${PG_HOST}:${PG_PORT}/${PG_DB}
DB_POOL_SIZE=10

ZENTRA_API_PORT=${API_PORT}
ZENTRA_AGENT_PORT=${AGENT_PORT}
FRONTEND_PORT=${FRONTEND_PORT}

ZENTRA_JWT_SECRET=${JWT_SECRET}
ZENTRA_ALLOW_INSECURE_JWT=0

ZENTRA_MASTER_KEY=${MASTER_KEY}

ZENTRA_ALLOW_ORIGINS=http://${EXTERNAL_IP}:${FRONTEND_PORT},http://${INTERNAL_IP}:${FRONTEND_PORT},http://localhost:${FRONTEND_PORT}
ZENTRA_BIND_ADDR=0.0.0.0

ZENTRA_LOG_LEVEL=info
ZENTRA_COLLECTION_INTERVAL=60
ZENTRA_DEMO_MODE=false

ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
RUST_LOG=info
ENVEOF

chmod 600 "${DEPLOY_DIR}/.env"
ok ".env written to ${DEPLOY_DIR}/.env"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 6 — Build Rust backend
# ═══════════════════════════════════════════════════════════════════════════
info "Step 6/11: Building Rust backend (this may take 5-15 minutes)..."
info "Build artifacts → ${BUILD_DIR}  (to save space on root volume)"

# Clean previous build artifacts (frees multi-GB of space)
if [ -d "${DEPLOY_DIR}/target" ]; then
    info "Removing previous build cache: ${DEPLOY_DIR}/target"
    rm -rf "${DEPLOY_DIR}/target"
    ok "Old build cache cleaned (${DEPLOY_DIR}/target)"
fi
if [ -d "${BUILD_DIR}" ]; then
    info "Removing previous build directory: ${BUILD_DIR}"
    rm -rf "${BUILD_DIR}"
    ok "Previous build cleaned (${BUILD_DIR})"
fi
mkdir -p "${BUILD_DIR}"
cd "${PROJECT_DIR}"

# Point cargo at /mnt/data so the multi-GB target/ dir stays off root
export CARGO_TARGET_DIR="${BUILD_DIR}"
# Set SQLX_OFFLINE so build doesn't need live DB
export SQLX_OFFLINE=true
# Source .env for build
set -a
# shellcheck disable=SC1090
source "${DEPLOY_DIR}/.env"
set +a

cargo build --release --bin zentra-server 2>&1 | tee -a "${LOG_FILE}"

if [ ! -f "${BUILD_DIR}/release/zentra-server" ]; then
    fail "Backend build failed — zentra-server binary not found"
fi

cp "${BUILD_DIR}/release/zentra-server" "${DEPLOY_DIR}/bin/zentra-server"
chmod +x "${DEPLOY_DIR}/bin/zentra-server"
ok "zentra-server built and copied to ${DEPLOY_DIR}/bin/"

# Also build the agent if it exists
if cargo build --release --bin zentra-agent 2>&1 | tee -a "${LOG_FILE}"; then
    if [ -f "${BUILD_DIR}/release/zentra-agent" ]; then
        cp "${BUILD_DIR}/release/zentra-agent" "${DEPLOY_DIR}/bin/zentra-agent"
        chmod +x "${DEPLOY_DIR}/bin/zentra-agent"
        ok "zentra-agent built and copied"
    fi
fi

# ═══════════════════════════════════════════════════════════════════════════
# STEP 7 — Build React frontend
# ═══════════════════════════════════════════════════════════════════════════
info "Step 7/11: Building React frontend..."

cd "${PROJECT_DIR}/frontend"
npm ci --silent 2>&1 | tee -a "${LOG_FILE}" | tail -3
npm run build 2>&1 | tee -a "${LOG_FILE}" | tail -5

if [ ! -d "${PROJECT_DIR}/frontend/dist" ]; then
    fail "Frontend build failed — dist/ not found"
fi

# Copy dist to deploy dir
rm -rf "${DEPLOY_DIR}/frontend/dist"
cp -r "${PROJECT_DIR}/frontend/dist" "${DEPLOY_DIR}/frontend/dist"
ok "Frontend built and copied to ${DEPLOY_DIR}/frontend/dist/"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 8 — Run database migrations
# ═══════════════════════════════════════════════════════════════════════════
info "Step 8/11: Running database migrations..."

cd "${PROJECT_DIR}"
export DATABASE_URL="postgresql://${PG_USER}:${PG_PASS}@${PG_HOST}:${PG_PORT}/${PG_DB}"

# Copy migrations to deploy dir
cp -r "${PROJECT_DIR}/migrations" "${DEPLOY_DIR}/"

if command -v sqlx >/dev/null 2>&1; then
    sqlx migrate run --source ./migrations 2>&1 | tee -a "${LOG_FILE}" | tail -10 || {
        warn "sqlx migrate returned non-zero (tables may already exist)"
    }
    ok "Migrations applied via sqlx"
else
    # Fallback: apply SQL files in order directly via psql
    info "sqlx not found — applying migrations via psql..."
    for f in $(ls -1 "${PROJECT_DIR}/migrations/"*.sql | sort); do
        fname=$(basename "$f")
        info "  Applying: ${fname}"
        psql -h "${PG_HOST}" -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" -f "$f" >> "${LOG_FILE}" 2>&1 || true
    done
    ok "Migrations applied via psql"
fi

# ═══════════════════════════════════════════════════════════════════════════
# STEP 9 — Configure Nginx
# ═══════════════════════════════════════════════════════════════════════════
info "Step 9/11: Configuring Nginx reverse proxy..."

cat > /etc/nginx/sites-available/zentra <<NGINXEOF
# Rate limiting zones
limit_req_zone \$binary_remote_addr zone=login_limit:10m rate=5r/m;
limit_req_zone \$binary_remote_addr zone=api_limit:10m rate=30r/s;
limit_req_zone \$binary_remote_addr zone=otlp_limit:10m rate=100r/s;

server {
    listen ${FRONTEND_PORT};
    server_name ${EXTERNAL_IP} ${INTERNAL_IP} localhost _;

    root ${DEPLOY_DIR}/frontend/dist;
    index index.html;

    server_tokens off;
    client_max_body_size 2m;

    # Security headers
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-XSS-Protection "0" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # Frontend SPA
    location / {
        try_files \$uri \$uri/ /index.html;

        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2|ttf|eot)\$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }

        location = /index.html {
            add_header Cache-Control "no-store, no-cache, must-revalidate";
        }
    }

    # Block sensitive files
    location ~ /\.(env|git|gitignore|dockerignore) {
        deny all;
        return 404;
    }
    location ~ \.(toml|lock|rs|sql|sh|ps1)\$ {
        deny all;
        return 404;
    }

    # Login — strict rate limit
    location = /api/auth/login {
        limit_req zone=login_limit burst=3 nodelay;
        limit_req_status 429;
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_read_timeout 10s;
        proxy_connect_timeout 5s;
    }

    # API proxy
    location /api/ {
        limit_req zone=api_limit burst=60 nodelay;
        limit_req_status 429;
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        proxy_set_header Upgrade \$http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 60s;
        proxy_connect_timeout 10s;
    }

    # OTLP ingest
    location /v1/ {
        limit_req zone=otlp_limit burst=200 nodelay;
        proxy_pass http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header Content-Type \$http_content_type;
        proxy_read_timeout 30s;
        client_max_body_size 10m;
    }

    # Metrics (internal only)
    location /metrics {
        proxy_pass http://127.0.0.1:${API_PORT};
        allow 127.0.0.1;
        allow 10.0.0.0/8;
        deny all;
    }

    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml;
    gzip_min_length 1024;
    gzip_vary on;

    access_log ${DEPLOY_DIR}/logs/nginx_access.log;
    error_log  ${DEPLOY_DIR}/logs/nginx_error.log warn;
}
NGINXEOF

# Enable site, disable default
rm -f /etc/nginx/sites-enabled/default
ln -sf /etc/nginx/sites-available/zentra /etc/nginx/sites-enabled/zentra

# Verify config
nginx -t >> "${LOG_FILE}" 2>&1 || fail "Nginx config test failed"
ok "Nginx configured on port ${FRONTEND_PORT}"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 10 — Systemd services
# ═══════════════════════════════════════════════════════════════════════════
info "Step 10/11: Creating systemd services..."

# Set ownership
chown -R "${SERVICE_USER}:${SERVICE_USER}" "${DEPLOY_DIR}"

# ── zentra-api.service ──
cat > /etc/systemd/system/zentra-api.service <<SVCEOF
[Unit]
Description=Zentra API Server
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_USER}
WorkingDirectory=${DEPLOY_DIR}
EnvironmentFile=${DEPLOY_DIR}/.env
ExecStart=${DEPLOY_DIR}/bin/zentra-server
Restart=always
RestartSec=5
StandardOutput=append:${DEPLOY_DIR}/logs/api.log
StandardError=append:${DEPLOY_DIR}/logs/api.log
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
SVCEOF

# ── zentra-agent.service (if binary exists) ──
if [ -f "${DEPLOY_DIR}/bin/zentra-agent" ]; then
cat > /etc/systemd/system/zentra-agent.service <<AGENTEOF
[Unit]
Description=Zentra Monitoring Agent
After=network.target zentra-api.service
Wants=zentra-api.service

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_USER}
WorkingDirectory=${DEPLOY_DIR}
EnvironmentFile=${DEPLOY_DIR}/.env
ExecStart=${DEPLOY_DIR}/bin/zentra-agent
Restart=always
RestartSec=5
StandardOutput=append:${DEPLOY_DIR}/logs/agent.log
StandardError=append:${DEPLOY_DIR}/logs/agent.log
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
AGENTEOF
fi

systemctl daemon-reload

# Enable and start services
systemctl enable zentra-api >> "${LOG_FILE}" 2>&1
systemctl restart zentra-api
ok "zentra-api service started"

if [ -f /etc/systemd/system/zentra-agent.service ]; then
    systemctl enable zentra-agent >> "${LOG_FILE}" 2>&1
    systemctl restart zentra-agent
    ok "zentra-agent service started"
fi

systemctl restart nginx
ok "Nginx restarted"

# ═══════════════════════════════════════════════════════════════════════════
# STEP 11 — Health check
# ═══════════════════════════════════════════════════════════════════════════
info "Step 11/11: Running health checks..."

sleep 5

HEALTH_OK=false
for i in 1 2 3 4 5 6 7 8 9 10; do
    if curl -sf "http://127.0.0.1:${API_PORT}/api/health" >/dev/null 2>&1; then
        HEALTH_OK=true
        break
    fi
    echo -n "."
    sleep 3
done
echo ""

if [ "${HEALTH_OK}" = true ]; then
    ok "API health check PASSED"
else
    warn "API health check failed — check logs: journalctl -u zentra-api -n 50"
    warn "Or: cat ${DEPLOY_DIR}/logs/api.log | tail -50"
fi

# Check nginx/frontend
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

echo ""
echo ""
echo "======================================================================"
echo "  Zentra — Setup Complete!"
echo "======================================================================"
echo ""
echo "  WEB UI (external):  http://${EXTERNAL_IP}:${FRONTEND_PORT}"
echo "  WEB UI (internal):  http://${INTERNAL_IP}:${FRONTEND_PORT}"
echo "  API endpoint:       http://${INTERNAL_IP}:${API_PORT}/api/health"
echo "  Login:              admin@Zentra.local / changeme"
echo ""
echo "  SERVICES:"
echo "    zentra-api:   ${API_STATUS}"
echo "    nginx:        ${NGINX_STATUS}"
echo ""
echo "  COMMANDS:"
echo "    Status:       systemctl status zentra-api"
echo "    Logs (API):   journalctl -u zentra-api -f"
echo "    Logs (file):  tail -f ${DEPLOY_DIR}/logs/api.log"
echo "    Restart:      systemctl restart zentra-api nginx"
echo "    Rebuild:      cd ${PROJECT_DIR} && cargo build --release --bin zentra-server"
echo ""
echo "  CONFIG:           ${DEPLOY_DIR}/.env"
echo "  NGINX:            /etc/nginx/sites-available/zentra"
echo "  SETUP LOG:        ${LOG_FILE}"
echo ""
echo "  IMPORTANT: Change the default admin password after first login!"
echo "======================================================================"
echo ""
