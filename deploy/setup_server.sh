#!/usr/bin/env bash
# =============================================================================
# Zentra — First-time Server Setup Script
# Run once on 10.88.11.162 as root to prepare /store/Zentra
# =============================================================================
set -euo pipefail

DEPLOY_PATH="/store/Zentra"
BACKEND_PORT="5006"
FRONTEND_PORT="5010"
DB_URL="${DATABASE_URL:?Error: DATABASE_URL not set}"

echo "Setting up Zentra server environment..."

# Create zentra service user (non-root, no login shell)
if ! id -u zentra >/dev/null 2>&1; then
    useradd --system --shell /usr/sbin/nologin --home-dir /store/Zentra --no-create-home zentra
    echo "Created service user: zentra"
fi

# Create directory structure
mkdir -p "${DEPLOY_PATH}/bin"
mkdir -p "${DEPLOY_PATH}/frontend/dist"
mkdir -p "${DEPLOY_PATH}/migrations"
mkdir -p "${DEPLOY_PATH}/logs"

# Set ownership of deploy directory
chown -R zentra:zentra "${DEPLOY_PATH}"

# Create metrics auth file (run once manually):
# htpasswd -c /etc/nginx/.htpasswd_metrics prometheus

# Write systemd service for API
cat > /etc/systemd/system/zentra-api.service << EOF
[Unit]
Description=Zentra API Server
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=simple
User=zentra
Group=zentra
WorkingDirectory=${DEPLOY_PATH}
EnvironmentFile=${DEPLOY_PATH}/.env
ExecStart=${DEPLOY_PATH}/bin/zentra-server
Restart=always
RestartSec=5
StandardOutput=append:${DEPLOY_PATH}/logs/api.log
StandardError=append:${DEPLOY_PATH}/logs/api.log
# API binds to port 5006 (>1024), no special capabilities needed.
# If you need port < 1024, add: AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
EOF

# Write systemd service for static frontend (using Python http.server)
cat > /etc/systemd/system/zentra-frontend.service << EOF
[Unit]
Description=Zentra Frontend Static Server
After=network.target

[Service]
Type=simple
User=zentra
Group=zentra
WorkingDirectory=${DEPLOY_PATH}/frontend/dist
ExecStart=/usr/bin/python3 -m http.server ${FRONTEND_PORT}
Restart=always
RestartSec=5
StandardOutput=append:${DEPLOY_PATH}/logs/frontend.log
StandardError=append:${DEPLOY_PATH}/logs/frontend.log

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable zentra-api
systemctl enable zentra-frontend

echo "Server setup complete."
echo "  Run 'systemctl start zentra-api zentra-frontend' after deployment."
echo "  API will listen on port ${BACKEND_PORT}"
echo "  Frontend will listen on port ${FRONTEND_PORT}"
