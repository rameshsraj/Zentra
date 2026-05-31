# =============================================================================
# Zentra — Hardened Runtime-Only Docker Image
# Rocky Linux 8.10 + PostgreSQL 16 + TimescaleDB + Nginx + zentra-server
# Contains ONLY pre-built artifacts — NO source code, NO credentials
#
# Build context: /store/Zentra/ (on the build server)
# Usage:  docker build -f docker/Dockerfile -t zentra:latest .
# =============================================================================
FROM rockylinux:8

LABEL maintainer="Zentra Team"
LABEL org.opencontainers.image.source="https://github.com/rameshsraj/zentra"
LABEL org.opencontainers.image.description="Zentra Infrastructure Monitoring — All-in-One"
LABEL org.opencontainers.image.version="1.0.0"

# Prevent interactive prompts
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# ---------------------------------------------------------------------------
# Install PostgreSQL 16 + TimescaleDB + Nginx + Supervisor
# ---------------------------------------------------------------------------
RUN dnf clean all \
    && dnf makecache \
    && dnf install -y epel-release glibc-langpack-en \
    && dnf install -y \
        https://download.postgresql.org/pub/repos/yum/reporpms/EL-8-x86_64/pgdg-redhat-repo-latest.noarch.rpm \
    && dnf -qy module disable postgresql \
    && dnf install -y \
        postgresql16-server \
        postgresql16-contrib \
        nginx \
        supervisor \
        curl \
        procps-ng \
        findutils \
        openssh-clients \
        sshpass \
        file \
    && dnf clean all

# Install TimescaleDB from official repo
RUN cat > /etc/yum.repos.d/timescaledb.repo <<'EOF'
[timescaledb]
name=TimescaleDB Repository
baseurl=https://packagecloud.io/timescale/timescaledb/el/8/$basearch
gpgcheck=0
enabled=1
EOF
RUN dnf install -y timescaledb-2-postgresql-16 \
    && dnf clean all

# ---------------------------------------------------------------------------
# Install Oracle Instant Client (for Oracle DB monitoring)
# ---------------------------------------------------------------------------
RUN dnf install -y libaio && \
    dnf install -y \
        https://download.oracle.com/otn_software/linux/instantclient/2114000/oracle-instantclient-basiclite-21.14.0.0.0-1.el8.x86_64.rpm \
    && dnf clean all

ENV LD_LIBRARY_PATH=/usr/lib/oracle/21/client64/lib:${LD_LIBRARY_PATH}
ENV PATH="/usr/lib/oracle/21/client64/bin:${PATH}"

# ---------------------------------------------------------------------------
# D06: Remove unnecessary development packages and reduce attack surface
# ---------------------------------------------------------------------------
RUN dnf remove -y gcc make cpp gdb strace ltrace 2>/dev/null || true \
    && dnf clean all \
    && rm -rf /var/cache/dnf /tmp/* /var/tmp/*

# Install e2fsprogs for chattr (used by entrypoint to make files immutable)
RUN dnf install -y e2fsprogs && dnf clean all

# Disable core dumps
RUN echo '* hard core 0' >> /etc/security/limits.conf \
    && echo '* soft core 0' >> /etc/security/limits.conf

# Remove unnecessary SUID/SGID binaries
RUN find / -perm /6000 -type f -exec chmod a-s {} \; 2>/dev/null || true

# Remove editors and scripting languages at build time
# NOTE: python3 is preserved — supervisord depends on it
RUN rm -f /usr/bin/vi /usr/bin/vim /usr/bin/nano /usr/bin/ed \
         /usr/bin/perl /usr/bin/lua /usr/bin/ruby 2>/dev/null || true

# ---------------------------------------------------------------------------
# PostgreSQL configuration
# ---------------------------------------------------------------------------
ENV PGDATA=/var/lib/postgresql/data
ENV PG_MAJOR=16
ENV PATH="/usr/pgsql-16/bin:$PATH"

RUN mkdir -p /var/lib/postgresql/data /var/run/postgresql /var/log/postgresql \
    && chown -R postgres:postgres /var/lib/postgresql /var/run/postgresql /var/log/postgresql

# ---------------------------------------------------------------------------
# D06: Create dedicated non-root runtime user for zentra-server
# ---------------------------------------------------------------------------
RUN useradd -r -s /sbin/nologin -d /opt/zentra zentra

# ---------------------------------------------------------------------------
# Copy pre-built backend binary (NO source code)
# ---------------------------------------------------------------------------
COPY target/release/zentra-server /usr/local/bin/zentra-server

# D03: Strip the binary to remove debug symbols and reduce size
RUN strip /usr/local/bin/zentra-server 2>/dev/null || true \
    && chown root:zentra /usr/local/bin/zentra-server \
    && chmod 550 /usr/local/bin/zentra-server

# ---------------------------------------------------------------------------
# Copy SQL migrations (needed at startup for schema + admin user creation)
# ---------------------------------------------------------------------------
COPY migrations/ /opt/zentra/migrations/

# ---------------------------------------------------------------------------
# Copy pre-built frontend dist (NO source code)
# ---------------------------------------------------------------------------
COPY frontend/dist/ /opt/zentra/frontend/dist/

# ---------------------------------------------------------------------------
# Configuration files
# ---------------------------------------------------------------------------
COPY docker/supervisord.conf /etc/supervisord.conf
COPY docker/nginx.conf /etc/nginx/conf.d/zentra.conf
COPY docker/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# Replace default nginx.conf with minimal wrapper
RUN rm -f /etc/nginx/conf.d/default.conf && \
    printf '%s\n' \
    'worker_processes auto;' \
    'error_log /var/log/nginx/error.log warn;' \
    'pid /run/nginx.pid;' \
    'events { worker_connections 1024; }' \
    'http {' \
    '    include /etc/nginx/mime.types;' \
    '    default_type application/octet-stream;' \
    '    sendfile on;' \
    '    keepalive_timeout 65;' \
    '    include /etc/nginx/conf.d/*.conf;' \
    '}' > /etc/nginx/nginx.conf

# Create log directories and SSH directory
RUN mkdir -p /opt/zentra/logs /var/log/supervisor /root/.ssh \
    && chmod 700 /root/.ssh \
    && chown zentra:zentra /opt/zentra/logs

# ---------------------------------------------------------------------------
# D02: Build metadata (written at image build time)
# ---------------------------------------------------------------------------
RUN echo "build_date=$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /opt/zentra/.build-info && \
    echo "image_version=1.0.0" >> /opt/zentra/.build-info && \
    chmod 400 /opt/zentra/.build-info

# ---------------------------------------------------------------------------
# D04: Restrict file permissions on sensitive contents
# ---------------------------------------------------------------------------
RUN chmod 400 /opt/zentra/migrations/*.sql && \
    chmod 600 /etc/supervisord.conf && \
    chmod 600 /etc/nginx/conf.d/zentra.conf && \
    chown root:postgres /usr/pgsql-16/bin/psql && \
    chmod 750 /usr/pgsql-16/bin/psql

# ---------------------------------------------------------------------------
# D03: Verify no source code leaked into the image
# ---------------------------------------------------------------------------
RUN test ! -d /opt/zentra/crates && \
    test ! -d /opt/zentra/.git && \
    test ! -f /opt/zentra/Cargo.toml && \
    test ! -d /opt/zentra/frontend/src && \
    test ! -d /opt/zentra/frontend/node_modules && \
    echo "[build] Source-code verification PASSED"

# ---------------------------------------------------------------------------
# D07: Generate integrity manifest for tamper protection
# ---------------------------------------------------------------------------
RUN sha256sum \
    /usr/local/bin/zentra-server \
    /opt/zentra/frontend/dist/index.html \
    /etc/nginx/conf.d/zentra.conf \
    /etc/supervisord.conf \
    /usr/local/bin/docker-entrypoint.sh \
    > /opt/zentra/.integrity && \
    chmod 400 /opt/zentra/.integrity && \
    sha256sum /opt/zentra/.integrity | cut -d' ' -f1 > /opt/zentra/.manifest-hash && \
    chmod 400 /opt/zentra/.manifest-hash

# ---------------------------------------------------------------------------
# D06: Final cleanup — remove build caches, temp files, shell history
# ---------------------------------------------------------------------------
RUN rm -rf /root/.bash_history /root/.cache /tmp/* /var/tmp/* && \
    history -c 2>/dev/null || true

# ---------------------------------------------------------------------------
# D09: Restrict entrypoint — make it read-only so it cannot be overwritten
# ---------------------------------------------------------------------------
RUN chmod 500 /usr/local/bin/docker-entrypoint.sh && \
    chown root:root /usr/local/bin/docker-entrypoint.sh

# ---------------------------------------------------------------------------
# D05: Only expose frontend port — API is internal (proxied by nginx)
# ---------------------------------------------------------------------------
# 5010 = Nginx (frontend + API proxy) — the ONLY externally accessible port
# PostgreSQL (5434) and API (5006) are internal only
EXPOSE 5010

# Persist database data across container restarts
VOLUME ["/var/lib/postgresql/data"]

# ---------------------------------------------------------------------------
# Health check
# ---------------------------------------------------------------------------
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -sf http://localhost:5010/api/health | grep -q '"status"' || exit 1

# ---------------------------------------------------------------------------
# Recommended runtime security flags (documented for operators):
#   docker run --read-only \
#     --tmpfs /tmp:rw,noexec,nosuid,size=64m \
#     --tmpfs /var/run:rw,noexec,nosuid,size=16m \
#     --tmpfs /var/log:rw,noexec,nosuid,size=128m \
#     --tmpfs /opt/zentra/logs:rw,noexec,nosuid,size=64m \
#     --security-opt=no-new-privileges:true \
#     --cap-drop ALL --cap-add CHOWN --cap-add SETUID --cap-add SETGID \
#     --cap-add DAC_OVERRIDE --cap-add FOWNER --cap-add NET_BIND_SERVICE \
#     --cap-add LINUX_IMMUTABLE \
#     -v zentra-data:/var/lib/postgresql/data \
#     -p 5010:5010 zentra:latest
# ---------------------------------------------------------------------------

# ---------------------------------------------------------------------------
# Entrypoint — initializes PostgreSQL on first run, then starts all services
# All credentials are passed via environment variables at runtime
# ---------------------------------------------------------------------------
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["supervisord", "-c", "/etc/supervisord.conf"]
