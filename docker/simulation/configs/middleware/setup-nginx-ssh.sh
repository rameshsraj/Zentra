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

echo "[nginx-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[nginx-ssh] Starting Nginx..."
exec nginx -g "daemon off;"
