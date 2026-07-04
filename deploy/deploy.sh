#!/usr/bin/env bash
# =============================================================================
# Zentra — Production Deployment Script
# Pushes source code to the server and builds EVERYTHING on the server.
# No local builds — macOS ARM cannot cross-compile for Linux x86_64.
#
# Target: root@10.88.11.162:/store/Zentra
# Usage:  ./deploy/deploy.sh [--backend-only | --frontend-only | --full]
# =============================================================================
set -euo pipefail

# Source port configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true

# ── Server config (set via env vars, .env, or deploy/deploy.env) ──
[ -f "${SCRIPT_DIR}/deploy.env" ] && source "${SCRIPT_DIR}/deploy.env"
SERVER_HOST="${SERVER_HOST:-10.88.11.162}"
SERVER_PORT="${SERVER_PORT:-22}"
SERVER_USER="${SERVER_USER:-root}"
SERVER_PASS="${SERVER_PASS:-}"
DEPLOY_PATH="${DEPLOY_PATH:-/store/Zentra}"
BACKEND_PORT="${ZENTRA_API_PORT:-5006}"
FRONTEND_PORT="${ZENTRA_FRONTEND_PORT:-5010}"

MODE="${1:---full}"

# SSH / SCP / rsync helper — uses sshpass if password is set
SSH_OPTS="-o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 -p ${SERVER_PORT}"
if command -v sshpass >/dev/null 2>&1 && [ -n "${SERVER_PASS}" ]; then
  export SSHPASS="${SERVER_PASS}"
  SSH="sshpass -e ssh ${SSH_OPTS} ${SERVER_USER}@${SERVER_HOST}"
  RSYNC_RSH="sshpass -e ssh ${SSH_OPTS}"
else
  SSH="ssh ${SSH_OPTS} ${SERVER_USER}@${SERVER_HOST}"
  RSYNC_RSH="ssh ${SSH_OPTS}"
fi

echo "======================================================"
echo "  Zentra Deployment (server-side build)"
echo "  Target: ${SERVER_USER}@${SERVER_HOST}:${DEPLOY_PATH}"
echo "  Mode:   ${MODE}"
echo "======================================================"

# ------------------------------------------------------------------
# Step 1: Sync source code to server via rsync
# ------------------------------------------------------------------
sync_source() {
  echo ""
  echo ">>> [1/4] Syncing source code to ${SERVER_HOST}:${DEPLOY_PATH}..."

  # Create required directories on server
  ${SSH} "mkdir -p ${DEPLOY_PATH}/{bin,logs,backups}"

  # macOS: disable ._* resource forks in tar/rsync
  export COPYFILE_DISABLE=1

  # rsync entire project except build artifacts and git
  rsync -az --delete \
    --exclude='.git/' \
    --exclude='target/' \
    --exclude='node_modules/' \
    --exclude='frontend/dist/' \
    --exclude='bin/' \
    --exclude='logs/' \
    --exclude='backups/' \
    --exclude='.DS_Store' \
    --exclude='._*' \
    --exclude='*.log' \
    --exclude='.env.local' \
    -e "${RSYNC_RSH}" \
    "${PROJECT_ROOT}/" \
    "${SERVER_USER}@${SERVER_HOST}:${DEPLOY_PATH}/"

  # Also push .env (excluded by .gitignore, handled separately)
  if [ -f "${PROJECT_ROOT}/.env" ]; then
    rsync -az -e "${RSYNC_RSH}" \
      "${PROJECT_ROOT}/.env" \
      "${SERVER_USER}@${SERVER_HOST}:${DEPLOY_PATH}/.env"
  fi

  echo ">>> [1/4] Source sync complete."
}

# ------------------------------------------------------------------
# Step 2: Build backend ON THE SERVER
# ------------------------------------------------------------------
build_backend_remote() {
  echo ""
  echo ">>> [2/4] Building Zentra API on server (release)..."

  # Backup current binary
  ${SSH} "
    if [ -f ${DEPLOY_PATH}/bin/zentra-server ]; then
      cp ${DEPLOY_PATH}/bin/zentra-server ${DEPLOY_PATH}/bin/zentra-server.bak.\$(date +%Y%m%d_%H%M%S)
      echo '  Binary backup created'
      # Keep only last 3 backups
      ls -t ${DEPLOY_PATH}/bin/zentra-server.bak.* 2>/dev/null | tail -n +4 | xargs rm -f 2>/dev/null || true
    fi
  "

  # Clean macOS resource fork files that break cargo
  ${SSH} "find ${DEPLOY_PATH}/crates ${DEPLOY_PATH}/plugins -name '._*' -delete 2>/dev/null || true"

  # Build server + agent on the server, then stop service → replace binaries → (restart in step 4)
  ${SSH} "
    cd ${DEPLOY_PATH}
    source .env 2>/dev/null || true
    export SQLX_OFFLINE=true

    # Build API server
    cargo build --release --bin zentra-server 2>&1 | tail -5
    # Stop service so binary isn't locked ('Text file busy')
    systemctl stop zentra-api 2>/dev/null || pkill -f zentra-server 2>/dev/null || true
    sleep 1
    rm -f bin/zentra-server
    cp target/release/zentra-server bin/zentra-server
    chmod +x bin/zentra-server
    echo '  Backend build complete — binary at bin/zentra-server'

    # Build agent binary and publish to download directory
    cargo build --release --bin zentra-agent 2>&1 | tail -3
    mkdir -p bin/dl
    cp target/release/zentra-agent bin/dl/zentra-agent-linux-x86_64
    chmod +x bin/dl/zentra-agent-linux-x86_64
    echo '  Agent build complete — binary at bin/dl/zentra-agent-linux-x86_64'
  "

  echo ">>> [2/4] Backend build complete."
}

# ------------------------------------------------------------------
# Step 3: Build frontend ON THE SERVER
# ------------------------------------------------------------------
build_frontend_remote() {
  echo ""
  echo ">>> [3/4] Building Zentra frontend on server..."

  ${SSH} "
    cd ${DEPLOY_PATH}/frontend
    npm ci --silent 2>&1 | tail -3
    npm run build 2>&1 | tail -5
    echo '  Frontend build complete — dist ready at frontend/dist/'
  "

  echo ">>> [3/4] Frontend build complete."
}

# ------------------------------------------------------------------
# Step 4: Run migrations, restart services, health check
# ------------------------------------------------------------------
restart_and_verify() {
  echo ""
  echo ">>> [4/4] Migrations, restart, and health check..."

  # Backup database before migration
  ${SSH} "
    mkdir -p ${DEPLOY_PATH}/backups
    PGPASSWORD=\${PGPASSWORD:-} pg_dump -h localhost -p 5434 -U postgres -d zentra -Fc \
      -f ${DEPLOY_PATH}/backups/zentra_pre_deploy_\$(date +%Y%m%d_%H%M%S).dump 2>/dev/null \
      || echo '  WARNING: Database backup skipped (pg_dump not available or PGPASSWORD not set)'
    # Keep only last 5 backups
    ls -t ${DEPLOY_PATH}/backups/zentra_pre_deploy_*.dump 2>/dev/null | tail -n +6 | xargs rm -f 2>/dev/null || true
  "

  # Run migrations
  ${SSH} "
    cd ${DEPLOY_PATH}
    source .env 2>/dev/null || true
    if command -v sqlx >/dev/null 2>&1; then
      sqlx migrate run --database-url \"\${DATABASE_URL}\" --source ./migrations 2>&1 || echo '  WARNING: Migration runner returned non-zero (tables may already exist)'
    else
      echo '  INFO: sqlx CLI not installed — migrations run by the app on startup'
    fi
  "

  # Restart services
  ${SSH} "
    # Restart API service
    if systemctl list-unit-files zentra-api.service >/dev/null 2>&1; then
      systemctl restart zentra-api
      echo '  zentra-api restarted via systemd'
    else
      pkill -f 'zentra-server' 2>/dev/null || true
      sleep 1
      cd ${DEPLOY_PATH}
      source .env 2>/dev/null || true
      nohup ${DEPLOY_PATH}/bin/zentra-server >> ${DEPLOY_PATH}/logs/api.log 2>&1 &
      disown
      echo '  zentra-server started (nohup)'
    fi

    # Sync nginx config and reload
    if [ -f ${DEPLOY_PATH}/deploy/nginx.conf ]; then
      cp ${DEPLOY_PATH}/deploy/nginx.conf /etc/nginx/conf.d/zentra.conf
      echo '  nginx config updated from deploy/nginx.conf'
    fi
    if systemctl is-active --quiet nginx 2>/dev/null; then
      nginx -t 2>&1 && (systemctl reload nginx 2>/dev/null || systemctl restart nginx) && echo '  nginx reloaded'
    fi
  "

  # Health check with retry
  ${SSH} "
    echo '  Running health check...'
    HEALTH_OK=false
    for i in 1 2 3 4 5; do
      sleep 2
      if curl -sf http://localhost:${BACKEND_PORT}/api/health | grep -q 'healthy'; then
        HEALTH_OK=true
        echo \"  Health check PASSED (attempt \$i)\"
        break
      fi
      echo \"  Health check attempt \$i failed, retrying...\"
    done

    if [ \"\$HEALTH_OK\" = false ]; then
      echo '  CRITICAL: Health check failed after 5 attempts!'
      echo '  Rolling back binary...'
      LATEST_BACKUP=\$(ls -t ${DEPLOY_PATH}/bin/zentra-server.bak.* 2>/dev/null | head -1)
      if [ -n \"\$LATEST_BACKUP\" ]; then
        systemctl stop zentra-api 2>/dev/null || true
        rm -f ${DEPLOY_PATH}/bin/zentra-server
        cp \"\$LATEST_BACKUP\" ${DEPLOY_PATH}/bin/zentra-server
        systemctl start zentra-api 2>/dev/null || true
        echo \"  Rolled back to \$LATEST_BACKUP\"
      fi
    fi
  "

  echo ">>> [4/4] Restart and verification complete."
}

# ------------------------------------------------------------------
# Main
# ------------------------------------------------------------------
case "${MODE}" in
  --backend-only)
    sync_source
    build_backend_remote
    restart_and_verify
    ;;
  --frontend-only)
    sync_source
    build_frontend_remote
    restart_and_verify
    ;;
  --full | *)
    sync_source
    build_backend_remote
    build_frontend_remote
    restart_and_verify
    ;;
esac

echo ""
echo "======================================================"
echo "  Deployment finished!"
echo "  API:      http://${SERVER_HOST}:${BACKEND_PORT}/api/health"
echo "  Frontend: http://${SERVER_HOST}:${FRONTEND_PORT}"
echo "======================================================"
