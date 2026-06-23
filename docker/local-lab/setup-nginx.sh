#!/bin/sh
# Linux with Nginx service — SSH + Nginx status
set -e

echo "[lab-nginx-01] Installing SSH server..."
apk add --no-cache openssh-server openssh-sftp-server procps 2>/dev/null || true

# Configure SSH
echo "[lab-nginx-01] Configuring SSH..."
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

# Configure Nginx stub_status
cat > /etc/nginx/conf.d/status.conf <<'NGINX_STATUS'
server {
    listen 8080;
    server_name _;
    location /nginx_status {
        stub_status on;
        allow all;
    }
}
NGINX_STATUS

# Default site
cat > /etc/nginx/conf.d/default.conf <<'NGINX_DEFAULT'
server {
    listen 80;
    server_name _;
    location / {
        return 200 'Zentra Lab — Nginx Server\n';
        add_header Content-Type text/plain;
    }
}
NGINX_DEFAULT

echo "[lab-nginx-01] Starting SSH daemon..."
/usr/sbin/sshd -D &

echo "[lab-nginx-01] Starting Nginx..."
echo "[lab-nginx-01] Nginx server ready (HTTP:80, Status:8080, SSH:22)"
exec nginx -g "daemon off;"
