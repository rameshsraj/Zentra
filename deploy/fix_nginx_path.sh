#!/usr/bin/env bash
set -euo pipefail

# Dynamically resolve the public IP from GCP metadata (no hardcoding)
EXT_IP=$(curl -sf -H 'Metadata-Flavor: Google' \
  'http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip' \
  2>/dev/null \
  || curl -sf --max-time 5 https://ifconfig.me 2>/dev/null \
  || hostname -I | awk '{print $1}')

echo "[fix-nginx] External IP: ${EXT_IP}"
echo "[fix-nginx] Setting root to /store/Zentra/frontend-dist"

mkdir -p /store/Zentra/logs

# Write nginx config — use quoted heredoc (<<'EOF') so nginx variables like
# $host, $remote_addr are written literally; only EXT_IP is substituted via sed.
cat > /etc/nginx/sites-available/zentra <<'NGINXEOF'
server {
    listen 5010;
    server_name _ EXTERNAL_IP_PLACEHOLDER;

    root /store/Zentra/frontend-dist;
    index index.html;

    location /api/ {
        proxy_pass         http://127.0.0.1:5006;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header   Upgrade           $http_upgrade;
        proxy_set_header   Connection        "upgrade";
        proxy_read_timeout 300s;
        proxy_send_timeout 300s;
    }

    location /otlp/ {
        proxy_pass         http://127.0.0.1:5006;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
    }

    location / {
        try_files $uri $uri/ /index.html;
    }

    access_log /store/Zentra/logs/nginx-access.log;
    error_log  /store/Zentra/logs/nginx-error.log;
}
NGINXEOF

# Substitute the placeholder with the actual IP
sed -i "s/EXTERNAL_IP_PLACEHOLDER/${EXT_IP}/" /etc/nginx/sites-available/zentra

ln -sf /etc/nginx/sites-available/zentra /etc/nginx/sites-enabled/zentra
rm -f /etc/nginx/sites-enabled/default

nginx -t
systemctl reload nginx

echo "[fix-nginx] Done. Frontend: http://${EXT_IP}:5010"
