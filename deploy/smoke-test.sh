#!/usr/bin/env bash
# Zentra Post-Deploy Smoke Test
# Run after deployment to verify core functionality.
# Usage: ./deploy/smoke-test.sh [base_url]
set -euo pipefail

# Source port config
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true

BASE="${1:-http://127.0.0.1:${ZENTRA_API_PORT:-5006}}"
PASS=0
FAIL=0

check() {
    local name="$1"
    local url="$2"
    local expected_status="${3:-200}"
    local method="${4:-GET}"
    local body="${5:-}"

    if [ "$method" = "POST" ]; then
        code=$(curl -sf -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" -d "$body" "$url" 2>/dev/null || echo "000")
    else
        code=$(curl -sf -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000")
    fi

    if [ "$code" = "$expected_status" ]; then
        echo "  ✓ PASS: $name (HTTP $code)"
        PASS=$((PASS + 1))
    else
        echo "  ✗ FAIL: $name (expected $expected_status, got $code)"
        FAIL=$((FAIL + 1))
    fi
}

echo "============================================"
echo " Zentra Smoke Test"
echo " Target: ${BASE}"
echo " Date: $(date)"
echo "============================================"
echo ""

echo "--- Core Health ---"
check "Health endpoint" "${BASE}/api/health" "200"

echo ""
echo "--- Database Connectivity ---"
# Health endpoint checks DB internally
HEALTH_BODY=$(curl -sf "${BASE}/api/health" 2>/dev/null || echo "{}")
if echo "$HEALTH_BODY" | grep -q '"status":"ok"'; then
    echo "  ✓ PASS: Database connected (zentra)"
    PASS=$((PASS + 1))
else
    echo "  ✗ FAIL: Database check failed"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "--- Auth Endpoints ---"
check "Login endpoint reachable" "${BASE}/api/auth/login" "400" "POST" '{}'
# 400 is expected because we send empty body

echo ""
echo "--- API Endpoints ---"
check "System status" "${BASE}/api/system/status" "200"

echo ""
echo "--- Security Headers ---"
HEADERS=$(curl -sI "${BASE}/api/health" 2>/dev/null || echo "")
if echo "$HEADERS" | grep -qi "x-content-type-options"; then
    echo "  ✓ PASS: X-Content-Type-Options present"
    PASS=$((PASS + 1))
else
    echo "  ✗ FAIL: X-Content-Type-Options missing"
    FAIL=$((FAIL + 1))
fi

echo ""
echo "============================================"
echo " RESULTS: $PASS passed, $FAIL failed"
echo " Total:   $((PASS + FAIL)) checks"
if [ $FAIL -gt 0 ]; then
    echo " STATUS:  FAILED"
    exit 1
else
    echo " STATUS:  PASSED"
fi
echo "============================================"
