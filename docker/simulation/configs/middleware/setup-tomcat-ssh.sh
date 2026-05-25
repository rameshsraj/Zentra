#!/bin/bash
# Setup script for Tomcat + SSH on Debian-based image
# Used by: mw-tomcat-01

echo "[tomcat-ssh] Installing SSH server..."
apt-get update -qq
apt-get install -y -qq openssh-server 2>/dev/null || true

echo "[tomcat-ssh] Configuring SSH..."
mkdir -p /run/sshd
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

sed -i 's/#PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
sed -i 's/PermitRootLogin prohibit-password/PermitRootLogin yes/' /etc/ssh/sshd_config
sed -i 's/#PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
sed -i 's/#PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config

echo "[tomcat-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[tomcat-ssh] Starting Tomcat..."
exec catalina.sh run
