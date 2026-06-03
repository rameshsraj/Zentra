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

# Create sshd_config if missing (Alpine may not have it by default)
if [ ! -f /etc/ssh/sshd_config ]; then
    cat > /etc/ssh/sshd_config << 'EOF'
Port 22
PermitRootLogin yes
PasswordAuthentication yes
PubkeyAuthentication yes
EOF
else
    # Enable root login and password auth — catch both commented and uncommented lines
    sed -i 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config
    sed -i 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication yes/' /etc/ssh/sshd_config
    sed -i 's/^[#[:space:]]*PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config
fi

echo "[apache-ssh] Starting SSH daemon..."
/usr/sbin/sshd -D &

# Enable status module and include our config
echo "Include conf/extra/httpd-status.conf" >> /usr/local/apache2/conf/httpd.conf

# Enable mod_status (already loaded in httpd:alpine)
sed -i 's/#LoadModule status_module/LoadModule status_module/' /usr/local/apache2/conf/httpd.conf

# Listen on 8080 for status
echo "Listen 8080" >> /usr/local/apache2/conf/httpd.conf

exec httpd-foreground
