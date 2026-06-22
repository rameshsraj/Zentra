#!/usr/bin/env bash
# =============================================================================
# Zentra Agent — Container Entrypoint
# Generates SSH keys, distributes them to simulation nodes, then starts
# the zentra-agent in REMOTE mode.
#
# Environment variables (set via docker-compose):
#   API_ENDPOINT   — Zentra API URL (e.g. http://zentra:5006)
#   AGENT_API_KEY  — API key for agent authentication
#   AGENT_MODE     — "remote" (default)
#   DC_NAME        — Datacenter/agent name
#   SIM_NODES      — Space-separated list of "hostname:ip" pairs
#   SSH_PASSWORD   — Password for sim node SSH (default: zentra123)
# =============================================================================
set -uo pipefail

echo "=============================================="
echo "  Zentra Agent — Starting"
echo "  Endpoint: ${API_ENDPOINT:-not set}"
echo "  DC Name:  ${DC_NAME:-zentra-agent}"
echo "=============================================="

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
export AGENT_MODE="${AGENT_MODE:-remote}"
export API_ENDPOINT="${API_ENDPOINT:-http://zentra:5006}"
export DC_NAME="${DC_NAME:-zentra-agent}"
export BUFFER_DIR="${BUFFER_DIR:-/opt/zentra-agent/buffer}"
export RUST_LOG="${RUST_LOG:-info}"
SSH_PASSWORD="${SSH_PASSWORD:-zentra123}"
SSH_KEY="/root/.ssh/id_rsa"

# ---------------------------------------------------------------------------
# Generate SSH key pair (if not already present)
# ---------------------------------------------------------------------------
if [ ! -f "${SSH_KEY}" ]; then
    echo "[agent] Generating SSH key pair..."
    ssh-keygen -t rsa -b 4096 -f "${SSH_KEY}" -N "" -q
    echo "[agent] SSH key generated at ${SSH_KEY}"
fi

# SSH config — disable host key checking for lab environment
cat > /root/.ssh/config <<'EOF'
Host *
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
    LogLevel ERROR
    GSSAPIAuthentication no
    ConnectTimeout 8
EOF
chmod 600 /root/.ssh/config

# ---------------------------------------------------------------------------
# Wait for simulation nodes to be ready (SSH daemons need time to start)
# ---------------------------------------------------------------------------
if [ -n "${SIM_NODES:-}" ]; then
    echo "[agent] Waiting for simulation nodes to start SSH..."

    # Wait up to 120s for at least one node to respond
    FIRST_IP=$(echo "${SIM_NODES}" | tr ' ' '\n' | head -1 | cut -d: -f2)
    SSH_READY=false
    for i in $(seq 1 24); do
        if bash -c "echo > /dev/tcp/${FIRST_IP}/22" 2>/dev/null; then
            SSH_READY=true
            break
        fi
        printf "."
        sleep 5
    done
    echo ""

    if [ "${SSH_READY}" = true ]; then
        echo "[agent] SSH detected — waiting 10s for all nodes to finish setup..."
        sleep 10
    else
        echo "[agent] WARNING: SSH not detected after 120s — attempting key distribution anyway"
    fi

    # ---------------------------------------------------------------------------
    # Distribute SSH keys to all simulation nodes
    # ---------------------------------------------------------------------------
    echo "[agent] Distributing SSH keys to simulation nodes..."

    KEY_OK=0
    KEY_FAIL=0
    for entry in ${SIM_NODES}; do
        NODE_NAME=$(echo "${entry}" | cut -d: -f1)
        NODE_IP=$(echo "${entry}" | cut -d: -f2)

        COPY_OK=false
        for attempt in 1 2 3 4 5; do
            if cat "${SSH_KEY}.pub" | sshpass -p "${SSH_PASSWORD}" ssh \
                -o StrictHostKeyChecking=no \
                -o GSSAPIAuthentication=no \
                -o ConnectTimeout=8 \
                "root@${NODE_IP}" \
                'mkdir -p /root/.ssh && chmod 700 /root/.ssh && cat >> /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys' \
                2>/dev/null; then
                # Verify passwordless SSH works
                if timeout 8 ssh -i "${SSH_KEY}" \
                    -o BatchMode=yes \
                    -o ConnectTimeout=8 \
                    -o StrictHostKeyChecking=no \
                    -o GSSAPIAuthentication=no \
                    "root@${NODE_IP}" "echo ok" >/dev/null 2>&1; then
                    COPY_OK=true
                    break
                fi
            fi
            sleep 5
        done

        if [ "${COPY_OK}" = true ]; then
            echo "[agent]   ${NODE_NAME} (${NODE_IP}) — key installed"
            KEY_OK=$((KEY_OK + 1))
        else
            echo "[agent]   ${NODE_NAME} (${NODE_IP}) — FAILED"
            KEY_FAIL=$((KEY_FAIL + 1))
        fi
    done

    TOTAL=$((KEY_OK + KEY_FAIL))
    echo "[agent] SSH keys: ${KEY_OK}/${TOTAL} nodes OK"
    if [ "${KEY_FAIL}" -gt 0 ]; then
        echo "[agent] WARNING: ${KEY_FAIL} nodes failed SSH key setup"
    fi
else
    echo "[agent] No SIM_NODES configured — skipping SSH key distribution"
fi

# ---------------------------------------------------------------------------
# Wait for Zentra API to be reachable
# ---------------------------------------------------------------------------
echo "[agent] Waiting for Zentra API at ${API_ENDPOINT}..."
for i in $(seq 1 60); do
    if curl -sf "${API_ENDPOINT}/api/health" >/dev/null 2>&1; then
        echo "[agent] Zentra API is reachable (after ${i}s)"
        break
    fi
    sleep 2
done

# ---------------------------------------------------------------------------
# Start the agent
# ---------------------------------------------------------------------------
echo "[agent] Starting zentra-agent in ${AGENT_MODE} mode..."
exec /usr/local/bin/zentra-agent
