#!/bin/bash
# Zentra rebuild + restart script
# Usage:
#   ./restart-services.sh                 # rebuild everything + restart
#   ./restart-services.sh --backend-only  # rebuild API + agent + collector only
#   ./restart-services.sh --frontend-only # rebuild frontend only
#   ./restart-services.sh --restart-only  # restart services only (no rebuild)

set -euo pipefail

HOST="root@10.88.11.162"
REMOTE_DIR="/store/Zentra"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-}"

# ── SSH helpers ────────────────────────────────────────────────────────────────
SSH_OPTS="-p 22 -o StrictHostKeyChecking=no -o PubkeyAuthentication=no -o ConnectTimeout=10"
SSH_CMD="sshpass -p skyline ssh $SSH_OPTS $HOST"
SCP_CMD="sshpass -p skyline scp -P 22 -o StrictHostKeyChecking=no -o PubkeyAuthentication=no"

run_remote() { $SSH_CMD "$@"; }

# ── Logging helpers ────────────────────────────────────────────────────────────
info()  { echo ""; echo ">>> $*"; }
ok()    { echo "    ✓  $*"; }
fail()  { echo "    ✗  $*" >&2; }
die()   { fail "$*"; exit 1; }

# ── Step 1: Sync source ────────────────────────────────────────────────────────
sync_source() {
    info "Syncing source → $HOST:$REMOTE_DIR"
    if command -v rsync &>/dev/null; then
        sshpass -p skyline rsync -az \
            --exclude target \
            --exclude node_modules \
            --exclude .git \
            --exclude '*.log' \
            -e "ssh $SSH_OPTS" \
            "$SCRIPT_DIR/" "$HOST:$REMOTE_DIR/" \
        && ok "rsync complete" && return 0
    fi
    # Fallback: scp key files only
    $SCP_CMD -r \
        "$SCRIPT_DIR/crates" \
        "$SCRIPT_DIR/Cargo.toml" "$SCRIPT_DIR/Cargo.lock" \
        "$SCRIPT_DIR/frontend" \
        "$HOST:$REMOTE_DIR/" \
    && ok "scp complete" && return 0
    die "Source sync failed"
}

# ── Step 2a: Build backend binaries ───────────────────────────────────────────
build_backend() {
    info "Building + replacing backend binaries on server ..."
    run_remote "
        set -e
        cd $REMOTE_DIR

        # Build all three crates
        cargo build --release -p zentra-api     2>&1 | grep -E '^error|Finished|^warning.*unused' | grep -v '^warning' || { echo 'zentra-api build FAILED'; exit 1; }
        cargo build --release -p zentra-agent   2>&1 | grep -E '^error|Finished' || { echo 'zentra-agent build FAILED'; exit 1; }
        cargo build --release -p zentra-collector 2>&1 | grep -E '^error|Finished' || { echo 'zentra-collector build FAILED'; exit 1; }

        mkdir -p bin/dl

        # Back up running binary
        [ -f bin/zentra-server ] && cp -f bin/zentra-server bin/zentra-server.bak || true

        # Stop service so binary file is not locked
        systemctl stop zentra-api 2>/dev/null || true
        sleep 1

        # Copy — rollback on failure
        cp -f target/release/zentra-server bin/zentra-server \
            || { cp -f bin/zentra-server.bak bin/zentra-server 2>/dev/null
                 echo 'ROLLBACK: restored previous zentra-server'; exit 1; }

        cp -f target/release/zentra-agent     bin/dl/zentra-agent-linux-x86_64
        cp -f target/release/zentra-collector bin/dl/zentra-collector-linux-x86_64

        echo 'Binaries replaced OK'
    " || die "Backend build/replace failed"
    ok "API + agent + collector binaries updated"
}

# ── Step 2b: Build frontend ────────────────────────────────────────────────────
build_frontend() {
    info "Building frontend on server ..."
    run_remote "
        set -e
        cd $REMOTE_DIR/frontend
        npm ci --silent 2>/dev/null || true
        npm run build 2>&1 | tail -8
        echo 'Frontend build OK'
    " || die "Frontend build FAILED"
    ok "Frontend built"
}

# ── Step 3: Restart services ──────────────────────────────────────────────────
restart_services() {
    info "Restarting services ..."
    run_remote "
        set -e

        # Restart application services
        systemctl restart zentra-api zentra-frontend

        # nginx: reload if running (avoids 'address already in use'), start if stopped
        if systemctl is-active --quiet nginx; then
            systemctl reload nginx
        else
            # Kill anything holding our port, then start
            fuser -k 5010/tcp 2>/dev/null || true
            sleep 1
            systemctl start nginx
        fi

        # Verify all three are active
        for svc in zentra-api zentra-frontend nginx; do
            if systemctl is-active --quiet \$svc; then
                echo \"  ✓ \$svc active\"
            else
                echo \"  ✗ \$svc FAILED\" >&2
                systemctl status \$svc --no-pager -l | tail -10 >&2
                exit 1
            fi
        done
    " || die "Service restart failed — check journalctl on the server"
    ok "All services running"
}

# ── Step 4: Health check ──────────────────────────────────────────────────────
health_check() {
    info "Running health check ..."
    for i in $(seq 1 8); do
        HTTP=$(curl -sf --max-time 5 "http://10.88.11.162:5006/api/health" 2>/dev/null || true)
        if [ -n "$HTTP" ]; then
            ok "API health check passed (attempt $i)"
            return 0
        fi
        echo "    waiting ... ($i/8)"
        sleep 4
    done
    die "API health check failed after 8 attempts — check: journalctl -u zentra-api -n 50"
}

# ── Main ──────────────────────────────────────────────────────────────────────
echo ""
echo "======================================================"
echo "  Zentra Deploy  |  mode: ${MODE:-(full)}"
echo "  Target: $HOST:$REMOTE_DIR"
echo "======================================================"

case "$MODE" in
    --backend-only)
        sync_source
        build_backend
        restart_services
        health_check
        ;;
    --frontend-only)
        sync_source
        build_frontend
        restart_services
        health_check
        ;;
    --restart-only)
        restart_services
        health_check
        ;;
    *)
        sync_source
        build_backend
        build_frontend
        restart_services
        health_check
        ;;
esac

echo ""
echo "======================================================"
echo "  Deployment complete!"
echo "  API:       http://10.88.11.162:5006/api/health"
echo "  Frontend:  http://10.88.11.162:5010"
echo "  Agent DL:  http://10.88.11.162:5010/dl/zentra-agent-linux-x86_64"
echo "======================================================"
