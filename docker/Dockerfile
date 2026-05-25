# =============================================================================
# Zentra — Runtime-Only Docker Image
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
        https://download.oracle.com/otn_software/linux/instantclient/2114000/oracle-instantclient-sqlplus-21.14.0.0.0-1.el8.x86_64.rpm \
    && dnf clean all

ENV LD_LIBRARY_PATH=/usr/lib/oracle/21/client64/lib:${LD_LIBRARY_PATH}
ENV PATH="/usr/lib/oracle/21/client64/bin:${PATH}"

# ---------------------------------------------------------------------------
# PostgreSQL configuration
# ---------------------------------------------------------------------------
ENV PGDATA=/var/lib/postgresql/data
ENV PG_MAJOR=16
ENV PATH="/usr/pgsql-16/bin:$PATH"

RUN mkdir -p /var/lib/postgresql/data /var/run/postgresql /var/log/postgresql \
    && chown -R postgres:postgres /var/lib/postgresql /var/run/postgresql /var/log/postgresql

# ---------------------------------------------------------------------------
# Copy pre-built backend binary (NO source code)
# ---------------------------------------------------------------------------
COPY target/release/zentra-server /usr/local/bin/zentra-server
RUN chmod +x /usr/local/bin/zentra-server

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
    && chmod 700 /root/.ssh

# ---------------------------------------------------------------------------
# Ports & Volumes
# ---------------------------------------------------------------------------
# 5010 = Nginx (frontend + API proxy)  — primary entry point
# 5006 = Backend API (internal, proxied by nginx)
# PostgreSQL (5434) is internal only - not exposed externally
EXPOSE 5010 5006

# Persist database data across container restarts
VOLUME ["/var/lib/postgresql/data"]

# ---------------------------------------------------------------------------
# Health check
# ---------------------------------------------------------------------------
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -sf http://localhost:5010/api/health | grep -q '"status"' || exit 1

# ---------------------------------------------------------------------------
# Entrypoint — initializes PostgreSQL on first run, then starts all services
# All credentials are passed via environment variables at runtime
# ---------------------------------------------------------------------------
ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["supervisord", "-c", "/etc/supervisord.conf"]
