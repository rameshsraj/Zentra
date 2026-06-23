#!/bin/sh
# Linux with Apache HTTP service — SSH + httpd server-status
set -e

echo "[lab-httpd-01] Installing SSH server..."
apk add --no-cache openssh-server openssh-sftp-server procps 2>/dev/null || true

# Configure SSH
echo "[lab-httpd-01] Configuring SSH..."
ssh-keygen -A 2>/dev/null || true
echo "root:zentra123" | chpasswd
mkdir -p /root/.ssh && chmod 700 /root/.ssh
touch /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys

cat > /etc/ssh/sshd_config <<'EOF'
Port 22
PermitRootLogin yes
PasswordAuthentication yes
PubkeyAuthentication yes
AuthorizedKeysFile .ssh/authorized_keys
UsePAM no
Subsystem sftp /usr/lib/openssh/sftp-server
EOF

# Enable Apache server-status module
cat >> /usr/local/apache2/conf/httpd.conf <<'HTTPD_STATUS'

# Zentra monitoring — server-status
LoadModule status_module modules/mod_status.so
<Location "/server-status">
    SetHandler server-status
    Require all granted
</Location>
ExtendedStatus On
HTTPD_STATUS

# Custom index page
echo "Zentra Lab — Apache HTTP Server" > /usr/local/apache2/htdocs/index.html

echo "[lab-httpd-01] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[lab-httpd-01] Starting Apache httpd..."
echo "[lab-httpd-01] Apache HTTP server ready (HTTP:80, Status:/server-status, SSH:22)"
exec httpd-foreground
