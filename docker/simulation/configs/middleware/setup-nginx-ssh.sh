#!/bin/sh
# Setup script for Nginx + SSH on Alpine
# Used by: mw-nginx-01

echo "[nginx-ssh] Installing SSH server..."
apk add --no-cache openssh-server openssh-sftp-server 2>/dev/null || true

echo "[nginx-ssh] Configuring SSH..."
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

# Enable root login and password auth — catch both commented and uncommented lines
sed -i 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
sed -i 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
sed -i 's/^[#[:space:]]*PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config

echo "[nginx-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[nginx-ssh] Starting Nginx..."
exec nginx -g "daemon off;"
