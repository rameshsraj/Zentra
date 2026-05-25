#!/usr/bin/env bash
# =============================================================================
# Zentra - Automated Installation Script (Linux)
# Downloads/loads the Docker image and starts all services automatically.
#
# Usage:
#   chmod +x install.sh
#   ./install.sh
#
# Prerequisites: Docker must be installed and running.
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration - edit these if needed
# ---------------------------------------------------------------------------
CONTAINER_NAME="zentra"
IMAGE_NAME="zentra:latest"
WEB_PORT="5010"
VOLUME_NAME="zentra-data"
LOG_DIR="/opt/zentra/logs"
ENV_FILE="/opt/zentra/.env"

# GHCR settings (only used if pulling from registry)
GHCR_IMAGE="ghcr.io/rameshsraj/zentra:latest"

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC}  $1"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo ""
echo "======================================================"
echo "  Zentra - Automated Installer (Linux)"
echo "======================================================"
echo ""

# ---------------------------------------------------------------------------
# Step 1: Check prerequisites
# ---------------------------------------------------------------------------
info "Checking prerequisites..."

if ! command -v docker >/dev/null 2>&1; then
    error "Docker is not installed."
    echo ""
    echo "  Install Docker first:"
    echo "    RHEL/Rocky/CentOS:  sudo dnf install -y docker && sudo systemctl enable --now docker"
    echo "    Ubuntu/Debian:      sudo apt-get install -y docker.io && sudo systemctl enable --now docker"
    echo ""
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    error "Docker daemon is not running. Start it with: sudo systemctl start docker"
    exit 1
fi

ok "Docker is installed and running ($(docker --version))"

# ---------------------------------------------------------------------------
# Step 2: Clean up existing installation
# ---------------------------------------------------------------------------
info "Cleaning up any previous Zentra installation..."

# Stop running container
if docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    info "Stopping running container '${CONTAINER_NAME}'..."
    docker stop "${CONTAINER_NAME}" >/dev/null 2>&1
    sleep 3
    ok "Container stopped"
fi

# Remove existing container
if docker ps -a --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    info "Removing old container '${CONTAINER_NAME}'..."
    docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1
    sleep 2
    ok "Old container removed"
fi

# Remove old volume (fresh database on each install)
if docker volume ls --format '{{.Name}}' | grep -q "^${VOLUME_NAME}$"; then
    info "Removing old data volume '${VOLUME_NAME}'..."
    docker volume rm "${VOLUME_NAME}" >/dev/null 2>&1
    sleep 2
    ok "Old volume removed (fresh database will be created)"
fi

# Remove old zentra images
OLD_IMAGES=$(docker images "${IMAGE_NAME}" -q 2>/dev/null)
if [ -n "${OLD_IMAGES}" ]; then
    info "Removing old zentra image(s)..."
    docker rmi -f ${OLD_IMAGES} >/dev/null 2>&1 || true
    sleep 2
    ok "Old zentra images removed"
fi

# Remove stale/dangling images
STALE_IMAGES=$(docker images --filter "dangling=true" -q 2>/dev/null)
if [ -n "${STALE_IMAGES}" ]; then
    info "Removing dangling images..."
    docker rmi ${STALE_IMAGES} >/dev/null 2>&1 || true
    sleep 2
    ok "Dangling images cleaned"
fi

# Remove old .env to regenerate with fresh secrets
if [ -f "${ENV_FILE}" ]; then
    info "Removing old environment file..."
    sudo rm -f "${ENV_FILE}"
    ok "Old .env removed (will regenerate with fresh secrets)"
fi

ok "Cleanup complete"

# ---------------------------------------------------------------------------
# Step 3: Load or pull the Docker image
# ---------------------------------------------------------------------------
info "Loading Zentra Docker image..."

# Check if tarball exists in current directory
TARBALL=""
for f in zentra-latest.tar.gz zentra*.tar.gz; do
    if [ -f "$f" ]; then
        TARBALL="$f"
        break
    fi
done

if [ -n "$TARBALL" ]; then
    info "Found tarball: ${TARBALL} - loading..."
    docker load -i "${TARBALL}"
    ok "Image loaded from ${TARBALL}"
elif docker image inspect "${IMAGE_NAME}" >/dev/null 2>&1; then
    ok "Image ${IMAGE_NAME} already exists locally"
else
    info "No tarball found. Pulling from GHCR..."
    warn "If the image is private, run first: docker login ghcr.io"
    docker pull "${GHCR_IMAGE}"
    docker tag "${GHCR_IMAGE}" "${IMAGE_NAME}"
    ok "Image pulled from GHCR"
fi

# Verify image exists
if ! docker image inspect "${IMAGE_NAME}" >/dev/null 2>&1; then
    error "Image ${IMAGE_NAME} not found. Cannot continue."
    exit 1
fi

IMAGE_SIZE=$(docker image inspect "${IMAGE_NAME}" --format='{{.Size}}' | awk '{printf "%.0f MB", $1/1024/1024}')
ok "Image ready: ${IMAGE_NAME} (${IMAGE_SIZE})"

# ---------------------------------------------------------------------------
# Step 4: Create .env file
# ---------------------------------------------------------------------------
if [ ! -f "${ENV_FILE}" ]; then
    info "Creating default environment file at ${ENV_FILE}..."
    sudo mkdir -p "$(dirname "${ENV_FILE}")"

    # Generate a random JWT secret
    JWT_SECRET=$(openssl rand -hex 32 2>/dev/null || head -c 64 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 64)
    PG_PASS=$(openssl rand -hex 16 2>/dev/null || head -c 32 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 24)

    sudo tee "${ENV_FILE}" > /dev/null <<EOF
# === Zentra Configuration ===
# Generated by install.sh on $(date +%Y-%m-%d)

# PostgreSQL
POSTGRES_USER=postgres
POSTGRES_PASSWORD=${PG_PASS}
POSTGRES_DB=zentra
POSTGRES_PORT=5434

# Zentra API
ZENTRA_JWT_SECRET=${JWT_SECRET}
ZENTRA_MASTER_KEY=${JWT_SECRET}
ZENTRA_ALLOW_INSECURE_JWT=1
ZENTRA_API_PORT=5006
ZENTRA_ALLOW_ORIGINS=*
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001

# Logging
RUST_LOG=info
EOF

    sudo chmod 600 "${ENV_FILE}"
    ok "Environment file created: ${ENV_FILE}"
    info "  PostgreSQL password: ${PG_PASS}"
    info "  JWT secret: auto-generated (64 chars)"
else
    ok "Environment file already exists: ${ENV_FILE}"
fi

# ---------------------------------------------------------------------------
# Step 5: Create log directory
# ---------------------------------------------------------------------------
sudo mkdir -p "${LOG_DIR}"
ok "Log directory ready: ${LOG_DIR}"

# ---------------------------------------------------------------------------
# Step 6: Start the container
# ---------------------------------------------------------------------------
info "Starting Zentra container..."

docker run -d \
    --name "${CONTAINER_NAME}" \
    --restart unless-stopped \
    --env-file "${ENV_FILE}" \
    -p "${WEB_PORT}:5010" \
    -v "${VOLUME_NAME}:/var/lib/postgresql/data" \
    -v "${LOG_DIR}:/opt/zentra/logs" \
    "${IMAGE_NAME}" >/dev/null

ok "Container '${CONTAINER_NAME}' started"

# ---------------------------------------------------------------------------
# Step 7: Wait for health check
# ---------------------------------------------------------------------------
info "Waiting for all services to start..."

HEALTHY=false
for i in $(seq 1 30); do
    STATUS=$(docker inspect --format='{{.State.Health.Status}}' "${CONTAINER_NAME}" 2>/dev/null || echo "starting")
    if [ "${STATUS}" = "healthy" ]; then
        HEALTHY=true
        break
    fi
    # Also try a direct curl
    if curl -sf "http://localhost:${WEB_PORT}/api/health" 2>/dev/null | grep -q '"status"'; then
        HEALTHY=true
        break
    fi
    printf "."
    sleep 2
done
echo ""

if [ "${HEALTHY}" = true ]; then
    ok "All services are running and healthy!"
else
    warn "Health check not yet passing - services may still be starting."
    warn "Check logs with: docker logs ${CONTAINER_NAME}"
fi

# ---------------------------------------------------------------------------
# Step 8: Print summary
# ---------------------------------------------------------------------------
SERVER_IP=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")

echo ""
echo "======================================================"
echo "  Zentra - Installation Complete!"
echo "======================================================"
echo ""
echo "  Web UI:     http://${SERVER_IP}:${WEB_PORT}"
echo "  Login:      admin@Zentra.local / changeme"
echo ""
echo "  Container:  docker ps | grep ${CONTAINER_NAME}"
echo "  Logs:       docker logs -f ${CONTAINER_NAME}"
echo "  Stop:       docker stop ${CONTAINER_NAME}"
echo "  Start:      docker start ${CONTAINER_NAME}"
echo "  Shell:      docker exec -it ${CONTAINER_NAME} bash"
echo ""
echo "  IMPORTANT: Change the default admin password after first login!"
echo "======================================================"
echo ""
