# Zentra — Docker Configuration Runbook

**Version:** 1.0.0  
**Date:** 2026-05-31  
**Classification:** Internal Use Only — Learning & Operations  
**Owner:** Zentra Team

---

## Table of Contents

1. [Overview](#1-overview)
2. [Prerequisites](#2-prerequisites)
3. [Building the Docker Image](#3-building-the-docker-image)
4. [Environment Variables](#4-environment-variables)
5. [Running the Container](#5-running-the-container)
6. [Container Architecture](#6-container-architecture)
7. [Nginx Configuration](#7-nginx-configuration)
8. [Supervisord Configuration](#8-supervisord-configuration)
9. [License & Trial Period](#9-license--trial-period)
10. [Docker Image Hardening](#10-docker-image-hardening)
11. [Datacenter Simulation Lab](#11-datacenter-simulation-lab)
12. [Volume Management & Persistence](#12-volume-management--persistence)
13. [Backup & Recovery](#13-backup--recovery)
14. [Health Checks & Monitoring](#14-health-checks--monitoring)
15. [Upgrading & Redeployment](#15-upgrading--redeployment)
16. [Troubleshooting](#16-troubleshooting)

---

## 1. Overview

Zentra ships as a **single all-in-one Docker image** based on Rocky Linux 8. The image contains:

| Component | Technology | Internal Port | Exposed |
|-----------|-----------|---------------|---------|
| **Frontend** | React SPA served by Nginx | 5010 | **Yes** (only exposed port) |
| **Backend API** | Rust / Axum | 5006 | No — proxied by Nginx |
| **Database** | PostgreSQL 16 + TimescaleDB | 5434 | No — localhost only |
| **OTLP gRPC** | Axum HTTP/2 server | 4317 | No — internal |
| **OTLP HTTP** | Axum HTTP handler | 4318 | No — proxied via `/v1/*` |
| **Process Manager** | Supervisord | — | — |

The image is designed for **air-gapped, zero-config deployment**. Just `docker run` and go.

---

## 2. Prerequisites

| Requirement | Minimum |
|-------------|---------|
| Docker Engine | 20.10+ |
| Available RAM | 2 GB |
| Available Disk | 5 GB (image) + volume for DB |
| CPU | 2 cores |
| OS | Linux (recommended), Windows Docker Desktop, macOS Docker Desktop |

### Build Server Requirements (for building the image)

| Requirement | Details |
|-------------|---------|
| Rust toolchain | 1.95+ |
| Node.js | 18+ (for frontend build) |
| Docker buildx | For multi-arch if needed |
| Cargo target | `x86_64-unknown-linux-gnu` |

---

## 3. Building the Docker Image

### 3.1 Full Build Steps

```bash
# On the build server (e.g., 10.88.11.162)
cd /store/Zentra

# Step 1: Build the frontend
cd frontend
npm ci
npx vite build
cd ..

# Step 2: Build the Rust backend (release mode)
cargo build --release --bin zentra-server

# Step 3: Build the Docker image
docker build -f docker/Dockerfile -t zentra:latest .

# Step 4: Export as portable tarball
docker save zentra:latest | gzip > docker/zentra-latest.tar.gz
```

### 3.2 What Gets Copied Into the Image

| Source | Destination in Image | Purpose |
|--------|---------------------|---------|
| `target/release/zentra-server` | `/usr/local/bin/zentra-server` | Backend binary |
| `migrations/` | `/opt/zentra/migrations/` | SQL migrations |
| `frontend/dist/` | `/opt/zentra/frontend/dist/` | Frontend SPA |
| `docker/supervisord.conf` | `/etc/supervisord.conf` | Process manager config |
| `docker/nginx.conf` | `/etc/nginx/conf.d/zentra.conf` | Reverse proxy config |
| `docker/docker-entrypoint.sh` | `/usr/local/bin/docker-entrypoint.sh` | Container startup script |

### 3.3 What Is NOT in the Image

- No source code (`crates/`, `frontend/src/`, `Cargo.toml`)
- No `.git` directory
- No `node_modules`
- No credentials or API keys
- No development tools (gcc, make, gdb, strace)
- Verified at build time by Dockerfile checks

### 3.4 Image Layers

```
Layer 1: Rocky Linux 8 base
Layer 2: PostgreSQL 16 + TimescaleDB + Nginx + Supervisor + SSH tools
Layer 3: Oracle Instant Client 21c (for Oracle DB monitoring)
Layer 4: Dev tool removal + e2fsprogs (chattr)
Layer 5: zentra-server binary (stripped, 550 permissions, root:zentra)
Layer 6: SQL migrations (400 permissions)
Layer 7: Frontend dist
Layer 8: Config files (supervisord, nginx, entrypoint)
Layer 9: Integrity manifest (.integrity + .manifest-hash)
Layer 10: Final cleanup + security hardening
```

---

## 4. Environment Variables

### 4.1 Required Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `POSTGRES_USER` | `postgres` | PostgreSQL superuser name |
| `POSTGRES_PASSWORD` | Auto-generated | Database password (**set this for production**) |
| `POSTGRES_DB` | `zentra` | Database name |
| `POSTGRES_PORT` | `5434` | PostgreSQL port (internal only) |
| `ZENTRA_API_PORT` | `5006` | Backend API port (internal only) |
| `ZENTRA_JWT_SECRET` | Auto-generated | JWT signing secret (min 32 chars for prod) |
| `ZENTRA_ALLOW_INSECURE_JWT` | `1` | Set to `0` in production |
| `ZENTRA_ORG_ID` | `00000000-...-0001` | Default organization UUID |
| `ZENTRA_ALLOW_ORIGINS` | `*` | CORS allowed origins |
| `ZENTRA_MASTER_KEY` | Auto-generated | AES-256 encryption key (64 hex chars) |
| `RUST_LOG` | `info` | Log verbosity (trace/debug/info/warn/error) |

### 4.2 Auto-Generated Values

If not explicitly set, the entrypoint auto-generates:
- `POSTGRES_PASSWORD` — 24-char random alphanumeric
- `ZENTRA_JWT_SECRET` — 48-char random alphanumeric
- `ZENTRA_MASTER_KEY` — 64-char hex (32 bytes)
- `DATABASE_URL` — Constructed from PG vars using restricted `zentra_app` user

### 4.3 OpenTelemetry Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | — | OTLP collector endpoint |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `grpc` | Protocol: `grpc` or `http/protobuf` |
| `ZENTRA_OTEL_SELF_ENABLED` | `true` | Enable self-monitoring telemetry |
| `ZENTRA_OTEL_SAMPLE_RATIO` | `1.0` | Trace sampling ratio (0.0–1.0) |

### 4.4 Operational Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZENTRA_BIND_ADDR` | `127.0.0.1` | API bind address (do NOT change) |
| `ZENTRA_SKIP_LOCKDOWN` | `0` | Set to `1` to disable self-lockdown (**dev only**) |
| `ZENTRA_CLEAN_SEED_DATA` | `1` | Auto-remove seed demo data on first boot |

### 4.5 Example .env File

```env
POSTGRES_USER=postgres
POSTGRES_PASSWORD=MySecureP@ssw0rd!
POSTGRES_DB=zentra
POSTGRES_PORT=5434
ZENTRA_API_PORT=5006
ZENTRA_JWT_SECRET=your-super-secret-jwt-key-at-least-32-chars-long!!
ZENTRA_ALLOW_INSECURE_JWT=0
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
ZENTRA_ALLOW_ORIGINS=https://zentra.yourcompany.com
RUST_LOG=info
```

---

## 5. Running the Container

### 5.1 Quick Start (Zero Config)

```bash
docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 \
  --security-opt=no-new-privileges:true \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

Access at `http://localhost:5010`  
Default login: **admin@zentra.local** / **changeme**

### 5.2 Production Deployment (Fully Hardened)

```bash
docker run -d \
  --name zentra \
  --restart unless-stopped \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \
  --tmpfs /var/run:rw,noexec,nosuid,size=16m \
  --tmpfs /var/log:rw,noexec,nosuid,size=128m \
  --tmpfs /opt/zentra/logs:rw,noexec,nosuid,size=64m \
  --tmpfs /root/.ssh:rw,noexec,nosuid,size=4m \
  --security-opt=no-new-privileges:true \
  --cap-drop ALL \
  --cap-add CHOWN --cap-add SETUID --cap-add SETGID \
  --cap-add DAC_OVERRIDE --cap-add FOWNER \
  --cap-add NET_BIND_SERVICE --cap-add LINUX_IMMUTABLE \
  --env-file /opt/zentra/.env \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

### 5.3 Air-Gapped Deployment

```bash
# Transfer tarball to air-gapped server via USB/SCP
scp zentra-latest.tar.gz admin@air-gapped-server:/tmp/

# On the air-gapped server
docker load -i /tmp/zentra-latest.tar.gz
docker run -d -p 5010:5010 --name zentra \
  --security-opt=no-new-privileges:true \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

### 5.4 Port Mapping Options

| Scenario | Command Flag | Description |
|----------|-------------|-------------|
| Standard | `-p 5010:5010` | Expose on default port |
| Custom port | `-p 8080:5010` | Map to port 8080 |
| Host-only | `-p 127.0.0.1:5010:5010` | Only accessible from host |
| Multiple NICs | `-p 10.0.0.5:5010:5010` | Bind to specific NIC |

---

## 6. Container Architecture

### 6.1 Startup Sequence

```
docker run
  └─→ docker-entrypoint.sh
        ├─→ Phase 1: Integrity verification (SHA-256 manifest check)
        ├─→ Phase 2: License check / generation
        │     ├─→ First boot: Generate signed license token
        │     └─→ Subsequent boots: Verify HMAC + check expiry
        ├─→ Phase 3: Environment variable setup (auto-gen secrets)
        ├─→ Phase 4: PostgreSQL initialization (first boot only)
        │     ├─→ initdb
        │     ├─→ Configure port, listen_addresses, pg_hba.conf
        │     ├─→ Enable TimescaleDB
        │     ├─→ Create zentra_app user (least-privilege)
        │     └─→ Stop PG (supervisord will restart it)
        ├─→ Phase 5: Copy license to PG volume (persistence)
        ├─→ Phase 6: SSH key generation (for monitoring)
        ├─→ Phase 7: Self-lockdown (D09)
        │     ├─→ Make files immutable (chattr +i)
        │     ├─→ Remove package managers, editors, tools
        │     ├─→ Remove chattr itself
        │     └─→ Schedule shell removal (5s after startup)
        └─→ Phase 8: exec supervisord
              ├─→ Priority 10: PostgreSQL
              ├─→ Priority 20: zentra-server (as user 'zentra')
              └─→ Priority 30: Nginx
```

### 6.2 Internal Network Layout

```
┌─────────────────────────────────────────────────────────┐
│                Docker Container                          │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │              Nginx (:5010)                        │   │
│  │   ┌─────────────────────────────────────────┐    │   │
│  │   │  /         → serve frontend/dist        │    │   │
│  │   │  /api/*    → proxy_pass :5006           │    │   │
│  │   │  /v1/*     → proxy_pass :5006 (OTLP)   │    │   │
│  │   │  /metrics  → proxy_pass :5006 (int.)   │    │   │
│  │   └─────────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────┘   │
│                          │                               │
│                    proxy_pass                            │
│                          │                               │
│  ┌──────────────────────────────────────────────────┐   │
│  │         zentra-server (:5006)                     │   │
│  │   user: zentra (non-root)                        │   │
│  │   ┌──────────┐  ┌──────────┐  ┌──────────────┐  │   │
│  │   │  Routes   │  │  OTLP    │  │  gRPC :4317  │  │   │
│  │   │  /api/*   │  │  /v1/*   │  │  (HTTP/2)    │  │   │
│  │   └──────────┘  └──────────┘  └──────────────┘  │   │
│  └──────────────────────────────────────────────────┘   │
│                          │                               │
│                    sqlx::PgPool                          │
│                          │                               │
│  ┌──────────────────────────────────────────────────┐   │
│  │       PostgreSQL 16 + TimescaleDB (:5434)         │   │
│  │   user: postgres                                  │   │
│  │   data: /var/lib/postgresql/data (VOLUME)         │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
           │
     Only :5010 exposed
```

### 6.3 File System Layout

| Path | Contents | Owner | Permissions |
|------|----------|-------|-------------|
| `/usr/local/bin/zentra-server` | Backend binary | `root:zentra` | 550 |
| `/usr/local/bin/docker-entrypoint.sh` | Startup script | `root:root` | 500 |
| `/opt/zentra/migrations/` | SQL migration files | `root` | 400 per file |
| `/opt/zentra/frontend/dist/` | React SPA build | `root` | Read-only |
| `/opt/zentra/logs/` | Runtime logs | `zentra` | Writable |
| `/opt/zentra/.integrity` | SHA-256 manifest | `root` | 400 |
| `/opt/zentra/.manifest-hash` | Manifest hash | `root` | 400 |
| `/opt/zentra/.build-info` | Build metadata | `root` | 400 |
| `/opt/zentra/.license` | License token (fallback) | `root` | 400 |
| `/var/lib/postgresql/data/` | **VOLUME** — PG data | `postgres` | 700 |
| `/var/lib/postgresql/data/.zentra-license` | License token (primary) | `postgres` | 400 |
| `/etc/supervisord.conf` | Supervisord config | `root` | 600 |
| `/etc/nginx/conf.d/zentra.conf` | Nginx site config | `root` | 600 |
| `/root/.ssh/` | SSH keys for monitoring | `root` | 700 |

---

## 7. Nginx Configuration

Nginx serves as the sole external entry point on port **5010**.

### 7.1 Routing Rules

| Location | Action | Details |
|----------|--------|---------|
| `/` | Serve SPA | `try_files $uri $uri/ /index.html` |
| `/api/*` | Reverse proxy | → `127.0.0.1:5006` |
| `/v1/*` | OTLP proxy | → `127.0.0.1:5006` (max body 10 MB) |
| `/api/auth/login` | Rate-limited proxy | 5 req/min per IP, burst 3 |
| `/api/system/*` | Internal only | `allow 127.0.0.1; deny all` |
| `/metrics` | Internal only | Allow private networks, deny public |
| `.*.(env\|git\|sql\|sh)$` | Blocked | Returns 404 |

### 7.2 Security Headers

```
X-Content-Type-Options: nosniff
X-Frame-Options: SAMEORIGIN
X-XSS-Protection: 1; mode=block
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: camera=(), microphone=(), geolocation=()
Content-Security-Policy: default-src 'self'; ...
server_tokens: off (hide version)
```

### 7.3 Customizing Nginx

To customize Nginx config without rebuilding:

```bash
# Mount a custom config
docker run -d -p 5010:5010 \
  -v /path/to/custom-nginx.conf:/etc/nginx/conf.d/zentra.conf:ro \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

---

## 8. Supervisord Configuration

Supervisord manages all three services inside the container.

### 8.1 Service Table

| Service | Command | User | Priority | Auto-restart | Start Delay |
|---------|---------|------|----------|-------------|-------------|
| `postgresql` | `/usr/pgsql-16/bin/postgres -D ...` | `postgres` | 10 (first) | Yes | 5s |
| `zentra-server` | `/usr/local/bin/zentra-server` | `zentra` | 20 (second) | Yes, 10 retries | 3s |
| `nginx` | `/usr/sbin/nginx -g "daemon off;"` | `root` | 30 (third) | Yes | 2s |

### 8.2 Log Locations

| Service | stdout | stderr |
|---------|--------|--------|
| PostgreSQL | `/var/log/supervisor/postgresql.log` | `/var/log/supervisor/postgresql-error.log` |
| zentra-server | `/opt/zentra/logs/api.log` | `/opt/zentra/logs/api-error.log` |
| Nginx | `/opt/zentra/logs/nginx.log` | `/opt/zentra/logs/nginx-error.log` |
| Nginx access | `/opt/zentra/logs/nginx_access.log` | — |
| Supervisord | `/var/log/supervisor/supervisord.log` | — |

### 8.3 Managing Services (Before Lockdown)

```bash
# Check status
docker exec zentra supervisorctl status

# Restart a service
docker exec zentra supervisorctl restart zentra-server

# View logs
docker exec zentra tail -100 /opt/zentra/logs/api.log
```

> **Note:** After the 5-second lockdown, `docker exec` will not work because all shells are removed.

---

## 9. License & Trial Period

### 9.1 How It Works

- **First boot:** Entrypoint generates a signed license token
  - Format: `install_epoch:expiry_epoch:hostname:sha256_hmac`
  - HMAC key: SHA-256 hash of the `zentra-server` binary
  - Written to `/opt/zentra/.license`, then copied to PG volume
- **Trial duration:** 3 days (configurable via `LICENSE_DAYS` in entrypoint)
- **Subsequent boots:** Verifies HMAC signature + checks expiry

### 9.2 Graceful Degradation

After trial expires, the container **still starts** but in degraded mode:

| Aspect | Behavior |
|--------|----------|
| Startup | Continues normally (no crash) |
| Dashboard | ✅ Fully functional |
| All other pages | ❌ Hidden from navigation |
| Non-dashboard API calls | Returns `402 Payment Required` |
| Frontend | Redirects all routes to `/dashboard` |
| Warning banner | Amber bar at top of page |

### 9.3 Renewing

| Method | Steps | Data Loss |
|--------|-------|-----------|
| Fresh volume | `docker volume rm zentra-data` + re-run | **Yes** — all data lost |
| Rebuild image | Build new image → new binary hash → new license | No (if same volume) |
| Production license | Contact Zentra team | No |

---

## 10. Docker Image Hardening

The image implements comprehensive security hardening (D01–D11 prompts).

### 10.1 Build-Time Hardening

| Measure | Details |
|---------|---------|
| **Non-root user** | `zentra` user for API server (D06) |
| **Binary stripping** | Debug symbols removed (`strip`) |
| **No source code** | Verified at build time — no `.git`, `Cargo.toml`, `src/` |
| **Restricted permissions** | Migrations 400, configs 600, binary 550 |
| **Dev tools removed** | gcc, make, gdb, strace, editors removed |
| **SUID/SGID cleared** | All setuid/setgid bits cleared |
| **Core dumps disabled** | `* hard core 0` in limits.conf |
| **Integrity manifest** | SHA-256 checksums of critical files |

### 10.2 Runtime Lockdown (D09)

After the entrypoint completes initialization:

| Step | Action |
|------|--------|
| 1 | Make license files immutable (`chattr +i`) |
| 2 | Make config, binary, migrations, frontend immutable |
| 3 | Remove package managers (dnf, yum, rpm) |
| 4 | Remove editors (vi, vim, nano, ed) |
| 5 | Remove scripting languages (perl, lua, ruby) |
| 6 | Remove dangerous tools (wget, ftp, telnet, dd, gcc, gdb) |
| 7 | Remove `chattr` itself (immutable bits permanent) |
| 8 | Schedule shell removal (5s after supervisord starts) |

### 10.3 Runtime Security Flags

| Flag | Purpose |
|------|---------|
| `--read-only` | Read-only root filesystem |
| `--tmpfs /tmp:rw,noexec,nosuid` | Writable tmp without execute |
| `--security-opt=no-new-privileges:true` | No privilege escalation |
| `--cap-drop ALL` | Drop all Linux capabilities |
| `--cap-add CHOWN,SETUID,SETGID,...` | Add only required capabilities |
| `--cap-add LINUX_IMMUTABLE` | Required for `chattr +i` lockdown |

### 10.4 Debugging Mode

Set `ZENTRA_SKIP_LOCKDOWN=1` to disable all lockdown measures:

```bash
docker run -d -p 5010:5010 \
  -e ZENTRA_SKIP_LOCKDOWN=1 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

> **WARNING:** Never use `ZENTRA_SKIP_LOCKDOWN=1` in production.

---

## 11. Datacenter Simulation Lab

The project includes a full simulation lab (`docker/simulation/`) that creates 12 Docker containers mimicking a real datacenter.

### 11.1 Lab Network

| Platform | Network Type | Subnet | IP Range |
|----------|-------------|--------|----------|
| **Linux** | macvlan (real LAN) | Host subnet (e.g., `10.88.11.0/24`) | `.201` – `.212` |
| **Windows** | Docker bridge | `10.100.0.0/24` | `.11` – `.22` |

### 11.2 Lab Components — IP & Port Reference

#### Linux Host (example: 10.88.11.x)

| # | Component | Container | IP Address | Ports | Image | Role |
|---|-----------|-----------|------------|-------|-------|------|
| 1 | **Linux Server 01** | `srv-linux-01` | 10.88.11.201 | 22 (SSH), 161 (SNMP) | `rockylinux:8` | Web Server |
| 2 | **Linux Server 02** | `srv-linux-02` | 10.88.11.202 | 22 (SSH), 161 (SNMP) | `ubuntu:22.04` | App Server |
| 3 | **Windows Server 01** | `srv-win-01` | 10.88.11.203 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | AD/DNS |
| 4 | **Windows Server 02** | `srv-win-02` | 10.88.11.204 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | File Server |
| 5 | **Core Switch** | `sw-core-01` | 10.88.11.205 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Cisco Catalyst 9300 |
| 6 | **Access Switch** | `sw-access-01` | 10.88.11.206 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Arista 7050X |
| 7 | **Firewall** | `fw-01` | 10.88.11.207 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Palo Alto PA-3260 |
| 8 | **Storage Array** | `stor-01` | 10.88.11.208 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Dell PowerStore 500T |
| 9 | **Nginx** | `mw-nginx-01` | 10.88.11.209 | 22, 80, 8080 | `nginx:1.25-alpine` | Reverse Proxy |
| 10 | **Apache** | `mw-apache-01` | 10.88.11.210 | 22, 80, 8080 | `httpd:2.4-alpine` | HTTP Server |
| 11 | **Tomcat** | `mw-tomcat-01` | 10.88.11.211 | 22, 8080, 9090 (JMX) | `tomcat:9-jdk17` | App Server |
| 12 | **Oracle DB** | `db-oracle-01` | 10.88.11.212 | 22, 1521 (OCI) | `gvenzl/oracle-xe:21-slim` | Oracle 21c XE |

#### Windows Host (bridge network: 10.100.0.x)

| # | Component | Container | IP Address | Ports | Role |
|---|-----------|-----------|------------|-------|------|
| 1 | Linux Server 01 | `srv-linux-01` | 10.100.0.11 | 22, 161 | Rocky Linux 8 |
| 2 | Linux Server 02 | `srv-linux-02` | 10.100.0.12 | 22, 161 | Ubuntu 22.04 |
| 3 | Windows Server 01 | `srv-win-01` | 10.100.0.13 | 22, 161 | Windows 2022 |
| 4 | Windows Server 02 | `srv-win-02` | 10.100.0.14 | 22, 161 | Windows 2019 |
| 5 | Core Switch | `sw-core-01` | 10.100.0.15 | 22, 161 | Cisco 9300 |
| 6 | Access Switch | `sw-access-01` | 10.100.0.16 | 22, 161 | Arista 7050X |
| 7 | Firewall | `fw-01` | 10.100.0.17 | 22, 161 | Palo Alto |
| 8 | Storage Array | `stor-01` | 10.100.0.18 | 22, 161 | Dell PowerStore |
| 9 | Nginx | `mw-nginx-01` | 10.100.0.19 | 22, 80, 8080 | Reverse Proxy |
| 10 | Apache | `mw-apache-01` | 10.100.0.20 | 22, 80, 8080 | HTTP Server |
| 11 | Tomcat | `mw-tomcat-01` | 10.100.0.21 | 22, 8080, 9090 | App Server |
| 12 | Oracle DB | `db-oracle-01` | 10.100.0.22 | 22, 1521 | Oracle 21c XE |

### 11.3 Lab Credentials

| Service | Username | Password | Notes |
|---------|----------|----------|-------|
| **All containers (SSH)** | `root` | `zentra123` | Port 22 |
| **SNMP (all devices)** | — | Community: `public` | SNMP v2c, port 161, read-only |
| **Tomcat Manager** | `zentra` | `zentra123` | `http://<ip>:8080/manager` |
| **Nginx Status** | — | — | `http://<ip>:8080/nginx_status` |
| **Apache Status** | — | — | `http://<ip>:8080/server-status` |
| **Oracle (SYS)** | `SYS` | `zentra123` | SID: `XE`, Service: `ZENTRADB` |
| **Oracle (Monitor)** | `zentra_mon` | `zentra123` | Read-only monitoring user |

### 11.4 Starting & Stopping the Lab

**Linux:**
```bash
cd docker/simulation
sudo ./setup.sh
```

**Windows:**
```powershell
cd docker\simulation
powershell -ExecutionPolicy Bypass -File setup.ps1
```

**Stopping:**
```bash
docker compose -f docker/simulation/docker-compose.generated.yml down
```

### 11.5 Adding Lab Targets to Zentra

| Target Type | Where in Zentra UI | What to Enter |
|-------------|-------------------|---------------|
| Linux/Windows Servers | Infrastructure → Servers → Add Target | IP, port 22, root/zentra123 |
| Network Switches | Infrastructure → Network → Add Device | IP, SNMP community `public` |
| Firewall | Infrastructure → Network → Add Device | IP, SNMP community `public` |
| Storage Array | Infrastructure → Storage → Add Array | IP, SSH root/zentra123 |
| Nginx | Middleware → Nginx | IP, `http://<ip>:8080/nginx_status` |
| Apache | Middleware → Apache | IP, `http://<ip>:8080/server-status` |
| Tomcat | Middleware → Tomcat | IP:8080, `zentra/zentra123` |
| Oracle | Database → Oracle | IP:1521, SID `XE`, `zentra_mon/zentra123` |

---

## 12. Volume Management & Persistence

### 12.1 What Persists

The Docker volume (`zentra-data`) is mounted at `/var/lib/postgresql/data` and contains:

| Item | Purpose |
|------|---------|
| PostgreSQL database files | All monitoring data, users, config |
| `.zentra-license` | License token (survives container recreation) |
| Backup files (if created) | Database dumps |

### 12.2 Volume Commands

```bash
# List volumes
docker volume ls

# Inspect volume
docker volume inspect zentra-data

# Remove volume (DESTROYS ALL DATA)
docker volume rm zentra-data

# Backup volume
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/zentra-data-backup.tar.gz -C /data .
```

---

## 13. Backup & Recovery

### 13.1 Database Backup (SQL Dump)

```bash
# Must run BEFORE lockdown (within 5s of startup) or with ZENTRA_SKIP_LOCKDOWN=1
docker exec zentra su - postgres -c \
  "pg_dump -p 5434 zentra | gzip > /var/lib/postgresql/data/zentra-backup.sql.gz"

# Copy backup out
docker cp zentra:/var/lib/postgresql/data/zentra-backup.sql.gz ./zentra-backup.sql.gz
```

### 13.2 Volume Backup (Full)

```bash
# Stop container
docker stop zentra

# Backup entire volume
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/zentra-data-backup.tar.gz -C /data .

# Restart
docker start zentra
```

### 13.3 Restore from Backup

```bash
# Remove old volume
docker volume rm zentra-data

# Create new volume and restore
docker volume create zentra-data
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \
  tar xzf /backup/zentra-data-backup.tar.gz -C /data

# Start container
docker run -d -p 5010:5010 -v zentra-data:/var/lib/postgresql/data zentra:latest
```

---

## 14. Health Checks & Monitoring

### 14.1 Built-In Health Check

The Dockerfile defines a health check:

```
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3
  CMD curl -sf http://localhost:5010/api/health | grep -q '"status"' || exit 1
```

### 14.2 Manual Health Checks

```bash
# API health
curl -sf http://localhost:5010/api/health
# → {"database":true,"status":"healthy","subscribers":0,"version":"1.0.0"}

# Database health
curl -sf http://localhost:5010/api/system/db

# License status
curl -sf http://localhost:5010/api/system/license-status

# Container health status
docker inspect --format='{{.State.Health.Status}}' zentra
```

### 14.3 Logs

```bash
# All container output
docker logs zentra
docker logs --tail 100 -f zentra

# Specific service logs (before lockdown)
docker exec zentra tail -100 /opt/zentra/logs/api.log
docker exec zentra tail -100 /opt/zentra/logs/nginx_access.log
```

---

## 15. Upgrading & Redeployment

### 15.1 Standard Upgrade

```bash
# 1. Build new image
docker build -f docker/Dockerfile -t zentra:latest .

# 2. Stop old container
docker stop zentra && docker rm zentra

# 3. Start with same volume (data preserved)
docker run -d --name zentra -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest

# Migrations run automatically on startup
```

### 15.2 Rollback

```bash
# Tag current image before upgrading
docker tag zentra:latest zentra:previous

# If upgrade fails, rollback
docker stop zentra && docker rm zentra
docker run -d --name zentra -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:previous
```

---

## 16. Troubleshooting

### 16.1 Common Issues

| Problem | Cause | Solution |
|---------|-------|---------|
| "License expired" on fresh install | Binary hash mismatch (rebuilt binary with existing volume) | Delete volume: `docker volume rm zentra-data` and re-run |
| "Integrity verification failed" | Image files modified post-build | Use the official image tarball |
| Container won't start | Port conflict or DB corruption | Check `docker logs zentra` |
| API returns 402 | Trial expired (degraded mode) | Only Dashboard available; get a license |
| `docker exec` fails | Shells removed by lockdown (D09) | By design — use `ZENTRA_SKIP_LOCKDOWN=1` for debug |
| Cannot install packages | Package managers removed | By design — rebuild image instead |
| SSH monitoring fails | SSH keys not configured | Copy container's public key to target server |
| OTLP ingest returns 401 | Invalid/missing API key | Generate at `/otel-security` |
| OTLP ingest returns 429 | Rate limit exceeded | Increase `rate_limit_rpm` for the API key |
| Slow queries | Missing indexes or data volume | Check TimescaleDB compression/retention |
| Nginx 502 Bad Gateway | Backend not ready yet | Wait 30s for startup; check API logs |
| "Permission denied" errors | Volume ownership mismatch | Run container with `--user root` once to fix permissions |

### 16.2 Debugging Checklist

```bash
# 1. Check container status
docker ps -a | grep zentra

# 2. Check health
docker inspect --format='{{.State.Health.Status}}' zentra

# 3. View startup logs
docker logs zentra 2>&1 | head -50

# 4. Check if API responds
curl -sf http://localhost:5010/api/health

# 5. Check database
curl -sf http://localhost:5010/api/system/db

# 6. For interactive debugging (dev only)
docker run -it --rm \
  -e ZENTRA_SKIP_LOCKDOWN=1 \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest /bin/bash
```

---

*— End of Docker Runbook —*

*Zentra Team • Internal Use Only*
