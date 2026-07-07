#!/bin/bash
# Zentra full build + deploy script
# Builds backend, frontend, server binary, and agent binaries from source on the remote server.
#
# Usage:
#   ./build-and-deploy.sh                  # full build: backend + frontend + restart
#   ./build-and-deploy.sh --backend-only   # build API + agent + collector, restart API
#   ./build-and-deploy.sh --frontend-only  # build frontend only, restart frontend
#   ./build-and-deploy.sh --agent-only     # build agent binary only (fastest)
#   ./build-and-deploy.sh --restart-only   # restart services without rebuilding

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────────
HOST="root@10.88.11.162"
REMOTE_DIR="/store/Zentra"
MODE="${1:-}"

SSH_OPTS="-p 22 -o StrictHostKeyChecking=no -o PreferredAuthentications=password -o PubkeyAuthentication=no -o ConnectTimeout=15"
SSH="sshpass -p skyline ssh $SSH_OPTS $HOST"

# ── Logging ────────────────────────────────────────────────────────────────────
info() { echo ""; echo ">>> $*"; }
ok()   { echo "    ✓  $*"; }
fail() { echo "    ✗  $*" >&2; }
die()  { fail "$*"; exit 1; }
step() { echo "    →  $*"; }

run_remote() { $SSH "$@"; }

# ── Banner ─────────────────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║         Zentra Build & Deploy Script                 ║"
echo "║  Mode   : ${MODE:-(full)}                            "
echo "║  Target : $HOST:$REMOTE_DIR                         "
echo "╚══════════════════════════════════════════════════════╝"

# ── Pre-flight: verify SSH ─────────────────────────────────────────────────────
info "Verifying SSH connection..."
run_remote "echo 'SSH OK'" || die "Cannot reach $HOST — check network/credentials"
ok "SSH connection verified"

# ── Background build helper ────────────────────────────────────────────────────
# Starts cargo build in background, polls until done. Avoids SSH timeout on long builds.
# Usage: bg_build <crate-name> <log-file>
bg_build() {
    local crate="$1"
    local logfile="$2"

    run_remote "
        rm -f $logfile
        nohup bash -c '
            cd $REMOTE_DIR
            cargo build --release -p $crate >> $logfile 2>&1 \
                && echo __BUILD_OK__ >> $logfile \
                || echo __BUILD_FAIL__ >> $logfile
        ' >/dev/null 2>&1 &
    "

    step "Building $crate (background)..."
    local elapsed=0
    while true; do
        sleep 10
        elapsed=$((elapsed + 10))
        local tail_out
        tail_out=$(sshpass -p skyline ssh $SSH_OPTS $HOST "tail -3 $logfile 2>/dev/null" 2>/dev/null || true)
        if echo "$tail_out" | grep -q '__BUILD_OK__'; then
            ok "$crate built successfully (${elapsed}s)"
            return 0
        elif echo "$tail_out" | grep -q '__BUILD_FAIL__'; then
            fail "$crate build FAILED — last errors:"
            run_remote "grep '^error' $logfile 2>/dev/null | head -20" || true
            return 1
        fi
        [ $((elapsed % 30)) -eq 0 ] && step "  ... still building $crate (${elapsed}s elapsed)"
    done
}

# ── Step 1: Build backend crates ───────────────────────────────────────────────
# Builds three crates:
#   zentra-api       → the main API server binary  (zentra-server)
#   zentra-agent     → the monitoring agent binary  (zentra-agent-linux-x86_64)
#   zentra-collector → the metric collector binary  (zentra-collector-linux-x86_64)
build_backend() {
    info "Building backend crates on server (this may take 5–15 minutes)..."

    bg_build zentra-api       /tmp/build-api.log       || die "zentra-api build failed"
    bg_build zentra-agent     /tmp/build-agent.log     || die "zentra-agent build failed"
    bg_build zentra-collector /tmp/build-collector.log || die "zentra-collector build failed"

    info "Copying built binaries into place..."
    run_remote "
        set -e
        cd $REMOTE_DIR

        # Create required directories
        mkdir -p bin/dl

        # ── Server binary ──────────────────────────────────────────────────
        # Back up current binary before replacing
        [ -f bin/zentra-server ] && cp -f bin/zentra-server bin/zentra-server.bak || true

        # Stop API so we can replace the binary
        systemctl stop zentra-api 2>/dev/null || true
        sleep 1

        # Copy new server binary
        cp -f target/release/zentra-server bin/zentra-server \
            || { cp -f bin/zentra-server.bak bin/zentra-server 2>/dev/null || true
                 echo 'ROLLBACK: restored previous zentra-server binary'
                 exit 1; }

        # ── Agent binary ───────────────────────────────────────────────────
        # Placed in bin/dl/ — this is the folder served for agent downloads
        # Agents download from: http://10.88.11.162:5010/dl/zentra-agent-linux-x86_64
        cp -f target/release/zentra-agent bin/dl/zentra-agent-linux-x86_64

        # ── Collector binary ───────────────────────────────────────────────
        cp -f target/release/zentra-collector bin/dl/zentra-collector-linux-x86_64

        echo 'All binaries copied OK'
    " || die "Failed to copy binaries into place"

    ok "Server binary    → $REMOTE_DIR/bin/zentra-server"
    ok "Agent binary     → $REMOTE_DIR/bin/dl/zentra-agent-linux-x86_64"
    ok "Collector binary → $REMOTE_DIR/bin/dl/zentra-collector-linux-x86_64"
}

# ── Step 2: Build agent binary only ───────────────────────────────────────────
build_agent_only() {
    info "Building zentra-agent only (fast build)..."

    bg_build zentra-agent /tmp/build-agent.log || die "zentra-agent build failed"

    run_remote "
        set -e
        cd $REMOTE_DIR
        mkdir -p bin/dl
        cp -f target/release/zentra-agent bin/dl/zentra-agent-linux-x86_64
        echo 'Agent binary updated'
    " || die "Failed to copy agent binary"

    ok "Agent binary → $REMOTE_DIR/bin/dl/zentra-agent-linux-x86_64"
    ok "Download URL → http://10.88.11.162:5010/dl/zentra-agent-linux-x86_64"
}

# ── Step 3: Build frontend ─────────────────────────────────────────────────────
# Runs npm ci + npm run build on the server in the frontend/ directory.
# Output goes into frontend/dist/ which is served by zentra-frontend.service.
build_frontend() {
    info "Building frontend on server..."
    run_remote "
        set -e
        cd $REMOTE_DIR/frontend
        echo '  Installing npm dependencies...'
        npm ci --silent 2>/dev/null || npm install --silent
        echo '  Running build...'
        npm run build 2>&1 | tail -10
        echo 'Frontend build OK'
    " || die "Frontend build failed"
    ok "Frontend built → $REMOTE_DIR/frontend/dist"
}

# ── Step 4: Restart services ───────────────────────────────────────────────────
restart_services() {
    info "Restarting services..."
    run_remote "
        set -e

        # Restart main application services
        systemctl restart zentra-api
        systemctl restart zentra-frontend

        # nginx: reload config if running, start fresh if stopped
        if systemctl is-active --quiet nginx; then
            systemctl reload nginx
        else
            fuser -k 5010/tcp 2>/dev/null || true
            sleep 1
            systemctl start nginx
        fi

        # Verify all services came up
        echo ''
        echo 'Service status:'
        for svc in zentra-api zentra-frontend nginx; do
            if systemctl is-active --quiet \$svc; then
                echo \"  ✓ \$svc is running\"
            else
                echo \"  ✗ \$svc FAILED\" >&2
                systemctl status \$svc --no-pager -l 2>&1 | tail -15 >&2
                exit 1
            fi
        done
    " || die "Service restart failed — run: journalctl -u zentra-api -n 50"
    ok "All services running"
}

# ── Step 5: Health check ───────────────────────────────────────────────────────
health_check() {
    info "Waiting for API to be ready..."
    local attempt=1
    while [ $attempt -le 10 ]; do
        local resp
        resp=$(curl -sf --max-time 5 "http://10.88.11.162:5006/api/health" 2>/dev/null || true)
        if [ -n "$resp" ]; then
            ok "API health check passed (attempt $attempt)"
            return 0
        fi
        step "Attempt $attempt/10 — waiting 5s..."
        sleep 5
        attempt=$((attempt + 1))
    done
    die "API health check failed — check: journalctl -u zentra-api -n 50 on server"
}

# ── Main dispatcher ────────────────────────────────────────────────────────────
case "$MODE" in

    --backend-only)
        # Build all three backend crates, replace binaries, restart API only
        build_backend
        restart_services
        health_check
        ;;

    --agent-only)
        # Just rebuild the agent binary and put it in the download folder
        # Does NOT restart any service — existing agents re-download on their own
        build_agent_only
        ;;

    --frontend-only)
        # Rebuild the frontend bundle only, restart frontend service
        build_frontend
        restart_services
        health_check
        ;;

    --restart-only)
        # Just restart all services — no build step
        restart_services
        health_check
        ;;

    *)
        # Full build: backend + frontend + restart + health check
        build_backend
        build_frontend
        restart_services
        health_check
        ;;
esac

# ── Final summary ──────────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  Build & Deploy Complete!                            ║"
echo "╠══════════════════════════════════════════════════════╣"
echo "║  API      →  http://10.88.11.162:5006/api/health    ║"
echo "║  Frontend →  http://10.88.11.162:5010               ║"
echo "║  Agent DL →  http://10.88.11.162:5010/dl/           ║"
echo "║               zentra-agent-linux-x86_64             ║"
echo "║               zentra-collector-linux-x86_64         ║"
echo "╚══════════════════════════════════════════════════════╝"
