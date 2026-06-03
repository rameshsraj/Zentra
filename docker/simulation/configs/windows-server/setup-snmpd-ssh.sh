#!/bin/sh
# Setup script for SNMP + SSH on Alpine-based containers
# Used by: srv-win-01, srv-win-02, sw-core-01, sw-access-01, fw-01, stor-01

echo "[snmpd-ssh] Installing SSH server..."
apk add --no-cache openssh-server openssh-sftp-server 2>/dev/null || true

echo "[snmpd-ssh] Configuring SSH..."
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
Subsystem sftp /usr/lib/openssh/sftp-server
EOF

echo "[snmpd-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[snmpd-ssh] Starting SNMP daemon..."
exec /usr/sbin/snmpd -f -Lo -C -c /etc/snmp/snmpd.conf
