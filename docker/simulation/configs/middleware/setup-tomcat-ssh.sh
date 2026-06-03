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

# Ensure sshd_config exists and enable root login + password auth
if [ -f /etc/ssh/sshd_config ]; then
    sed -i 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
    sed -i 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
    sed -i 's/^[#[:space:]]*PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config
else
    cat > /etc/ssh/sshd_config << 'EOF'
Port 22
PermitRootLogin yes
PasswordAuthentication yes
PubkeyAuthentication yes
EOF
fi

echo "[tomcat-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[tomcat-ssh] Starting Tomcat..."
exec catalina.sh run
