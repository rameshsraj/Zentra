#!/bin/bash
# Zentra full build + deploy script
# Runs entirely on the local server — assumes code is already present.
# Builds backend, frontend, server binary, and agent binaries from source.
#
# Usage:
#   ./build-and-deploy.sh                  # full build: backend + frontend + restart
#   ./build-and-deploy.sh --backend-only   # build API + agent + collector, restart API
#   ./build-and-deploy.sh --frontend-only  # build frontend only, restart frontend
#   ./build-and-deploy.sh --agent-only     # build agent binary only (fastest)
#   ./build-and-deploy.sh --restart-only   # restart services without rebuilding

set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────────
# Detect the server's own primary IP automatically
SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}')
if [ -z "$SERVER_IP" ]; then
    SERVER_IP=$(ip route get 1.1.1.1 2>/dev/null | awk '/src/{print $7}' | head -1)
fi
if [ -z "$SERVER_IP" ]; then
    SERVER_IP="127.0.0.1"
fi

REMOTE_DIR="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-}"

# Script runs locally — no SSH needed
# All commands execute directly on this machine

# ── Logging ────────────────────────────────────────────────────────────────────
info() { echo ""; echo ">>> $*"; }
ok()   { echo "    ✓  $*"; }
fail() { echo "    ✗  $*" >&2; }
die()  { fail "$*"; exit 1; }
step() { echo "    →  $*"; }

# run_remote: executes commands locally (script runs on the server itself)
run_remote() { bash -c "$*"; }

# ── Banner ─────────────────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║         Zentra Build & Deploy Script                 ║"
echo "║  Mode   : ${MODE:-(full)}                            "
echo "║  Dir    : $REMOTE_DIR                               "
echo "║  IP     : $SERVER_IP                                "
echo "╚══════════════════════════════════════════════════════╝"

# ── Pre-flight: verify working directory ─────────────────────────────────────
info "Verifying build directory..."
[ -f "$REMOTE_DIR/Cargo.toml" ] || die "Cargo.toml not found in $REMOTE_DIR — is the code present?"
[ -d "$REMOTE_DIR/frontend" ]   || die "frontend/ directory not found in $REMOTE_DIR"
ok "Build directory verified: $REMOTE_DIR"

# ── Background build helper ────────────────────────────────────────────────────
# Starts cargo build in background, polls until done. Avoids SSH timeout on long builds.
# Usage: bg_build <crate-name> <log-file>
bg_build() {
    local crate="$1"
    local logfile="$2"

    rm -f "$logfile"
    (
        cd "$REMOTE_DIR"
        cargo build --release -p "$crate" >> "$logfile" 2>&1 \
            && echo __BUILD_OK__ >> "$logfile" \
            || echo __BUILD_FAIL__ >> "$logfile"
    ) &

    step "Building $crate (background)..."
    local elapsed=0
    while true; do
        sleep 10
        elapsed=$((elapsed + 10))
        local tail_out
        tail_out=$(tail -3 "$logfile" 2>/dev/null || true)
        if echo "$tail_out" | grep -q '__BUILD_OK__'; then
            ok "$crate built successfully (${elapsed}s)"
            return 0
        elif echo "$tail_out" | grep -q '__BUILD_FAIL__'; then
            fail "$crate build FAILED — last errors:"
            grep '^error' "$logfile" 2>/dev/null | head -20 || true
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
    (
        set -e
        cd "$REMOTE_DIR"

        # Create required directories
        mkdir -p bin/dl

        # ── Server binary ──────────────────────────────────────────────────
        [ -f bin/zentra-server ] && cp -f bin/zentra-server bin/zentra-server.bak || true
        systemctl stop zentra-api 2>/dev/null || true
        sleep 1
        cp -f target/release/zentra-server bin/zentra-server \
            || { cp -f bin/zentra-server.bak bin/zentra-server 2>/dev/null || true
                 echo 'ROLLBACK: restored previous zentra-server binary'
                 exit 1; }

        # ── Agent binary ───────────────────────────────────────────────────
        # Placed in bin/dl/ — served for agent downloads
        cp -f target/release/zentra-agent bin/dl/zentra-agent-linux-x86_64

        # ── Collector binary ───────────────────────────────────────────────
        cp -f target/release/zentra-collector bin/dl/zentra-collector-linux-x86_64

        echo 'All binaries copied OK'
    ) || die "Failed to copy binaries into place"

    ok "Server binary    → $REMOTE_DIR/bin/zentra-server"
    ok "Agent binary     → $REMOTE_DIR/bin/dl/zentra-agent-linux-x86_64"
    ok "Collector binary → $REMOTE_DIR/bin/dl/zentra-collector-linux-x86_64"
}

# ── Step 2: Build agent binary only ───────────────────────────────────────────
build_agent_only() {
    info "Building zentra-agent only (fast build)..."

    bg_build zentra-agent /tmp/build-agent.log || die "zentra-agent build failed"

    (
        set -e
        cd "$REMOTE_DIR"
        mkdir -p bin/dl
        cp -f target/release/zentra-agent bin/dl/zentra-agent-linux-x86_64
        echo 'Agent binary updated'
    ) || die "Failed to copy agent binary"

    ok "Agent binary → $REMOTE_DIR/bin/dl/zentra-agent-linux-x86_64"
    ok "Download URL → http://${SERVER_IP}:5010/dl/zentra-agent-linux-x86_64"
}

# ── Step 3: Build frontend ─────────────────────────────────────────────────────
# Runs npm ci + npm run build on the server in the frontend/ directory.
# Output goes into frontend/dist/ which is served by nginx on port 5010.
build_frontend() {
    info "Building frontend on server..."
    (
        set -e
        cd "$REMOTE_DIR/frontend"
        echo '  Installing npm dependencies...'
        npm ci --silent 2>/dev/null || npm install --silent
        echo '  Running build...'
        npm run build 2>&1 | tail -10
        echo 'Frontend build OK'
    ) || die "Frontend build failed"
    ok "Frontend built → $REMOTE_DIR/frontend/dist"
}

# ── Step 4: Restart services ───────────────────────────────────────────────────
restart_services() {
    info "Restarting services..."
    (
        set -e
        systemctl restart zentra-api

        if systemctl is-active --quiet nginx; then
            systemctl reload nginx
        else
            fuser -k 5010/tcp 2>/dev/null || true
            sleep 1
            systemctl start nginx
        fi

        echo ''
        echo 'Service status:'
        for svc in zentra-api nginx; do
            if systemctl is-active --quiet "$svc"; then
                echo "  ✓ $svc is running"
            else
                echo "  ✗ $svc FAILED" >&2
                systemctl status "$svc" --no-pager -l 2>&1 | tail -15 >&2
                exit 1
            fi
        done
    ) || die "Service restart failed — run: journalctl -u zentra-api -n 50"
    ok "All services running"
}

# ── Step 5: Health check ───────────────────────────────────────────────────────
health_check() {
    info "Waiting for API to be ready..."
    local attempt=1
    while [ $attempt -le 10 ]; do
        local resp
        resp=$(curl -sf --max-time 5 "http://${SERVER_IP}:5006/api/health" 2>/dev/null || true)
        if [ -n "$resp" ]; then
            ok "API health check passed (attempt $attempt)"
            return 0
        fi
        step "Attempt $attempt/10 — waiting 5s..."
        sleep 5
        attempt=$((attempt + 1))
    done
    die "API health check failed — check: journalctl -u zentra-api -n 50"
}

frontend_health_check() {
    info "Waiting for frontend to be ready..."
    local attempt=1
    while [ $attempt -le 10 ]; do
        if curl -sf --max-time 5 "http://127.0.0.1:5010/" >/dev/null 2>&1; then
            ok "Frontend health check passed (attempt $attempt)"
            return 0
        fi
        step "Attempt $attempt/10 — waiting 5s..."
        sleep 5
        attempt=$((attempt + 1))
    done
    die "Frontend health check failed — check: journalctl -u nginx -n 50"
}

# ── Main dispatcher ────────────────────────────────────────────────────────────
case "$MODE" in

    --backend-only)
        # Build all three backend crates, replace binaries, restart API only
        build_backend
        restart_services
        health_check
        frontend_health_check
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
        frontend_health_check
        ;;

    --restart-only)
        # Just restart all services — no build step
        restart_services
        health_check
        frontend_health_check
        ;;

    *)
        # Full build: backend + frontend + restart + health check
        build_backend
        build_frontend
        restart_services
        health_check
        frontend_health_check
        ;;
esac

# ── Final summary ──────────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║  Build & Deploy Complete!                            ║"
echo "╠══════════════════════════════════════════════════════╣"
echo "║  API      →  http://${SERVER_IP}:5006/api/health    ║"
echo "║  Frontend →  http://${SERVER_IP}:5010               ║"
echo "║  Agent DL →  http://${SERVER_IP}:5010/dl/           ║"
echo "║               zentra-agent-linux-x86_64             ║"
echo "║               zentra-collector-linux-x86_64         ║"
echo "╚══════════════════════════════════════════════════════╝"
