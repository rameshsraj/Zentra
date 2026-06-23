#!/usr/bin/env bash
# =============================================================================
# Zentra — Publish Build Artifacts to GitHub Release Repos
# =============================================================================
# Run on the Linux BUILD SERVER after build-image.sh completes.
#
# Pushes pre-built binaries to two PRIVATE GitHub repos:
#   1. rameshsraj/zentra-release  — server binary + frontend + migrations + setup
#   2. rameshsraj/zentra-agent    — agent binary + setup script
#
# The repos contain NO source code — only deployment artifacts.
#
# Prerequisites:
#   - git installed on build server
#   - GitHub PAT with 'repo' scope
#   - Build artifacts in target/release/, frontend/dist/, migrations/
#
# Usage:
#   export GITHUB_TOKEN="ghp_xxxxxxxxxxxx"
#   bash deploy/publish-release.sh
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

GITHUB_USER="rameshsraj"
RELEASE_REPO="zentra-release"
AGENT_REPO="zentra-agent"

# ---------------------------------------------------------------------------
# GitHub token
# ---------------------------------------------------------------------------
if [ -z "${GITHUB_TOKEN:-}" ]; then
    echo -n "GitHub PAT (with repo scope): "
    read -rs GITHUB_TOKEN
    echo ""
fi
[ -z "${GITHUB_TOKEN}" ] && echo "[ERROR] Token required." && exit 1

RELEASE_URL="https://${GITHUB_USER}:${GITHUB_TOKEN}@github.com/${GITHUB_USER}/${RELEASE_REPO}.git"
AGENT_URL="https://${GITHUB_USER}:${GITHUB_TOKEN}@github.com/${GITHUB_USER}/${AGENT_REPO}.git"

WORK_DIR="/tmp/zentra-publish-$$"
mkdir -p "${WORK_DIR}"
trap 'rm -rf "${WORK_DIR}"' EXIT

VERSION_TAG="v$(date -u +%Y%m%d.%H%M)"
BUILD_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ)

echo "=============================================="
echo "  Zentra — Publish Release"
echo "  Version: ${VERSION_TAG}"
echo "  Date:    ${BUILD_DATE}"
echo "=============================================="

# ---------------------------------------------------------------------------
# Verify build artifacts
# ---------------------------------------------------------------------------
echo ""
echo "[1/4] Verifying build artifacts..."

BINARY="${PROJECT_ROOT}/target/release/zentra-server"
AGENT_BINARY="${PROJECT_ROOT}/target/release/zentra-agent"
FRONTEND="${PROJECT_ROOT}/frontend/dist"
MIGRATIONS="${PROJECT_ROOT}/migrations"
NGINX_CONF="${PROJECT_ROOT}/docker/nginx.conf"
SETUP_SCRIPT="${PROJECT_ROOT}/deploy/setup-debian12.sh"
AGENT_SETUP="${PROJECT_ROOT}/deploy/setup-agent.sh"

MISSING=false
for f in "${BINARY}" "${AGENT_BINARY}"; do
    [ ! -f "$f" ] && echo "  MISSING: $f" && MISSING=true
done
for d in "${FRONTEND}" "${MIGRATIONS}"; do
    [ ! -d "$d" ] && echo "  MISSING: $d" && MISSING=true
done
[ "${MISSING}" = true ] && echo "[ERROR] Run build-image.sh first." && exit 1

echo "  zentra-server: $(du -h "${BINARY}" | awk '{print $1}')"
echo "  zentra-agent:  $(du -h "${AGENT_BINARY}" | awk '{print $1}')"
echo "  frontend/dist: $(find "${FRONTEND}" -type f | wc -l) files"
echo "  migrations:    $(find "${MIGRATIONS}" -name '*.sql' | wc -l) SQL files"

# ---------------------------------------------------------------------------
# Publish zentra-release
# ---------------------------------------------------------------------------
echo ""
echo "[2/4] Publishing ${RELEASE_REPO}..."

RELEASE_DIR="${WORK_DIR}/${RELEASE_REPO}"

# Clone or init
if git ls-remote "${RELEASE_URL}" HEAD >/dev/null 2>&1; then
    git clone --depth 1 "${RELEASE_URL}" "${RELEASE_DIR}" 2>/dev/null
else
    mkdir -p "${RELEASE_DIR}"
    cd "${RELEASE_DIR}"
    git init
    git remote add origin "${RELEASE_URL}"
fi

cd "${RELEASE_DIR}"

# Clean old artifacts (keep .git)
find . -mindepth 1 -maxdepth 1 ! -name '.git' -exec rm -rf {} +

# Copy artifacts
mkdir -p bin frontend migrations deploy

cp "${BINARY}"       bin/zentra-server
chmod 755            bin/zentra-server
cp -r "${FRONTEND}"  frontend/dist
cp -r "${MIGRATIONS}"/. migrations/
cp "${NGINX_CONF}"   deploy/nginx.conf
cp "${SETUP_SCRIPT}" deploy/setup-debian12.sh
chmod 755            deploy/setup-debian12.sh

# Version info
cat > VERSION <<EOF
version=${VERSION_TAG}
build_date=${BUILD_DATE}
build_host=$(hostname)
binary_size=$(du -h bin/zentra-server | awk '{print $1}')
migration_count=$(find migrations -name '*.sql' | wc -l)
EOF

# README
cat > README.md <<'READMEEOF'
# Zentra Release

Pre-built deployment artifacts for the Zentra monitoring platform.

**This repo contains NO source code — only compiled binaries and static assets.**

## Contents

| Path | Description |
|---|---|
| `bin/zentra-server` | Zentra API server binary (Linux x86_64) |
| `frontend/dist/` | React SPA static files |
| `migrations/` | SQL migration files (reference — binary embeds them) |
| `deploy/setup-debian12.sh` | Automated setup script for Debian/Rocky Linux |
| `deploy/nginx.conf` | Nginx reverse proxy config |
| `VERSION` | Build metadata |

## Quick Deploy (GCP or any Linux server)

```bash
# Clone this repo
git clone https://github.com/rameshsraj/zentra-release.git /store/zentra

# Run setup
cd /store/zentra
sudo bash deploy/setup-debian12.sh
```

See the setup script for configuration options.
READMEEOF

# .gitattributes — mark binary as LFS-trackable (optional) or just large file
cat > .gitattributes <<'EOF'
bin/zentra-server binary
EOF

# Commit and push
git add -A
git config user.email "build@zentra.local"
git config user.name "Zentra Build Server"
git commit -m "Release ${VERSION_TAG} — $(date -u +%Y-%m-%d)" --allow-empty
git branch -M main
git push -u origin main --force

echo "  ✓ ${RELEASE_REPO} published (${VERSION_TAG})"

# ---------------------------------------------------------------------------
# Publish zentra-agent
# ---------------------------------------------------------------------------
echo ""
echo "[3/4] Publishing ${AGENT_REPO}..."

AGENT_DIR="${WORK_DIR}/${AGENT_REPO}"

if git ls-remote "${AGENT_URL}" HEAD >/dev/null 2>&1; then
    git clone --depth 1 "${AGENT_URL}" "${AGENT_DIR}" 2>/dev/null
else
    mkdir -p "${AGENT_DIR}"
    cd "${AGENT_DIR}"
    git init
    git remote add origin "${AGENT_URL}"
fi

cd "${AGENT_DIR}"

# Clean old artifacts
find . -mindepth 1 -maxdepth 1 ! -name '.git' -exec rm -rf {} +

# Copy artifacts
mkdir -p bin deploy

cp "${AGENT_BINARY}" bin/zentra-agent
chmod 755            bin/zentra-agent

# Copy agent setup script if it exists
if [ -f "${AGENT_SETUP}" ]; then
    cp "${AGENT_SETUP}" deploy/setup-agent.sh
    chmod 755            deploy/setup-agent.sh
fi

# Copy agent entrypoint (for Docker users)
if [ -f "${PROJECT_ROOT}/docker/agent/agent-entrypoint.sh" ]; then
    cp "${PROJECT_ROOT}/docker/agent/agent-entrypoint.sh" deploy/agent-entrypoint.sh
    chmod 755 deploy/agent-entrypoint.sh
fi

# Version info
cat > VERSION <<EOF
version=${VERSION_TAG}
build_date=${BUILD_DATE}
build_host=$(hostname)
binary_size=$(du -h bin/zentra-agent | awk '{print $1}')
EOF

# README
cat > README.md <<'READMEEOF'
# Zentra Agent

Pre-built Zentra monitoring agent binary for on-premises deployment.

**This repo contains NO source code — only the compiled agent binary.**

## Contents

| Path | Description |
|---|---|
| `bin/zentra-agent` | Zentra agent binary (Linux x86_64) |
| `deploy/setup-agent.sh` | Automated setup script for on-premises Linux hosts |
| `deploy/agent-entrypoint.sh` | Docker entrypoint (optional) |
| `VERSION` | Build metadata |

## Quick Deploy (On-Premises)

```bash
# Clone this repo
git clone https://github.com/rameshsraj/zentra-agent.git /store/zentra-agent

# Run setup
cd /store/zentra-agent
sudo bash deploy/setup-agent.sh
```

The agent collects metrics via SSH/SNMP from your infrastructure
and pushes them to the Zentra API server.
READMEEOF

cat > .gitattributes <<'EOF'
bin/zentra-agent binary
EOF

git add -A
git config user.email "build@zentra.local"
git config user.name "Zentra Build Server"
git commit -m "Release ${VERSION_TAG} — $(date -u +%Y-%m-%d)" --allow-empty
git branch -M main
git push -u origin main --force

echo "  ✓ ${AGENT_REPO} published (${VERSION_TAG})"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo ""
echo "[4/4] Done!"
echo ""
echo "=============================================="
echo "  Published:"
echo "    ${RELEASE_REPO}: https://github.com/${GITHUB_USER}/${RELEASE_REPO}"
echo "    ${AGENT_REPO}:   https://github.com/${GITHUB_USER}/${AGENT_REPO}"
echo "  Version: ${VERSION_TAG}"
echo ""
echo "  GCP deploy:"
echo "    git clone https://github.com/${GITHUB_USER}/${RELEASE_REPO}.git /store/zentra"
echo "    cd /store/zentra && sudo bash deploy/setup-debian12.sh"
echo ""
echo "  Agent deploy:"
echo "    git clone https://github.com/${GITHUB_USER}/${AGENT_REPO}.git /store/zentra-agent"
echo "    cd /store/zentra-agent && sudo bash deploy/setup-agent.sh"
echo "=============================================="
