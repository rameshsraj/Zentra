#!/bin/bash
# Setup script for Rocky Linux server simulation WITH Zentra Agent
# Installs SNMP daemon, SSH, and starts the zentra-agent in REMOTE mode

set -e

echo "[srv-linux] Installing packages..."
dnf install -y net-snmp net-snmp-utils openssh-server procps-ng cronie nginx 2>/dev/null || true

# Configure SSH
echo "[srv-linux] Configuring SSH..."
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

cat > /etc/ssh/sshd_config << 'EOF'
Port 22
PermitRootLogin yes
PasswordAuthentication yes
PubkeyAuthentication yes
AuthorizedKeysFile .ssh/authorized_keys
UsePAM no
Subsystem sftp /usr/libexec/openssh/sftp-server
EOF

# Start services
echo "[srv-linux] Starting SNMP daemon..."
/usr/sbin/snmpd -f -Lo -C -c /etc/snmp/snmpd.conf &

echo "[srv-linux] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[srv-linux] Starting Nginx..."
nginx -g "daemon off;" 2>/dev/null &

# Start Zentra Agent (if binary is mounted)
if [ -x /opt/zentra-agent/zentra-agent ]; then
    echo "[srv-linux] Starting Zentra Agent in REMOTE mode..."
    echo "[srv-linux]   API_ENDPOINT=${API_ENDPOINT:-not set}"
    echo "[srv-linux]   DC_NAME=${DC_NAME:-$(hostname)}"

    # Create buffer directory
    mkdir -p /opt/zentra-agent/buffer /opt/zentra-agent/logs

    # Set defaults if not provided via environment
    export AGENT_MODE="${AGENT_MODE:-remote}"
    export API_ENDPOINT="${API_ENDPOINT:-http://zentra:5006}"
    export DC_NAME="${DC_NAME:-$(hostname)}"
    export BUFFER_DIR="${BUFFER_DIR:-/opt/zentra-agent/buffer}"
    export ZENTRA_LOG_LEVEL="${ZENTRA_LOG_LEVEL:-info}"

    /opt/zentra-agent/zentra-agent &
    echo "[srv-linux] Zentra Agent started (PID $!)"
else
    echo "[srv-linux] WARNING: zentra-agent binary not found at /opt/zentra-agent/zentra-agent"
    echo "[srv-linux] Agent monitoring disabled. Server will only be monitored via SNMP/SSH."
fi

echo "[srv-linux] Rocky Linux server ready."
echo "[srv-linux] SSH: port 22, SNMP: port 161"

# Generate some CPU/disk activity periodically
while true; do
    dd if=/dev/urandom of=/tmp/load-test bs=1M count=5 2>/dev/null || true
    rm -f /tmp/load-test
    sleep 30
done
