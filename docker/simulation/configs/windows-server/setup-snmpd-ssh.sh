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

# Enable root login and password auth — catch both commented and uncommented lines
sed -i 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
sed -i 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
sed -i 's/^[#[:space:]]*PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config

echo "[snmpd-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[snmpd-ssh] Starting SNMP daemon..."
exec /usr/sbin/snmpd -f -Lo -C -c /etc/snmp/snmpd.conf
