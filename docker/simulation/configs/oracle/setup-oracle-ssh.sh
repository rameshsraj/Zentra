#!/bin/bash
# Setup script to add SSH to Oracle XE container
# This runs as a sidecar-like init before Oracle starts
# Used by: db-oracle-01

echo "[oracle-ssh] Installing SSH server..."
microdnf install -y openssh-server openssh-clients passwd 2>/dev/null || \
    dnf install -y openssh-server openssh-clients passwd 2>/dev/null || \
    yum install -y openssh-server openssh-clients passwd 2>/dev/null || true

echo "[oracle-ssh] Configuring SSH..."
# Generate host keys (required for sshd to start)
ssh-keygen -A 2>/dev/null || true
# Ensure individual host key types exist
for ktype in rsa ecdsa ed25519; do
    kfile="/etc/ssh/ssh_host_${ktype}_key"
    if [ ! -f "${kfile}" ]; then
        ssh-keygen -t ${ktype} -f "${kfile}" -N "" -q 2>/dev/null || true
    fi
done

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

echo "[oracle-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &
SSHD_PID=$!
sleep 1
if kill -0 ${SSHD_PID} 2>/dev/null; then
    echo "[oracle-ssh] SSH ready on port 22 (pid ${SSHD_PID})"
else
    echo "[oracle-ssh] ERROR: sshd failed to start, trying with debug..."
    /usr/sbin/sshd -d -e &
fi
# Hand off to the original Oracle entrypoint
exec /container-entrypoint.sh "$@"
