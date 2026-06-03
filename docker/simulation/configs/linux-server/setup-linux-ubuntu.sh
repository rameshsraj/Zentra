#!/bin/bash
# Setup script for Ubuntu server simulation
# Installs SNMP daemon, SSH, and starts services

set -e

echo "[srv-linux] Installing packages..."
apt-get update -qq
apt-get install -y -qq snmpd snmp openssh-server procps cron

# Configure SSH
echo "[srv-linux] Configuring SSH..."
mkdir -p /run/sshd
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys
# Enable root login and password auth — catch both commented and uncommented lines
sed -i 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
sed -i 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
sed -i 's/^[#[:space:]]*PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config

# Start services
echo "[srv-linux] Starting SNMP daemon..."
snmpd -f -Lo -C -c /etc/snmp/snmpd.conf &

echo "[srv-linux] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[srv-linux] Ubuntu server ready."
echo "[srv-linux] SSH: port 22, SNMP: port 161"

# Generate some CPU/disk activity periodically
while true; do
    dd if=/dev/urandom of=/tmp/load-test bs=1M count=5 2>/dev/null || true
    rm -f /tmp/load-test
    sleep 30
done
