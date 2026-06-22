#!/usr/bin/env bash
# =============================================================================
# Zentra — Docker Image Build Script
# Run this on the Linux build server.
#
# This script builds the Docker image, verifies it, exports a tarball,
# then CLEANS UP all Docker artifacts. Nothing is left running on the server.
# The server is a build machine only — not a Docker runtime host.
#
# Usage:
#   ./docker/build-image.sh              # Full build (frontend + backend + image + tarball)
#   ./docker/build-image.sh --skip-fe    # Skip frontend build (use existing dist/)
#   ./docker/build-image.sh --skip-be    # Skip backend build (use existing binary)
#   ./docker/build-image.sh --image-only # Skip both, just rebuild Docker image + tarball
#
# After build:
#   scp <server>:/path/to/docker/zentra-latest.tar.gz ./docker/
#
# Prerequisites:
#   - Rust 1.95+ (rustc, cargo)
#   - Node.js 18+ (node, npm)
#   - Docker 20.10+
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
IMAGE_NAME="zentra"
AGENT_IMAGE_NAME="zentra-agent"
IMAGE_TAG="latest"
TARBALL="${SCRIPT_DIR}/zentra-latest.tar.gz"
AGENT_TARBALL="${SCRIPT_DIR}/zentra-agent-latest.tar.gz"

SKIP_FE=false
SKIP_BE=false

for arg in "$@"; do
    case "$arg" in
        --skip-fe)    SKIP_FE=true ;;
        --skip-be)    SKIP_BE=true ;;
        --image-only) SKIP_FE=true; SKIP_BE=true ;;
        -h|--help)
            echo "Usage: $0 [--skip-fe] [--skip-be] [--image-only]"
            exit 0 ;;
        *) echo "Unknown arg: $arg"; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Cleanup function — always runs on exit (success or failure)
# Ensures NO Docker containers, images, or volumes are left on the server.
# ---------------------------------------------------------------------------
cleanup_docker() {
    echo ""
    echo "[CLEANUP] Stopping all Docker containers..."
    docker ps -q 2>/dev/null | xargs -r docker stop 2>/dev/null || true
    echo "[CLEANUP] Removing all Docker containers..."
    docker ps -aq 2>/dev/null | xargs -r docker rm -f 2>/dev/null || true
    echo "[CLEANUP] Removing built images..."
    docker rmi "${IMAGE_NAME}:${IMAGE_TAG}" 2>/dev/null || true
    docker rmi "${AGENT_IMAGE_NAME}:${IMAGE_TAG}" 2>/dev/null || true
    echo "[CLEANUP] Pruning dangling images, volumes, and networks..."
    docker image prune -f 2>/dev/null || true
    docker volume prune -f 2>/dev/null || true
    docker network prune -f 2>/dev/null || true
    echo "[CLEANUP] Docker is clean — nothing running on this server."
}
trap cleanup_docker EXIT

echo "=============================================="
echo "  Zentra — Docker Image Build"
echo "  Server:  $(hostname)"
echo "  Date:    $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "  Root:    ${PROJECT_ROOT}"
echo "=============================================="

# ── D01: Build Frontend ──
if [ "$SKIP_FE" = false ]; then
    echo ""
    echo "[D01] Building frontend..."
    cd "${PROJECT_ROOT}/frontend"
    npm ci --prefer-offline 2>&1 | tail -3
    npx vite build 2>&1 | tail -5
    echo "[D01] Frontend build complete."
    ls -la "${PROJECT_ROOT}/frontend/dist/index.html"
else
    echo "[D01] Skipping frontend build (--skip-fe)"
fi

# Verify frontend dist exists
if [ ! -f "${PROJECT_ROOT}/frontend/dist/index.html" ]; then
    echo "[ERROR] frontend/dist/index.html not found. Run without --skip-fe."
    exit 1
fi

# ── D02: Build Rust Backend ──
if [ "$SKIP_BE" = false ]; then
    echo ""
    echo "[D02] Building Rust backend (release)..."
    cd "${PROJECT_ROOT}"
    cargo build --release --bin zentra-server --bin zentra-agent 2>&1 | tail -5
    echo "[D02] Backend build complete."
    ls -lh "${PROJECT_ROOT}/target/release/zentra-server"
    ls -lh "${PROJECT_ROOT}/target/release/zentra-agent"
else
    echo "[D02] Skipping backend build (--skip-be)"
fi

# Verify binary exists
if [ ! -f "${PROJECT_ROOT}/target/release/zentra-server" ]; then
    echo "[ERROR] target/release/zentra-server not found. Run without --skip-be."
    exit 1
fi

# Verify it's a Linux ELF binary
FILE_TYPE=$(file "${PROJECT_ROOT}/target/release/zentra-server")
if ! echo "$FILE_TYPE" | grep -q "ELF"; then
    echo "[ERROR] Binary is not a Linux ELF executable: ${FILE_TYPE}"
    exit 1
fi

# ── D03: Build Docker Image ──
echo ""
echo "[D03] Building Docker image ${IMAGE_NAME}:${IMAGE_TAG}..."
cd "${PROJECT_ROOT}"
docker build -f docker/Dockerfile -t "${IMAGE_NAME}:${IMAGE_TAG}" .
echo "[D03] Docker image built."
docker images "${IMAGE_NAME}:${IMAGE_TAG}" --format "  Image: {{.Repository}}:{{.Tag}}  Size: {{.Size}}  Created: {{.CreatedAt}}"

# ── D04: Verify No Source Code ──
echo ""
echo "[D04] Verifying no source code in image..."
LEAKED=$(docker run --rm --entrypoint sh "${IMAGE_NAME}:${IMAGE_TAG}" -c \
    'find / -name "*.rs" -o -name "Cargo.toml" -o -name "Cargo.lock" -o -name ".git" -o -name "node_modules" -o -name "*.tsx" -o -name "*.ts" 2>/dev/null | grep -v /proc/ | head -20' || true)
if [ -n "$LEAKED" ]; then
    echo "[ERROR] Source code found in image:"
    echo "$LEAKED"
    exit 1
fi
echo "[D04] PASSED — no source code in image."

# Verify build artifacts present
echo "[D04] Checking build artifacts..."
docker run --rm --entrypoint sh "${IMAGE_NAME}:${IMAGE_TAG}" -c '
    echo "  Binary:    $(ls -lh /usr/local/bin/zentra-server | awk "{print \$5}")"
    echo "  Frontend:  $(ls /opt/zentra/frontend/dist/index.html 2>/dev/null && echo OK || echo MISSING)"
    echo "  Migrations: $(ls /opt/zentra/migrations/*.sql 2>/dev/null | wc -l) SQL files"
    echo "  Integrity: $(ls /opt/zentra/.integrity 2>/dev/null && echo OK || echo MISSING)"
    echo "  Build-info: $(cat /opt/zentra/.build-info 2>/dev/null | head -1)"
'

# ── D05: Build Agent Docker Image ──
echo ""
echo "[D05] Building agent Docker image ${AGENT_IMAGE_NAME}:${IMAGE_TAG}..."

# Copy agent binary to simulation/bin for the Dockerfile context
mkdir -p "${PROJECT_ROOT}/docker/simulation/bin"
if [ -f "${PROJECT_ROOT}/target/release/zentra-agent" ]; then
    cp "${PROJECT_ROOT}/target/release/zentra-agent" "${PROJECT_ROOT}/docker/simulation/bin/zentra-agent"
    echo "[D05] Agent binary copied to docker/simulation/bin/"
fi

if [ ! -f "${PROJECT_ROOT}/docker/simulation/bin/zentra-agent" ]; then
    echo "[WARN] Agent binary not found — skipping agent image build."
else
    cd "${PROJECT_ROOT}"
    docker build -f docker/agent/Dockerfile -t "${AGENT_IMAGE_NAME}:${IMAGE_TAG}" .
    echo "[D05] Agent image built."
    docker images "${AGENT_IMAGE_NAME}:${IMAGE_TAG}" --format "  Image: {{.Repository}}:{{.Tag}}  Size: {{.Size}}  Created: {{.CreatedAt}}"
fi

# ── D08: Export as Tarballs ──
echo ""
echo "[D08] Exporting images as tarballs..."
docker save "${IMAGE_NAME}:${IMAGE_TAG}" | gzip > "${TARBALL}"
TARBALL_SIZE=$(ls -lh "${TARBALL}" | awk '{print $5}')
echo "[D08] Zentra app tarball: ${TARBALL} (${TARBALL_SIZE})"

AGENT_TARBALL_SIZE="N/A"
if docker image inspect "${AGENT_IMAGE_NAME}:${IMAGE_TAG}" >/dev/null 2>&1; then
    docker save "${AGENT_IMAGE_NAME}:${IMAGE_TAG}" | gzip > "${AGENT_TARBALL}"
    AGENT_TARBALL_SIZE=$(ls -lh "${AGENT_TARBALL}" | awk '{print $5}')
    echo "[D08] Agent tarball:     ${AGENT_TARBALL} (${AGENT_TARBALL_SIZE})"
fi

# ── Cleanup happens automatically via EXIT trap ──
# After tarballs are saved, the trap removes all Docker containers, images, volumes, networks.

echo ""
echo "=============================================="
echo "  BUILD COMPLETE"
echo "  App tarball:   ${TARBALL} (${TARBALL_SIZE})"
echo "  Agent tarball: ${AGENT_TARBALL} (${AGENT_TARBALL_SIZE})"
echo "  Date:          $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""
echo "  Next: copy tarballs to your local machine:"
echo "    scp $(hostname):${TARBALL} ./docker/zentra-latest.tar.gz"
echo "    scp $(hostname):${AGENT_TARBALL} ./docker/zentra-agent-latest.tar.gz"
echo "=============================================="
