#!/bin/sh
# Apache HTTP entrypoint - enable server-status, SSH, and start

# Install SSH server and chpasswd
echo "[apache-ssh] Installing SSH server..."
apk add --no-cache openssh-server openssh-sftp-server whois 2>/dev/null || true

echo "[apache-ssh] Configuring SSH..."
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

echo "[apache-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

# Enable status module and include our config
echo "Include conf/extra/httpd-status.conf" >> /usr/local/apache2/conf/httpd.conf

# Enable mod_status (already loaded in httpd:alpine)
sed -i 's/#LoadModule status_module/LoadModule status_module/' /usr/local/apache2/conf/httpd.conf

# Listen on 8080 for status
echo "Listen 8080" >> /usr/local/apache2/conf/httpd.conf

exec httpd-foreground
