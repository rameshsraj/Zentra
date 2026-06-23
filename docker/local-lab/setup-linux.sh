#!/bin/bash
# Plain Linux server — SSH + SNMP + CPU load simulation
set -e

echo "[lab-linux-01] Installing packages..."
dnf install -y net-snmp net-snmp-utils openssh-server procps-ng 2>/dev/null || true

# Configure SNMP
cat > /etc/snmp/snmpd.conf <<'SNMPCONF'
rocommunity public 0.0.0.0/0
syslocation "Local Lab — Linux Server"
syscontact admin@zentra.local
agentaddress udp:161
SNMPCONF

# Configure SSH
echo "[lab-linux-01] Configuring SSH..."
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

cat > /etc/ssh/sshd_config <<'EOF'
Port 22
PermitRootLogin yes
PasswordAuthentication yes
PubkeyAuthentication yes
AuthorizedKeysFile .ssh/authorized_keys
UsePAM no
Subsystem sftp /usr/libexec/openssh/sftp-server
EOF

# Start services
echo "[lab-linux-01] Starting SNMP daemon..."
/usr/sbin/snmpd -f -Lo -C -c /etc/snmp/snmpd.conf &

echo "[lab-linux-01] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[lab-linux-01] Rocky Linux server ready (SSH:22, SNMP:161)"

# Generate CPU/disk activity so the agent has metrics to collect
while true; do
    dd if=/dev/urandom of=/tmp/load-test bs=1M count=5 2>/dev/null || true
    rm -f /tmp/load-test
    sleep 30
done
