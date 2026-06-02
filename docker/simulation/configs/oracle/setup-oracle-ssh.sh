#!/bin/bash
# Setup script to add SSH to Oracle XE container
# This runs as a sidecar-like init before Oracle starts
# Used by: db-oracle-01

echo "[oracle-ssh] Installing SSH server..."
microdnf install -y openssh-server openssh-clients 2>/dev/null || \
    dnf install -y openssh-server openssh-clients 2>/dev/null || \
    yum install -y openssh-server openssh-clients 2>/dev/null || true

echo "[oracle-ssh] Configuring SSH..."
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

# Oracle XE image may have a minimal sshd_config
if [ -f /etc/ssh/sshd_config ]; then
    # Enable root login and password auth — catch both commented and uncommented lines
    sed -i 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
    sed -i 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
    sed -i 's/^[#[:space:]]*PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config
fi

echo "[oracle-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[oracle-ssh] SSH ready on port 22"
# Hand off to the original Oracle entrypoint
exec /container-entrypoint.sh "$@"
