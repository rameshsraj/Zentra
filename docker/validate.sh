#!/bin/bash
# =============================================================================
# docker/validate.sh — Post-build validation for Zentra Docker image
# Tests all security, functionality, and hardening requirements (D01–D08)
#
# Usage:  bash docker/validate.sh
# =============================================================================
set +e

IMAGE="zentra:latest"
CONTAINER="zentra-validate"
PASS=0
FAIL=0

check() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then
        echo "  ✓ ${desc}"; ((PASS++))
    else
        echo "  ✗ ${desc}"; ((FAIL++))
    fi
}

echo "=== Zentra Docker Image Validation ==="
echo "Image: ${IMAGE}"
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 1. Image exists
# ─────────────────────────────────────────────────────────────────────────────
echo "[1] Image exists"
check "Image present" docker image inspect ${IMAGE}
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 2. No source code (D03)
# ─────────────────────────────────────────────────────────────────────────────
echo "[2] No source code in image (D03)"
check "No .rs files"      docker run --rm ${IMAGE} sh -c '! find / -name "*.rs" 2>/dev/null | grep -q .'
check "No Cargo.toml"     docker run --rm ${IMAGE} sh -c '! test -f /opt/zentra/Cargo.toml'
check "No Cargo.lock"     docker run --rm ${IMAGE} sh -c '! test -f /opt/zentra/Cargo.lock'
check "No .git dir"       docker run --rm ${IMAGE} sh -c '! test -d /opt/zentra/.git'
check "No node_modules"   docker run --rm ${IMAGE} sh -c '! find / -name node_modules -type d 2>/dev/null | grep -q .'
check "No frontend/src"   docker run --rm ${IMAGE} sh -c '! test -d /opt/zentra/frontend/src'
check "No crates/"        docker run --rm ${IMAGE} sh -c '! test -d /opt/zentra/crates'
check "No .toml files"    docker run --rm ${IMAGE} sh -c '! find /opt/zentra -name "*.toml" 2>/dev/null | grep -q .'
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 3. Build artifacts present (D02)
# ─────────────────────────────────────────────────────────────────────────────
echo "[3] Build artifacts present (D02)"
check "zentra-server binary"   docker run --rm ${IMAGE} test -x /usr/local/bin/zentra-server
check "Frontend dist"          docker run --rm ${IMAGE} test -f /opt/zentra/frontend/dist/index.html
check "Migrations dir"         docker run --rm ${IMAGE} test -d /opt/zentra/migrations
check "Build info"             docker run --rm ${IMAGE} test -f /opt/zentra/.build-info
check "Integrity manifest"     docker run --rm ${IMAGE} test -f /opt/zentra/.integrity
check "Manifest hash"          docker run --rm ${IMAGE} test -f /opt/zentra/.manifest-hash
check "Entrypoint exists"      docker run --rm ${IMAGE} test -x /usr/local/bin/docker-entrypoint.sh
check "Supervisord conf"       docker run --rm ${IMAGE} test -f /etc/supervisord.conf
check "Nginx conf"             docker run --rm ${IMAGE} test -f /etc/nginx/conf.d/zentra.conf
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 4. Security hardening (D06)
# ─────────────────────────────────────────────────────────────────────────────
echo "[4] Security hardening (D06)"
check "Binary is stripped"     docker run --rm ${IMAGE} sh -c 'file /usr/local/bin/zentra-server | grep -q stripped'
check "No gcc"                 docker run --rm ${IMAGE} sh -c '! which gcc 2>/dev/null'
check "No make"                docker run --rm ${IMAGE} sh -c '! which make 2>/dev/null'
check "No strace"              docker run --rm ${IMAGE} sh -c '! which strace 2>/dev/null'
check "zentra user exists"     docker run --rm ${IMAGE} id zentra
check "zentra user nologin"    docker run --rm ${IMAGE} sh -c 'getent passwd zentra | grep -q nologin'
check "Core dumps disabled"    docker run --rm ${IMAGE} sh -c 'grep -q "hard core 0" /etc/security/limits.conf'
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 5. Port exposure (D05)
# ─────────────────────────────────────────────────────────────────────────────
echo "[5] Port exposure (D05)"
check "Only port 5010 EXPOSE"  docker image inspect ${IMAGE} --format='{{.Config.ExposedPorts}}' | grep -q '5010' 
check "Port 5006 NOT exposed"  docker image inspect ${IMAGE} --format='{{.Config.ExposedPorts}}' | grep -qv '5006'
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 6. File permissions (D04)
# ─────────────────────────────────────────────────────────────────────────────
echo "[6] File permissions (D04)"
check "Binary perms (550)"     docker run --rm ${IMAGE} sh -c 'stat -c %a /usr/local/bin/zentra-server | grep -q 550'
check "SQL migrations (400)"   docker run --rm ${IMAGE} sh -c 'stat -c %a /opt/zentra/migrations/*.sql 2>/dev/null | head -1 | grep -q 400'
check "supervisord.conf (600)" docker run --rm ${IMAGE} sh -c 'stat -c %a /etc/supervisord.conf | grep -q 600'
check "nginx.conf (600)"       docker run --rm ${IMAGE} sh -c 'stat -c %a /etc/nginx/conf.d/zentra.conf | grep -q 600'
check "Integrity (400)"        docker run --rm ${IMAGE} sh -c 'stat -c %a /opt/zentra/.integrity | grep -q 400'
check "psql restricted (750)"  docker run --rm ${IMAGE} sh -c 'stat -c %a /usr/pgsql-16/bin/psql | grep -q 750'
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 7. Container lifecycle (D02 end-to-end)
# ─────────────────────────────────────────────────────────────────────────────
echo "[7] Container lifecycle (end-to-end)"
docker rm -f ${CONTAINER} 2>/dev/null || true

echo "  Starting container on port 15010..."
docker run -d --name ${CONTAINER} -p 15010:5010 ${IMAGE} >/dev/null

# Wait for services to come up (up to 60s)
echo "  Waiting for services..."
READY=false
for i in $(seq 1 60); do
    if curl -sf http://localhost:15010/api/health 2>/dev/null | grep -q healthy; then
        READY=true
        break
    fi
    sleep 1
done

if [ "${READY}" = true ]; then
    echo "  Services ready after ~${i}s"
else
    echo "  WARNING: Services not healthy after 60s — some checks may fail"
fi

check "Container running"       docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}$"
check "Health endpoint"         curl -sf http://localhost:15010/api/health | grep -q healthy
check "Frontend loads"          curl -sf http://localhost:15010/ | grep -q '<div id="root">'
check "Supervisord status"      docker exec ${CONTAINER} supervisorctl status | grep -q RUNNING
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 8. License mechanism (D08)
# ─────────────────────────────────────────────────────────────────────────────
echo "[8] License mechanism (D08)"
check "License file created"    docker exec ${CONTAINER} test -f /opt/zentra/.license
check "License in PG volume"    docker exec ${CONTAINER} test -f /var/lib/postgresql/data/.zentra-license
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 9. Tamper protection (D07)
# ─────────────────────────────────────────────────────────────────────────────
echo "[9] Tamper protection (D07)"
check "Integrity manifest OK"   docker exec ${CONTAINER} sha256sum -c /opt/zentra/.integrity --quiet
check "Manifest hash matches"   docker exec ${CONTAINER} sh -c '[ "$(cat /opt/zentra/.manifest-hash)" = "$(sha256sum /opt/zentra/.integrity | cut -d" " -f1)" ]'
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# 10. API bind address (D05)
# ─────────────────────────────────────────────────────────────────────────────
echo "[10] API bind address (D05)"
# Port 5006 = hex 138E, Port 5434 = hex 153A, 127.0.0.1 = hex 0100007F
check "API on 127.0.0.1 only"   docker exec ${CONTAINER} sh -c 'grep -qi "0100007F:138E" /proc/net/tcp'
check "PG on 127.0.0.1 only"    docker exec ${CONTAINER} sh -c 'grep -qi "0100007F:153A" /proc/net/tcp'
echo ""

# ─────────────────────────────────────────────────────────────────────────────
# Cleanup
# ─────────────────────────────────────────────────────────────────────────────
echo "Cleaning up..."
docker rm -f ${CONTAINER} >/dev/null 2>&1 || true

echo ""
echo "=== Results: ${PASS} passed, ${FAIL} failed ==="
if [ ${FAIL} -eq 0 ]; then
    echo "ALL CHECKS PASSED ✓"
else
    echo "SOME CHECKS FAILED ✗"
fi
exit ${FAIL}
