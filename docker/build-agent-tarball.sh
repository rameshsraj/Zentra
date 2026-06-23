#!/usr/bin/env bash
# =============================================================================
# Zentra — Build Agent Tarball Only
# =============================================================================
# Builds the zentra-agent Docker image, exports it as a gzipped tarball,
# then REMOVES the image from the server (the server is a build machine only).
#
# Output: ./docker/local-lab/zentra-agent-latest.tar.gz
# This file is then scp'd to a Windows / Mac machine for the local-lab.
#
# Prerequisites:
#   - target/release/zentra-agent binary present (run `cargo build --release` first)
#   - Docker installed
#
# Usage:
#   bash docker/build-agent-tarball.sh
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
IMAGE="zentra-agent:latest"
OUT_DIR="${SCRIPT_DIR}/local-lab"
TARBALL="${OUT_DIR}/zentra-agent-latest.tar.gz"

# Cleanup: always remove the agent image on exit so the server stays clean
cleanup() {
    echo ""
    echo "[cleanup] Removing local docker image ${IMAGE}..."
    docker rmi -f "${IMAGE}" >/dev/null 2>&1 || true
    docker image prune -f >/dev/null 2>&1 || true
    echo "[cleanup] Done."
}
trap cleanup EXIT

echo "=============================================="
echo "  Zentra — Build Agent Tarball"
echo "  Output: ${TARBALL}"
echo "=============================================="

# ─── 1. Verify agent binary ───────────────────────────────────────────────
AGENT_BIN="${PROJECT_ROOT}/target/release/zentra-agent"
if [ ! -f "${AGENT_BIN}" ]; then
    echo "[ERROR] Agent binary not found at ${AGENT_BIN}"
    echo "        Run: cargo build --release -p zentra-agent"
    exit 1
fi
echo "[1/4] Agent binary present ($(du -h "${AGENT_BIN}" | awk '{print $1}'))"

# ─── 2. Stage agent binary for Dockerfile context ─────────────────────────
mkdir -p "${PROJECT_ROOT}/docker/simulation/bin"
cp "${AGENT_BIN}" "${PROJECT_ROOT}/docker/simulation/bin/zentra-agent"
echo "[2/4] Agent binary staged for build context"

# ─── 3. Build the image ───────────────────────────────────────────────────
echo "[3/4] Building docker image ${IMAGE}..."
cd "${PROJECT_ROOT}"
docker build --no-cache -f docker/agent/Dockerfile -t "${IMAGE}" . > /dev/null
echo "       Built: $(docker images "${IMAGE}" --format '{{.Size}}')"

# ─── 4. Save tarball ──────────────────────────────────────────────────────
mkdir -p "${OUT_DIR}"
rm -f "${TARBALL}"
docker save "${IMAGE}" | gzip > "${TARBALL}"
SIZE=$(du -h "${TARBALL}" | awk '{print $1}')
echo "[4/4] Tarball saved: ${TARBALL} (${SIZE})"

echo ""
echo "=============================================="
echo "  DONE"
echo "  Tarball:  ${TARBALL}"
echo "  Size:     ${SIZE}"
echo ""
echo "  Next — copy to your Mac/Windows machine:"
echo "    scp $(hostname):${TARBALL} ./docker/local-lab/"
echo "=============================================="
