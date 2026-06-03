#!/bin/bash
# Setup script for Tomcat + SSH on Debian-based image
# Used by: mw-tomcat-01

export DEBIAN_FRONTEND=noninteractive

echo "[tomcat-ssh] Installing SSH server..."
apt-get update -qq
apt-get install -y -qq -o Dpkg::Options::="--force-confold" -o Dpkg::Options::="--force-confdef" openssh-server

echo "[tomcat-ssh] Configuring SSH..."
mkdir -p /run/sshd
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

echo "[tomcat-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[tomcat-ssh] Starting Tomcat..."
exec catalina.sh run
