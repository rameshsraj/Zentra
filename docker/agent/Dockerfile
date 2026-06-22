# =============================================================================
# Zentra Agent — Lightweight Collector Container
# Rocky Linux 8 + zentra-agent binary + SSH/SNMP tools
# Sits on the container lab subnet, collects metrics from all sim nodes,
# and pushes them to the Zentra API over HTTP.
#
# Contains ONLY the pre-built agent binary — NO source code
#
# Build context: /store/Zentra/ (on the build server)
# Usage:  docker build -f docker/agent/Dockerfile -t zentra-agent:latest .
# =============================================================================
FROM rockylinux:8

LABEL maintainer="Zentra Team"
LABEL org.opencontainers.image.description="Zentra Agent — Infrastructure Collector"
LABEL org.opencontainers.image.version="1.0.0"

# Prevent interactive prompts
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8

# ---------------------------------------------------------------------------
# Install SSH client, SNMP tools, and basic utilities
# ---------------------------------------------------------------------------
RUN dnf clean all \
    && dnf makecache \
    && dnf install -y \
        openssh-clients \
        sshpass \
        net-snmp \
        net-snmp-utils \
        curl \
        procps-ng \
        findutils \
    && dnf clean all \
    && rm -rf /var/cache/dnf /tmp/* /var/tmp/*

# ---------------------------------------------------------------------------
# Copy pre-built agent binary (NO source code)
# ---------------------------------------------------------------------------
COPY docker/simulation/bin/zentra-agent /usr/local/bin/zentra-agent
RUN chmod 555 /usr/local/bin/zentra-agent

# ---------------------------------------------------------------------------
# Copy entrypoint
# ---------------------------------------------------------------------------
COPY docker/agent/agent-entrypoint.sh /usr/local/bin/agent-entrypoint.sh
RUN chmod 555 /usr/local/bin/agent-entrypoint.sh

# ---------------------------------------------------------------------------
# Create working directories
# ---------------------------------------------------------------------------
RUN mkdir -p /opt/zentra-agent/buffer \
             /opt/zentra-agent/logs \
             /root/.ssh \
    && chmod 700 /root/.ssh

# ---------------------------------------------------------------------------
# Health check — agent should respond to targets fetch
# ---------------------------------------------------------------------------
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD pgrep -f zentra-agent > /dev/null || exit 1

ENTRYPOINT ["agent-entrypoint.sh"]
