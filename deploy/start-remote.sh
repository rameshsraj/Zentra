#!/bin/bash
# =============================================================
# Zentra Remote Startup Script
# Run this ON the remote server: bash /store/Zentra/start.sh
# =============================================================
set -e

# Source port configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true

DEPLOY_PATH="/store/Zentra"
BACKEND_PORT="${ZENTRA_API_PORT}"
FRONTEND_PORT="${ZENTRA_FRONTEND_PORT}"

cd "$DEPLOY_PATH"

# Source .env
set -a; source .env 2>/dev/null; set +a

echo "============================================"
echo " Zentra Service Manager"
echo "============================================"

# Kill any existing services
echo "[1/4] Stopping old services..."
pkill -f zentra-server 2>/dev/null || true
pkill -f zentra-agent 2>/dev/null || true
fuser -k ${BACKEND_PORT}/tcp 2>/dev/null || true
fuser -k ${FRONTEND_PORT}/tcp 2>/dev/null || true
sleep 1
echo "  Done."

# Start API server
echo "[2/4] Starting zentra-server on port $BACKEND_PORT..."
nohup ./target/release/zentra-server > /var/log/zentra-server.log 2>&1 &
echo "  PID: $!"

# Start agent
echo "[3/4] Starting zentra-agent..."
nohup ./target/release/zentra-agent > /var/log/zentra-agent.log 2>&1 &
echo "  PID: $!"

# Start frontend
echo "[4/4] Starting frontend on port $FRONTEND_PORT..."
if which nginx >/dev/null 2>&1; then
    echo "  Using nginx for frontend"
else
    cd "$DEPLOY_PATH/frontend/dist"
    nohup python3 -m http.server $FRONTEND_PORT > /var/log/Zentra-frontend.log 2>&1 &
    echo "  PID: $! (python3 HTTP server)"
fi

sleep 2
echo ""
echo "============================================"
echo " Zentra services started!"
echo " API:      http://$(hostname -I | awk '{print $1}'):$BACKEND_PORT"
echo " Frontend: http://$(hostname -I | awk '{print $1}'):$FRONTEND_PORT"
echo " Logs:     /var/log/Zentra-*.log"
echo "============================================"
