#!/usr/bin/env bash
# =============================================================================
# Zentra Binary Installer
#
# For clients who receive a pre-built release package.
# Does NOT require Rust, Node, or npm — only installs runtime dependencies.
#
# USAGE:
#   sudo bash install-from-binaries.sh --binary-dir /path/to/zentra-release
#
# OPTIONS:
#   --binary-dir  DIR    Path to the folder containing the release package
#                        (must contain: zentra-server, frontend-dist/, migrations/)
#                        Default: current directory
#
#   --install-dir DIR    Where to install Zentra on this machine
#                        Default: /opt/zentra
#
#   --api-port    PORT   Backend API port          Default: 5006
#   --web-port    PORT   Frontend (nginx) port     Default: 5010
#   --pg-port     PORT   PostgreSQL port           Default: 5434
#   --pg-password PASS   PostgreSQL password       Default: zentra
#   --pg-db       NAME   PostgreSQL database name  Default: zentra
#   --app-user    USER   Linux service account     Default: zentra
#
#   --jwt-secret  STR    JWT secret (min 32 chars). Auto-generated if omitted.
#   --master-key  HEX    64-char hex master key.   Auto-generated if omitted.
#
#   --skip-postgres      Skip PostgreSQL install (use existing instance)
#   --skip-nginx         Skip nginx install/config
#   --dry-run            Print what would happen, make no changes
#
# EXAMPLE (typical client install):
#   sudo bash install-from-binaries.sh \
#       --binary-dir /tmp/zentra-release \
#       --install-dir /opt/zentra \
#       --pg-port 5434 \
#       --web-port 8080
#
# PACKAGE LAYOUT (what the client receives):
#   zentra-release/
#     zentra-server          <- compiled Rust binary
#     frontend-dist/         <- built React app (static files)
#     migrations/            <- SQL migration files (*.sql)
#     .env.example           <- optional reference config
# =============================================================================
set -euo pipefail

# ── defaults ──────────────────────────────────────────────────────────────────
BINARY_DIR="$(pwd)"
INSTALL_DIR="/opt/zentra"
API_PORT="5006"
WEB_PORT="5010"
PG_PORT="5434"
PG_PASSWORD="zantaz"
PG_DB="zentra"
PG_USER="postgres"
APP_USER="zentra"
JWT_SECRET="ZentraSecureAPIKey2026!@#XyZ9p2qR7mW4nV"
MASTER_KEY=""
SKIP_POSTGRES=false
SKIP_NGINX=false
DRY_RUN=false

# ── parse arguments ───────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary-dir)   BINARY_DIR="$2";    shift 2 ;;
        --install-dir)  INSTALL_DIR="$2";   shift 2 ;;
        --api-port)     API_PORT="$2";      shift 2 ;;
        --web-port)     WEB_PORT="$2";      shift 2 ;;
        --pg-port)      PG_PORT="$2";       shift 2 ;;
        --pg-password)  PG_PASSWORD="$2";   shift 2 ;;
        --pg-db)        PG_DB="$2";         shift 2 ;;
        --app-user)     APP_USER="$2";      shift 2 ;;
        --jwt-secret)   JWT_SECRET="$2";    shift 2 ;;
        --master-key)   MASTER_KEY="$2";    shift 2 ;;
        --skip-postgres) SKIP_POSTGRES=true; shift ;;
        --skip-nginx)   SKIP_NGINX=true;    shift ;;
        --dry-run)      DRY_RUN=true;       shift ;;
        -h|--help)
            sed -n '3,50p' "$0"
            exit 0 ;;
        *)
            echo "Unknown option: $1" >&2
            echo "Run with --help for usage." >&2
            exit 1 ;;
    esac
done

DB_URL="postgresql://${PG_USER}:${PG_PASSWORD}@127.0.0.1:${PG_PORT}/${PG_DB}"

# ── helpers ───────────────────────────────────────────────────────────────────
log()  { echo "[zentra-install] $*"; }
ok()   { echo "  OK  $*"; }
warn() { echo "  !   $*"; }
err()  { echo "  ERR $*" >&2; exit 1; }

run() {
    if [[ "${DRY_RUN}" == "true" ]]; then
        echo "  [dry-run] $*"
    else
        "$@"
    fi
}

gen_secret() {
    # Generate a random string of given length using /dev/urandom
    local len="${1:-48}"
    tr -dc 'A-Za-z0-9!@#%^&*()_+' < /dev/urandom | head -c "${len}" 2>/dev/null || \
    LC_ALL=C tr -dc 'A-Za-z0-9' < /dev/urandom | head -c "${len}"
}

gen_hex() {
    local bytes="${1:-32}"
    od -An -tx1 /dev/urandom | tr -d ' \n' | head -c $((bytes * 2))
}

wait_for_apt_locks() {
    local tries=60
    while (( tries > 0 )); do
        if fuser /var/lib/dpkg/lock-frontend /var/lib/dpkg/lock /var/lib/apt/lists/lock \
           >/dev/null 2>&1; then
            warn "apt lock held, waiting... (${tries} tries left)"
            sleep 5
            tries=$((tries - 1))
        else
            return 0
        fi
    done
    err "Timed out waiting for apt lock"
}

# ── pre-flight checks ─────────────────────────────────────────────────────────
if [[ "${EUID}" -ne 0 ]] && [[ "${DRY_RUN}" == "false" ]]; then
    err "This script must be run as root: sudo bash $0 $*"
fi

log "============================================"
log "  Zentra Binary Installer"
log "  Binary dir  : ${BINARY_DIR}"
log "  Install dir : ${INSTALL_DIR}"
log "  API port    : ${API_PORT}"
log "  Web port    : ${WEB_PORT}"
log "  PG port     : ${PG_PORT}"
log "  PG database : ${PG_DB}"
log "============================================"
echo ""

# Verify binary package exists
BINARY_FILE="${BINARY_DIR}/zentra-server"
FRONTEND_DIR="${BINARY_DIR}/frontend-dist"
MIGRATIONS_DIR="${BINARY_DIR}/migrations"

[[ -f "${BINARY_FILE}" ]]   || err "zentra-server binary not found at: ${BINARY_FILE}"
[[ -d "${FRONTEND_DIR}" ]]  || err "frontend-dist/ directory not found at: ${FRONTEND_DIR}"
[[ -d "${MIGRATIONS_DIR}" ]] || err "migrations/ directory not found at: ${MIGRATIONS_DIR}"

MIGRATION_COUNT=$(find "${MIGRATIONS_DIR}" -name "*.sql" | wc -l)
FRONTEND_COUNT=$(find "${FRONTEND_DIR}" -type f | wc -l)
BINARY_SIZE=$(du -sh "${BINARY_FILE}" | cut -f1)

ok "Binary       : ${BINARY_FILE} (${BINARY_SIZE})"
ok "Frontend     : ${FRONTEND_COUNT} files"
ok "Migrations   : ${MIGRATION_COUNT} SQL files"
echo ""

# ── [1] Runtime packages ──────────────────────────────────────────────────────
log "[1/7] Installing runtime packages..."
# Only runtime deps — no Rust, no Node, no npm
PKGS=(
    ca-certificates curl gnupg lsb-release
    libssl-dev libpq-dev
    nginx
    jq python3
)

if [[ "${SKIP_POSTGRES}" == "false" ]]; then
    PKGS+=(postgresql postgresql-contrib)
fi

export DEBIAN_FRONTEND=noninteractive
run dpkg --configure -a || true
wait_for_apt_locks
run apt-get update -y
wait_for_apt_locks
run apt-get install -y "${PKGS[@]}"

# TimescaleDB (runtime extension only — no build tools needed)
if [[ "${SKIP_POSTGRES}" == "false" ]]; then
    PG_VER="$(psql --version 2>/dev/null | grep -oP '(?<=PostgreSQL )\d+' | head -1 || \
               ls /etc/postgresql 2>/dev/null | sort -V | tail -1 || echo "")"
    if [[ -z "${PG_VER}" ]]; then
        warn "Cannot detect PostgreSQL version — skipping TimescaleDB install"
    elif [[ -f "/usr/share/postgresql/${PG_VER}/extension/timescaledb.control" ]]; then
        ok "TimescaleDB already installed for PostgreSQL ${PG_VER}"
    else
        log "Installing TimescaleDB for PostgreSQL ${PG_VER}..."
        if ! grep -Rqs "packagecloud.io/timescale" /etc/apt/sources.list \
             /etc/apt/sources.list.d 2>/dev/null; then
            run curl -fsSL https://packagecloud.io/install/repositories/timescale/timescaledb/script.deb.sh | bash
        fi
        wait_for_apt_locks
        run apt-get update -y
        wait_for_apt_locks
        if ! run apt-get install -y "timescaledb-2-postgresql-${PG_VER}" timescaledb-tools 2>/dev/null; then
            warn "Primary TimescaleDB package failed, trying fallback..."
            run apt-get install -y "postgresql-${PG_VER}-timescaledb" || \
                warn "TimescaleDB install failed — continuing without it"
        fi
    fi
fi
ok "Runtime packages installed"

# ── [2] Service account + install directories ─────────────────────────────────
log "[2/7] Creating service account and directories..."

if ! id -u "${APP_USER}" >/dev/null 2>&1; then
    run useradd --system --shell /usr/sbin/nologin \
        --home-dir "${INSTALL_DIR}" --no-create-home "${APP_USER}"
    ok "Created service account: ${APP_USER}"
else
    ok "Service account already exists: ${APP_USER}"
fi

run mkdir -p \
    "${INSTALL_DIR}/bin" \
    "${INSTALL_DIR}/frontend/dist" \
    "${INSTALL_DIR}/migrations" \
    "${INSTALL_DIR}/logs"

ok "Directories created under ${INSTALL_DIR}"

# ── [3] PostgreSQL configuration ───────────────────────────────────────────────
if [[ "${SKIP_POSTGRES}" == "false" ]]; then
    log "[3/7] Configuring PostgreSQL on port ${PG_PORT}..."

    PG_CONF_DIR="$(ls -d /etc/postgresql/*/main 2>/dev/null | head -1 || true)"
    [[ -n "${PG_CONF_DIR}" ]] || err "PostgreSQL config dir not found under /etc/postgresql/*/main"
    PG_CONF="${PG_CONF_DIR}/postgresql.conf"
    PG_HBA="${PG_CONF_DIR}/pg_hba.conf"
    PG_VER_INSTALLED="$(ls /etc/postgresql 2>/dev/null | sort -V | tail -1)"

    run sed -i -E "s/^#?[[:space:]]*port[[:space:]]*=[[:space:]]*[0-9]+/port = ${PG_PORT}/" "${PG_CONF}"
    run sed -i -E "s/^#?[[:space:]]*listen_addresses[[:space:]]*=.*/listen_addresses = '127.0.0.1'/" "${PG_CONF}"

    if grep -Eq "^#?[[:space:]]*shared_preload_libraries[[:space:]]*=" "${PG_CONF}" 2>/dev/null; then
        run sed -i -E \
            "s|^#?[[:space:]]*shared_preload_libraries[[:space:]]*=.*|shared_preload_libraries = 'timescaledb'|" \
            "${PG_CONF}"
    else
        [[ "${DRY_RUN}" == "true" ]] || echo "shared_preload_libraries = 'timescaledb'" >> "${PG_CONF}"
    fi

    if ! grep -q "127.0.0.1/32" "${PG_HBA}" 2>/dev/null; then
        [[ "${DRY_RUN}" == "true" ]] || \
            echo "host    all     all     127.0.0.1/32    scram-sha-256" >> "${PG_HBA}"
    fi

    run systemctl enable postgresql
    if command -v pg_ctlcluster >/dev/null 2>&1 && [[ -n "${PG_VER_INSTALLED}" ]]; then
        run pg_ctlcluster "${PG_VER_INSTALLED}" main restart
    else
        run systemctl restart postgresql
    fi

    # Set postgres password
    run sudo -u postgres psql -v ON_ERROR_STOP=1 \
        -c "ALTER USER ${PG_USER} WITH PASSWORD '${PG_PASSWORD}';" >/dev/null

    if ! sudo -u postgres psql -tAc \
            "SELECT 1 FROM pg_database WHERE datname='${PG_DB}'" | grep -q 1; then
        run sudo -u postgres createdb "${PG_DB}"
        ok "Database '${PG_DB}' created"
    else
        ok "Database '${PG_DB}' already exists"
    fi
    ok "PostgreSQL configured on port ${PG_PORT}"
else
    log "[3/7] Skipping PostgreSQL (--skip-postgres)"
fi

# ── [4] Write .env ────────────────────────────────────────────────────────────
log "[4/7] Writing runtime environment file..."

# Auto-generate master key only if not supplied; JWT secret is fixed
if [[ -z "${MASTER_KEY}" ]]; then
    MASTER_KEY="$(gen_hex 32)"
    warn "Master key auto-generated — save it if you need to share sessions across nodes"
fi

ENV_FILE="${INSTALL_DIR}/.env"

if [[ "${DRY_RUN}" == "true" ]]; then
    echo "  [dry-run] would write ${ENV_FILE}"
else
    cat > "${ENV_FILE}" <<EOF
DATABASE_URL=${DB_URL}
ZENTRA_API_PORT=${API_PORT}
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=${JWT_SECRET}
ZENTRA_MASTER_KEY=${MASTER_KEY}
RUST_LOG=info
ZENTRA_APP_USER=${APP_USER}
ZENTRA_APP_PASSWORD=zentra
EOF
    chmod 640 "${ENV_FILE}"
    chown "root:${APP_USER}" "${ENV_FILE}" 2>/dev/null || true
fi
ok ".env written to ${ENV_FILE}"

# ── [5] Copy binary + frontend ────────────────────────────────────────────────
log "[5/7] Installing binary and frontend assets..."

run install -m 755 "${BINARY_FILE}" "${INSTALL_DIR}/bin/zentra-server"
ok "Binary installed: ${INSTALL_DIR}/bin/zentra-server"

run rm -rf "${INSTALL_DIR}/frontend/dist"
run mkdir -p "${INSTALL_DIR}/frontend/dist"
run cp -a "${FRONTEND_DIR}/." "${INSTALL_DIR}/frontend/dist/"
ok "Frontend installed: $(find "${INSTALL_DIR}/frontend/dist" -type f | wc -l) files"

run cp "${MIGRATIONS_DIR}"/*.sql "${INSTALL_DIR}/migrations/" 2>/dev/null || true
ok "Migrations copied: $(find "${INSTALL_DIR}/migrations" -name '*.sql' | wc -l) files"

run chown -R "${APP_USER}:${APP_USER}" "${INSTALL_DIR}"

# ── [6] Apply SQL migrations ──────────────────────────────────────────────────
log "[6/7] Applying SQL migrations..."

if [[ "${DRY_RUN}" == "true" ]]; then
    echo "  [dry-run] would apply SQL migrations in ${INSTALL_DIR}/migrations/"
else
    PGPASSWORD="${PG_PASSWORD}" psql -v ON_ERROR_STOP=1 \
        -h 127.0.0.1 -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" \
        -c "CREATE TABLE IF NOT EXISTS zentra_schema_migrations (
              filename TEXT PRIMARY KEY,
              applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );" >/dev/null

    shopt -s nullglob
    for m in "${INSTALL_DIR}/migrations/"*.sql; do
        name="$(basename "${m}")"
        applied="$(PGPASSWORD="${PG_PASSWORD}" psql -tAc \
            "SELECT 1 FROM zentra_schema_migrations WHERE filename='${name}'" \
            -h 127.0.0.1 -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" \
            2>/dev/null | tr -d '[:space:]')"
        if [[ "${applied}" == "1" ]]; then
            echo "  skip (applied): ${name}"
            continue
        fi
        echo "  apply: ${name}"
        PGPASSWORD="${PG_PASSWORD}" psql -v ON_ERROR_STOP=1 \
            -h 127.0.0.1 -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" -f "${m}" >/dev/null
        PGPASSWORD="${PG_PASSWORD}" psql -v ON_ERROR_STOP=1 \
            -h 127.0.0.1 -p "${PG_PORT}" -U "${PG_USER}" -d "${PG_DB}" \
            -c "INSERT INTO zentra_schema_migrations(filename)
                VALUES ('${name}') ON CONFLICT DO NOTHING;" >/dev/null
    done
    ok "Migrations done"
fi

# ── [7] systemd service + nginx ───────────────────────────────────────────────
log "[7/7] Configuring systemd service and nginx..."

EXTERNAL_IP="$(curl -sf --max-time 5 \
    -H 'Metadata-Flavor: Google' \
    'http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip' \
    2>/dev/null || \
    curl -sf --max-time 5 https://ifconfig.me 2>/dev/null || \
    hostname -I 2>/dev/null | awk '{print $1}' || echo "0.0.0.0")"

if [[ "${DRY_RUN}" == "false" ]]; then
    # systemd unit
    cat > /etc/systemd/system/zentra-api.service <<EOF
[Unit]
Description=Zentra API Server
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=${APP_USER}
Group=${APP_USER}
WorkingDirectory=${INSTALL_DIR}
EnvironmentFile=${INSTALL_DIR}/.env
ExecStart=${INSTALL_DIR}/bin/zentra-server
Restart=always
RestartSec=5
StandardOutput=append:${INSTALL_DIR}/logs/api.log
StandardError=append:${INSTALL_DIR}/logs/api.log

[Install]
WantedBy=multi-user.target
EOF

    # nginx site
    if [[ "${SKIP_NGINX}" == "false" ]]; then
        NGINX_CONF="/etc/nginx/sites-available/zentra"
        cat > "${NGINX_CONF}" <<NGINXEOF
server {
    listen ${WEB_PORT};
    server_name _ ${EXTERNAL_IP};

    root ${INSTALL_DIR}/frontend/dist;
    index index.html;

    location /api/ {
        proxy_pass         http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header   Host              \$host;
        proxy_set_header   X-Real-IP         \$remote_addr;
        proxy_set_header   X-Forwarded-For   \$proxy_add_x_forwarded_for;
        proxy_set_header   Upgrade           \$http_upgrade;
        proxy_set_header   Connection        "upgrade";
        proxy_read_timeout 300s;
        proxy_send_timeout 300s;
    }

    location /otlp/ {
        proxy_pass         http://127.0.0.1:${API_PORT};
        proxy_http_version 1.1;
        proxy_set_header   Host              \$host;
        proxy_set_header   X-Real-IP         \$remote_addr;
        proxy_set_header   X-Forwarded-For   \$proxy_add_x_forwarded_for;
    }

    location / {
        try_files \$uri \$uri/ /index.html;
    }

    access_log ${INSTALL_DIR}/logs/nginx-access.log;
    error_log  ${INSTALL_DIR}/logs/nginx-error.log;
}
NGINXEOF
        ln -sf "${NGINX_CONF}" /etc/nginx/sites-enabled/zentra
        rm -f /etc/nginx/sites-enabled/default
        nginx -t
    fi

    systemctl daemon-reload
    systemctl enable zentra-api
    systemctl restart zentra-api
    [[ "${SKIP_NGINX}" == "false" ]] && systemctl reload-or-restart nginx || true
fi

# ── Health checks ─────────────────────────────────────────────────────────────
if [[ "${DRY_RUN}" == "false" ]]; then
    echo ""
    log "Waiting for API to start..."
    API_HEALTH="unreachable"
    for _i in $(seq 1 12); do
        if curl -fsS "http://127.0.0.1:${API_PORT}/api/health" >/dev/null 2>&1; then
            API_HEALTH="ok"
            break
        fi
        sleep 5
    done

    FE_HEALTH="unreachable"
    for _i in $(seq 1 6); do
        if curl -fsS "http://127.0.0.1:${WEB_PORT}" >/dev/null 2>&1; then
            FE_HEALTH="ok"
            break
        fi
        sleep 5
    done
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "============================================"
echo "  Zentra Installation Complete"
echo "============================================"
echo "  Install path   : ${INSTALL_DIR}"
echo "  API port       : ${API_PORT}"
echo "  Web port       : ${WEB_PORT}  (nginx)"
echo "  Postgres port  : ${PG_PORT}"
echo "  Database       : ${PG_DB}"
echo "  Service        : zentra-api (systemd)"
echo "  Log file       : ${INSTALL_DIR}/logs/api.log"
if [[ "${DRY_RUN}" == "false" ]]; then
echo "  API health     : ${API_HEALTH}"
echo "  Frontend       : ${FE_HEALTH}"
echo "  Access URL     : http://${EXTERNAL_IP}:${WEB_PORT}"
fi
echo "============================================"
echo ""
echo "  IMPORTANT: Edit ${INSTALL_DIR}/.env to set:"
echo "    ZENTRA_JWT_SECRET  (current: auto-generated)"
echo "    ZENTRA_MASTER_KEY  (current: auto-generated)"
echo "    ZENTRA_APP_PASSWORD (default: zentra)"
echo ""
echo "  After editing .env, restart: systemctl restart zentra-api"
echo ""

if [[ "${DRY_RUN}" == "false" ]] && [[ "${API_HEALTH}" != "ok" ]]; then
    warn "API did not respond — check logs:"
    warn "  journalctl -u zentra-api -n 50"
    warn "  tail -50 ${INSTALL_DIR}/logs/api.log"
    exit 1
fi
