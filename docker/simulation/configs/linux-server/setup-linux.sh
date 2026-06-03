#!/bin/bash
# Setup script for Rocky Linux server simulation
# Installs SNMP daemon, SSH, and starts services

set -e

echo "[srv-linux] Installing packages..."
dnf install -y net-snmp net-snmp-utils openssh-server procps-ng cronie nginx 2>/dev/null || true

# Configure SSH
echo "[srv-linux] Configuring SSH..."
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

# Create a complete sshd_config (overwrite to avoid misconfiguration)
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

echo "[srv-linux] Rocky Linux server ready."
echo "[srv-linux] SSH: port 22, SNMP: port 161"

# Generate some CPU/disk activity periodically
while true; do
    dd if=/dev/urandom of=/tmp/load-test bs=1M count=5 2>/dev/null || true
    rm -f /tmp/load-test
    sleep 30
done
