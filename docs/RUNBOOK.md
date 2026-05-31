# Zentra — Operations Runbook

**Version:** 1.0.0  
**Date:** 2026-05-31  
**Classification:** Internal Use Only — Learning & Operations  
**Owner:** Zentra Team

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Architecture](#2-architecture)
3. [Technology Stack](#3-technology-stack)
4. [Crate & Plugin Structure](#4-crate--plugin-structure)
5. [Environment Configuration](#5-environment-configuration)
6. [Lab Components — IP & Port Reference](#6-lab-components--ip--port-reference)
7. [Docker Deployment](#7-docker-deployment)
8. [Docker Image Hardening](#8-docker-image-hardening)
9. [License & Trial Period](#9-license--trial-period)
10. [Authentication & RBAC](#10-authentication--rbac)
11. [API Reference](#11-api-reference)
12. [Frontend Modules](#12-frontend-modules)
13. [OpenTelemetry Configuration](#13-opentelemetry-configuration)
14. [Database Schema](#14-database-schema)
15. [Monitoring Targets](#15-monitoring-targets)
16. [Alerting System](#16-alerting-system)
17. [AI & Analytics](#17-ai--analytics)
18. [Security Module](#18-security-module)
19. [Network Monitoring](#19-network-monitoring)
20. [Middleware Monitoring](#20-middleware-monitoring)
21. [Connection Pooling](#21-connection-pooling)
22. [Backup & Recovery](#22-backup--recovery)
23. [Troubleshooting](#23-troubleshooting)

---

## 1. Project Overview

Zentra is an all-in-one infrastructure monitoring platform built with a Rust backend and React frontend. It runs as a single Docker container that includes:

- **Backend API** — Rust/Axum on port 5006 (internal)
- **Frontend SPA** — React/Vite served by Nginx on port 5010
- **Database** — PostgreSQL 16 + TimescaleDB (internal, port 5434)
- **Process Manager** — Supervisord managing all three services

The platform monitors servers, databases (PostgreSQL, Oracle), middleware (Nginx, Apache, Tomcat, HAProxy, Redis, Kafka, etc.), network devices, storage arrays, and security events. It includes OpenTelemetry ingest, AI-powered analytics, and a built-in alerting engine.

---

## 2. Architecture

```
                         ┌─────────────────────────────────────────────────────┐
                         │              Docker Container (Rocky Linux 8)       │
                         │                                                     │
  Users ──► :5010 ──►    │  ┌─────────┐    ┌──────────────┐   ┌────────────┐  │
                         │  │  Nginx   │───▶│ zentra-server│──▶│ PostgreSQL │  │
                         │  │  :5010   │    │   :5006      │   │   :5434    │  │
                         │  └─────────┘    │  (Rust/Axum) │   │ +TimescaleDB│ │
                         │                  └──────────────┘   └────────────┘  │
                         │                         │                           │
                         │                  ┌──────┴───────┐                   │
                         │                  │  Supervisord  │                  │
                         │                  └──────────────┘                   │
                         └─────────────────────────────────────────────────────┘

  OTLP Clients ──► :4317 (gRPC) ──► zentra-server (ingest pipeline)
                ──► :4318 (HTTP) ──► zentra-server (/v1/traces, /v1/metrics, /v1/logs)
```

**Key design principles:**
- Single container, zero external dependencies
- Only port 5010 exposed externally (Nginx proxy)
- API and database are internal-only (127.0.0.1)
- Signed license with tamper detection
- Self-lockdown after startup (no shells, immutable files)

---

## 3. Technology Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| **Backend** | Rust + Axum | 1.95+ / Axum 0.7 |
| **Frontend** | React + TypeScript + Vite | React 18 |
| **UI Framework** | TailwindCSS + Lucide Icons | TailwindCSS 3 |
| **State Management** | TanStack React Query | v5 |
| **Database** | PostgreSQL + TimescaleDB | PG 16 + TSDB 2 |
| **Process Manager** | Supervisord | 4.2 |
| **Reverse Proxy** | Nginx | Latest (Rocky 8) |
| **Container** | Docker (Rocky Linux 8) | — |
| **Telemetry** | OpenTelemetry SDK | 0.27 |
| **Crypto** | Ed25519 (ring), SHA-256, AES-256-GCM | — |
| **Auth** | JWT (HS256) + MFA (TOTP) | — |
| **SSH** | OpenSSH + sshpass | — |

---

## 4. Crate & Plugin Structure

### Backend Crates (`crates/`)

| Crate | Purpose |
|-------|---------|
| `zentra-api` | Main Axum HTTP server, routes, handlers, middleware |
| `zentra-core` | Shared models, crypto (AES, license verification), telemetry init, SSH pool |
| `zentra-db` | Database pool management, repository layer, migration support |
| `zentra-stream` | Event bus (tokio broadcast), notification routing, alert rule evaluation |
| `zentra-agent` | Standalone agent binary for remote server metric collection |
| `zentra-ingestion` | OTLP payload parsing and batch ingest pipeline |
| `zentra-query` | Query services for metrics, logs, traces |
| `zentra-scrape` | Prometheus-compatible scrape target management |
| `zentra-ml` | ML model integration for anomaly detection |
| `zentra-predict` | Predictive alerting and capacity forecasting |
| `zentra-log-engine` | Log parsing, discovery rules, structured log processing |
| `zentra-netflow` | NetFlow/sFlow/IPFIX collector and analysis |
| `zentra-drilldown` | Deep-dive analytics for infrastructure metrics |

### Plugins (`plugins/`)

| Plugin | Purpose |
|--------|---------|
| `server-plugin` | Linux/Windows server metrics (CPU, memory, disk, network) via SSH |
| `network-plugin` | SNMP polling, device discovery, topology mapping |
| `database-plugin` | PostgreSQL, Oracle, MySQL monitoring |
| `storage-plugin` | Storage array monitoring (Dell, NetApp, Pure Storage) |
| `security-plugin` | IDS/IPS rules, firewall logs, threat detection |
| `cloud-plugin` | AWS, Azure, GCP cloud resource monitoring |
| `ai-plugin` | AI/ML insights, anomaly detection, RCA |
| `license-plugin` | Third-party software license tracking (Oracle, VMware, etc.) |

---

## 5. Environment Configuration

### Required Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `POSTGRES_USER` | `postgres` | PostgreSQL superuser |
| `POSTGRES_PASSWORD` | Auto-generated | Database password |
| `POSTGRES_DB` | `zentra` | Database name |
| `POSTGRES_PORT` | `5434` | PostgreSQL port (internal) |
| `ZENTRA_API_PORT` | `5006` | Backend API port (internal) |
| `ZENTRA_JWT_SECRET` | Auto-generated | JWT signing secret (min 32 chars for production) |
| `ZENTRA_ALLOW_INSECURE_JWT` | `1` | Set to `0` in production to enforce strong JWT secret |
| `ZENTRA_ORG_ID` | `00000000-...0001` | Default organization UUID |
| `ZENTRA_ALLOW_ORIGINS` | `*` | CORS allowed origins |
| `ZENTRA_BIND_ADDR` | `127.0.0.1` | API bind address (internal only) |
| `RUST_LOG` | `info` | Log verbosity (trace/debug/info/warn/error) |
| `DATABASE_URL` | Auto-constructed | Full PostgreSQL connection string |
| `ZENTRA_MASTER_KEY` | Auto-generated | AES-256 key for credential encryption (64 hex chars) |

### OpenTelemetry Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | — | OTLP collector endpoint (e.g., `http://collector:4317`) |
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `grpc` | Protocol: `grpc` or `http/protobuf` |
| `ZENTRA_OTEL_SELF_ENABLED` | `true` | Enable self-monitoring telemetry |
| `ZENTRA_OTEL_SAMPLE_RATIO` | `1.0` | Trace sampling ratio (0.0–1.0) |
| `ZENTRA_OTLP_SAMPLE_RATIO` | `1.0` | Receiver-side sampling for ingest |
| `ZENTRA_METRICS_CARDINALITY_CAP_*` | `1000` | Per-key metric label cardinality cap |

### Example `.env` File

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

## 6. Lab Components — IP & Port Reference

The Zentra project includes a full **Datacenter Simulation Lab** (`docker/simulation/`) that spins up 12 Docker containers simulating real infrastructure: Linux/Windows servers, network switches, a firewall, a storage array, middleware services, and an Oracle database.

### 6.1 Zentra Main Container

| Component | Container Name | IP Address | Port(s) | Protocol | Description |
|-----------|---------------|------------|---------|----------|-------------|
| **Zentra Platform** | `zentra` | Host network | **5010** | HTTP | Frontend (Nginx) — main UI entry point |
| — API (internal) | — | 127.0.0.1 | 5006 | HTTP | Backend API (Axum) — only accessible via Nginx |
| — PostgreSQL (internal) | — | 127.0.0.1 | 5434 | TCP | PostgreSQL 16 + TimescaleDB — not exposed |
| — OTLP gRPC (internal) | — | 0.0.0.0 | 4317 | gRPC | OpenTelemetry gRPC ingest |
| — OTLP HTTP (internal) | — | 0.0.0.0 | 4318 | HTTP | OpenTelemetry HTTP ingest |

> **Access URL:** `http://<host-ip>:5010`  
> **Default login:** `admin@zentra.local` / `changeme`

### 6.2 Simulation Lab Containers

The lab uses a Docker network (`zentra-dc`). IP assignment depends on the host network:

- **Linux (macvlan):** IPs are assigned from the host's subnet starting at offset `.201` (e.g., `10.88.11.201`)
- **Windows (bridge):** IPs are assigned from `10.100.0.0/24` starting at `.11` (e.g., `10.100.0.11`)

The setup scripts (`setup.sh` / `setup.ps1`) auto-detect the network and assign IPs.

#### Linux Host Example (10.88.11.x subnet)

| # | Component | Container | IP Address | Port(s) | Image | Description |
|---|-----------|-----------|------------|---------|-------|-------------|
| 1 | **Linux Server 01** | `srv-linux-01` | 10.88.11.201 | 22 (SSH), 161 (SNMP) | `rockylinux:8` | Rocky Linux 8 — Web Server role |
| 2 | **Linux Server 02** | `srv-linux-02` | 10.88.11.202 | 22 (SSH), 161 (SNMP) | `ubuntu:22.04` | Ubuntu 22.04 — App Server role |
| 3 | **Windows Server 01** | `srv-win-01` | 10.88.11.203 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Simulated Windows Server 2022 (AD/DNS) |
| 4 | **Windows Server 02** | `srv-win-02` | 10.88.11.204 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Simulated Windows Server 2019 (File Server) |
| 5 | **Core Switch** | `sw-core-01` | 10.88.11.205 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Simulated Cisco Catalyst 9300 |
| 6 | **Access Switch** | `sw-access-01` | 10.88.11.206 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Simulated Arista 7050X |
| 7 | **Firewall** | `fw-01` | 10.88.11.207 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Simulated Palo Alto PA-3260 |
| 8 | **Storage Array** | `stor-01` | 10.88.11.208 | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Simulated Dell PowerStore 500T |
| 9 | **Nginx** | `mw-nginx-01` | 10.88.11.209 | 22 (SSH), 80 (HTTP), 8080 (status) | `nginx:1.25-alpine` | Nginx Reverse Proxy |
| 10 | **Apache** | `mw-apache-01` | 10.88.11.210 | 22 (SSH), 80 (HTTP), 8080 (status) | `httpd:2.4-alpine` | Apache HTTP Server |
| 11 | **Tomcat** | `mw-tomcat-01` | 10.88.11.211 | 22 (SSH), 8080 (HTTP), 9090 (JMX) | `tomcat:9-jdk17` | Apache Tomcat 9 |
| 12 | **Oracle Database** | `db-oracle-01` | 10.88.11.212 | 22 (SSH), 1521 (OCI) | `gvenzl/oracle-xe:21-slim` | Oracle 21c XE (ZENTRADB) |

#### Windows Host Example (10.100.0.x bridge)

| # | Component | Container | IP Address | Port(s) | Description |
|---|-----------|-----------|------------|---------|-------------|
| 1 | **Linux Server 01** | `srv-linux-01` | 10.100.0.11 | 22, 161 | Rocky Linux 8 |
| 2 | **Linux Server 02** | `srv-linux-02` | 10.100.0.12 | 22, 161 | Ubuntu 22.04 |
| 3 | **Windows Server 01** | `srv-win-01` | 10.100.0.13 | 22, 161 | Windows Server 2022 |
| 4 | **Windows Server 02** | `srv-win-02` | 10.100.0.14 | 22, 161 | Windows Server 2019 |
| 5 | **Core Switch** | `sw-core-01` | 10.100.0.15 | 22, 161 | Cisco Catalyst 9300 |
| 6 | **Access Switch** | `sw-access-01` | 10.100.0.16 | 22, 161 | Arista 7050X |
| 7 | **Firewall** | `fw-01` | 10.100.0.17 | 22, 161 | Palo Alto PA-3260 |
| 8 | **Storage Array** | `stor-01` | 10.100.0.18 | 22, 161 | Dell PowerStore 500T |
| 9 | **Nginx** | `mw-nginx-01` | 10.100.0.19 | 22, 80, 8080 | Nginx Reverse Proxy |
| 10 | **Apache** | `mw-apache-01` | 10.100.0.20 | 22, 80, 8080 | Apache HTTP Server |
| 11 | **Tomcat** | `mw-tomcat-01` | 10.100.0.21 | 22, 8080, 9090 | Apache Tomcat 9 |
| 12 | **Oracle Database** | `db-oracle-01` | 10.100.0.22 | 22, 1521 | Oracle 21c XE |

### 6.3 Credentials

| Service | Username | Password | Notes |
|---------|----------|----------|-------|
| **All containers (SSH)** | `root` | `zentra123` | SSH on port 22 |
| **SNMP (all devices)** | — | Community: `public` | SNMP v2c, port 161, read-only |
| **Tomcat Manager** | `zentra` | `zentra123` | HTTP at `http://<ip>:8080/manager` |
| **Nginx Status** | — | — | HTTP at `http://<ip>:8080/nginx_status` |
| **Apache Status** | — | — | HTTP at `http://<ip>:8080/server-status` |
| **Oracle Database** | `SYS` | `zentra123` | SID: `XE`, Service: `ZENTRADB`, port 1521 |
| **Oracle Monitor User** | `zentra_mon` | `zentra123` | Read-only monitoring user |
| **Zentra Web UI** | `admin@zentra.local` | `changeme` | Port 5010 |

### 6.4 Network Topology

```
                    ┌─────────────┐
                    │  fw-01      │   Palo Alto Firewall
                    │  :22, :161  │
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │                         │
       ┌──────┴──────┐          ┌───────┴──────┐
       │ sw-core-01  │          │ sw-access-01 │   Network Switches
       │ :22, :161   │          │ :22, :161    │
       └──────┬──────┘          └───────┬──────┘
              │                         │
    ┌─────────┼──────────┐    ┌─────────┼──────────┐
    │         │          │    │         │          │
┌───┴───┐ ┌──┴───┐ ┌────┴┐ ┌┴────┐ ┌──┴───┐ ┌────┴──┐
│srv-   │ │srv-  │ │stor-│ │srv- │ │srv-  │ │db-    │
│linux  │ │linux │ │01   │ │win  │ │win   │ │oracle │
│-01    │ │-02   │ │     │ │-01  │ │-02   │ │-01    │
└───────┘ └──────┘ └─────┘ └─────┘ └──────┘ └───────┘

              ┌─────────────────────────────┐
              │      Middleware Tier         │
              │  mw-nginx  mw-apache  mw-   │
              │  -01       -01        tomcat │
              │  :80,:8080 :80,:8080  -01    │
              │                      :8080  │
              └─────────────────────────────┘
```

### 6.5 Starting the Lab

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

### 6.6 Adding Lab Targets to Zentra

| Target Type | Where in Zentra | What to Enter |
|-------------|----------------|---------------|
| Linux/Windows Servers | Infrastructure → Servers → Add Target | IP, port 22, root/zentra123 |
| Network Switches | Infrastructure → Network → Add Device | IP, SNMP community `public` |
| Firewall | Infrastructure → Network → Add Device | IP, SNMP community `public` |
| Storage Array | Infrastructure → Storage → Add Array | IP, SSH root/zentra123 |
| Nginx | Middleware → Nginx | IP, status URL `http://<ip>:8080/nginx_status` |
| Apache | Middleware → Apache | IP, status URL `http://<ip>:8080/server-status` |
| Tomcat | Middleware → Tomcat | IP:8080, manager `zentra/zentra123` |
| Oracle | Database → Oracle | IP:1521, SID `XE`, `zentra_mon/zentra123` |

---

## 7. Docker Deployment

### Quick Start (Zero Config)

```bash
docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 \
  --security-opt=no-new-privileges:true \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

Access at `http://localhost:5010`  
Default login: `admin@zentra.local` / `changeme`

### Production Deployment (Hardened)

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

### Air-Gapped Deployment

```bash
# On a machine with no internet
docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 --name zentra \
  --security-opt=no-new-privileges:true \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

### Building the Docker Image

```bash
# On the build server (10.88.11.162)
cd /store/Zentra

# 1. Build frontend
cd frontend && npm ci && npx vite build && cd ..

# 2. Build backend (release)
cargo build --release --bin zentra-server

# 3. Build Docker image
docker build -f docker/Dockerfile -t zentra:latest .

# 4. Export image as tarball
docker save zentra:latest | gzip > docker/zentra-latest.tar.gz
```

### Container Services (Supervisord)

| Service | User | Port | Auto-restart |
|---------|------|------|-------------|
| `postgresql` | `postgres` | 5434 | Yes |
| `zentra-server` | `zentra` | 5006 | Yes |
| `nginx` | `root` | 5010 | Yes |

---

## 8. Docker Image Hardening

The Docker image implements **D01–D11 hardening prompts** for production security.

### Self-Lockdown (D09/D11)

After the entrypoint completes initialization, the container locks itself down:

| Measure | Details |
|---------|---------|
| **Immutable files** | Config, license, binary, migrations, and frontend files locked with `chattr +i`. Even root cannot modify. |
| **No shells** | `bash`, `sh`, `csh`, `zsh` removed 5 seconds after startup. `docker exec` becomes useless. |
| **No package managers** | `dnf`, `yum`, `rpm` removed. No software can be installed at runtime. |
| **No editors** | `vi`, `vim`, `nano`, `ed`, `perl`, `lua`, `ruby` all removed. |
| **No dangerous tools** | `wget`, `ftp`, `telnet`, `dd`, `strings`, `gcc`, `make`, `strace`, `gdb` all removed. |
| **chattr removed** | After making files immutable, `chattr` itself is deleted — immutable bits can never be reversed. |
| **SSH preserved** | Only `sshpass`, `ssh`, `scp` kept (required for monitoring target servers). |
| **Tamper protection** | SHA-256 integrity manifest verifies critical files on every startup. |

### Debugging Mode

Set `ZENTRA_SKIP_LOCKDOWN=1` to disable self-lockdown (development only, **never** use in production).

---

## 9. License & Trial Period

### How It Works

1. **First boot**: `docker-entrypoint.sh` generates a signed license token:
   - Format: `install_epoch:expiry_epoch:hostname:sha256_hmac`
   - HMAC key: SHA-256 hash of the `zentra-server` binary
   - Stored at `/var/lib/postgresql/data/.zentra-license` (PG volume)
   - Fallback: `/opt/zentra/.license`

2. **Subsequent boots**: Entrypoint verifies the HMAC signature and checks expiry.

3. **Trial duration**: **3 days** (configurable via `LICENSE_DAYS` in entrypoint).

### Graceful Degradation After Expiry

After the trial expires, the container **still starts** but in degraded mode:

- Entrypoint logs a WARNING and sets `ZENTRA_TRIAL_EXPIRED=1`
- Backend middleware returns **402 Payment Required** for non-essential API routes
- Frontend hides all navigation except the **Command Center (Dashboard)**
- An amber warning banner is displayed at the top of the page

### Allowed Routes in Degraded Mode

| Route | Purpose |
|-------|---------|
| `/api/health` | Health check |
| `/api/auth/*` | Login, logout, token refresh |
| `/api/system/license-status` | License status (includes `trial_expired` flag) |
| `/api/config/mode` | App mode check |
| `/api/system/status` | System status |
| `/api/system/db` | Database health |
| `/metrics` | Prometheus metrics |

### Binary-Level Verification (Hardened)

The Rust binary (`license_verifier.rs`) **independently** reads the license file from disk:

- Computes its own SHA-256 hash
- Verifies the HMAC signature
- Checks the expiry epoch
- Does **NOT** trust the `ZENTRA_TRIAL_EXPIRED` environment variable in Docker
- If the license file is missing but the DB directory exists → treated as **bypass attempt** → expired
- Result is cached in `OnceLock` (one disk read per process lifetime)

### Renewing the License

1. **Rebuild and redeploy** — Build a fresh image; license resets on first boot with a new volume.
2. **Reset the volume** — `docker volume rm zentra-data` and re-run (data will be lost).
3. **Contact Zentra team** — For extended or permanent licenses.

---

## 10. Authentication & RBAC

### Authentication Flow

1. User submits email/password to `POST /api/auth/login`
2. Server validates credentials, returns JWT token + refresh token
3. Frontend stores tokens in `localStorage` (`Zentra_token`, `Zentra_refresh_token`)
4. All API requests include `Authorization: Bearer <token>` header
5. Token auto-refreshes 5 minutes before expiry (proactive refresh)
6. MFA (TOTP) support via `POST /api/auth/mfa/setup` and `POST /api/auth/mfa/verify`

### Two-Layer RBAC

**Layer 1 — Module Access** (which top-level sidebar modules a user sees):

| Module ID | Sidebar Label |
|-----------|--------------|
| `command_center` | Command Center |
| `infrastructure` | Infrastructure |
| `database` | Database |
| `middleware` | Middleware |
| `security` | Security |
| `logs` | Logs |
| `alerts_events` | Alerts & Events |
| `ai_analytics` | AI & Analytics |
| `opentelemetry` | OpenTelemetry |
| `admin` | Admin |

**Layer 2 — Sub-Module Menu Items** (dotted convention, e.g., `infrastructure.servers`):
- If a user has a module but NO sub-items → sees ALL children
- If sub-items exist → only those are shown

### Default Users

| User | Role | Modules |
|------|------|---------|
| `admin@zentra.local` | Admin | All (isAdmin=true) |

### Key Auth Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/auth/login` | POST | Login (email + password) |
| `/api/auth/logout` | POST | Invalidate session |
| `/api/auth/refresh` | POST | Refresh JWT token |
| `/api/auth/me` | GET | Get current user profile + module_access + menu_items |
| `/api/auth/mfa/setup` | POST | Generate MFA QR code |
| `/api/auth/mfa/verify` | POST | Verify MFA TOTP code |
| `/api/auth/sessions` | GET | List active sessions |
| `/api/auth/sessions/:id` | DELETE | Revoke a session |
| `/api/auth/password` | POST | Change password |

---

## 11. API Reference

### Health & System

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/health` | GET | Health check (returns `{"status":"healthy"}`) |
| `/api/system/status` | GET | System status (internal only) |
| `/api/system/db` | GET | Database connectivity check |
| `/api/system/license-status` | GET | License status with `trial_expired` flag |
| `/api/system/pool-status` | GET | Connection pool statistics |
| `/api/config/mode` | GET/POST | Demo/Live mode toggle |

### Targets & Infrastructure

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/targets` | GET/POST | List/create monitoring targets |
| `/api/targets/:id` | GET/PUT/DELETE | Target CRUD |
| `/api/targets/:id/verify-ssh` | POST | Test SSH connectivity |
| `/api/scrape-targets` | GET/POST | Prometheus scrape targets |
| `/api/collectors` | GET/POST | Collector management |
| `/api/service-discovery` | GET | Auto-discovered services |
| `/api/healthchecks` | GET/POST | HTTP health checks |
| `/api/maintenance-windows` | GET/POST | Maintenance window scheduling |

### Alerts & Notifications

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/alerts` | GET | List active alerts |
| `/api/alerts/:id/acknowledge` | POST | Acknowledge an alert |
| `/api/alerts/:id/resolve` | POST | Resolve an alert |
| `/api/alert-rules` | GET/POST | Alert rule CRUD |
| `/api/notification-channels` | GET/POST | Notification channel management |
| `/api/alert-templates` | GET/POST | Alert template management |
| `/api/silences` | GET/POST | Alert silencing rules |

### OpenTelemetry Ingest

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/traces` | POST | OTLP trace ingest (JSON or Protobuf) |
| `/v1/metrics` | POST | OTLP metrics ingest |
| `/v1/logs` | POST | OTLP logs ingest |
| `/v1/profiles` | POST | OTLP profiles ingest |
| `:4317` | gRPC | gRPC OTLP ingest (all signals) |

### AI & Analytics

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/ai/chat` | POST | AI-assisted chat |
| `/api/ai/insights` | GET | AI-generated insights |
| `/api/ai/predictive-alerts` | GET | Predictive alert analysis |
| `/api/ai/rca` | POST | Root cause analysis |
| `/api/ai/global-search` | GET | Cross-module search |

### Admin

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/admin/users` | GET/POST | User management |
| `/api/admin/users/:id` | GET/PUT/DELETE | User CRUD |
| `/api/admin/groups` | GET/POST | Group management |
| `/api/admin/roles` | GET/POST | Role management |

---

## 12. Frontend Modules

### Navigation Structure

| Module | Route | Sub-pages |
|--------|-------|-----------|
| **Command Center** | `/dashboard` | Overview dashboard with fleet cards |
| **Infrastructure** | `/infrastructure` | Dashboard, Servers (`/servers`), Storage (`/storage-arrays`), Network (`/network`), eBPF (`/cilium`) |
| **Database** | `/databases` | Dashboard, Oracle Monitoring (`/oracle`), Metric Editor (`/oracle/metrics`) |
| **Middleware** | `/middleware` | Dashboard + dynamic sub-pages per detected service (Nginx, Apache, Tomcat, HAProxy, Elasticsearch, Redis, Memcached, RabbitMQ, Kafka, Docker, Kubernetes, PHP-FPM) |
| **Security** | `/security/dashboard` | Dashboard, Threat Hunting (`/security/hunt`), Firewall (`/security/firewall`) |
| **Logs** | `/log-explorer` | Explorer, Sources (`/log-sources`), Alerts (`/log-alerts`), Analytics (`/log-analytics`) |
| **Alerts & Events** | `/alerts` | Active Alerts, Rules (`/alerts/rules`), Channels (`/notifications`) |
| **AI & Analytics** | `/ai-insights` | Insights, Predictive (`/predictive-alerts`), RCA (`/rca`), RunBooks (`/runbooks`), LLM Observability (`/llm-observability`), Profiling (`/profiling`) |
| **OpenTelemetry** | `/otel-dashboard` | Telemetry Health, Traces (`/traces`), Resources (`/otel-resources`), Collector Setup (`/otel-config`), SDK Guide (`/otel-sdk-guide`), Connectors (`/otel-connectors`), Pipeline Topology (`/otel-pipeline`), Span Metrics RED (`/otel-span-metrics`), Signal Alerts (`/otel-alert-rules`), OTLP Security (`/otel-security`) |
| **Admin** | `/admin/users` | Users, Groups (`/admin/groups`), Roles (`/admin/roles`), My Profile (`/admin/profile`) |

### Frontend Architecture

- **Auth Context** (`AuthContext.tsx`): Manages user state, JWT tokens, `hasModule()`, `hasMenuItem()`, `isAdmin`
- **Layout** (`Layout.tsx`): Sidebar navigation with RBAC filtering, license status check, trial expired banner
- **API Client** (`_client.ts`): Centralized `fetchJSON()` with auto-refresh on 401, proactive token renewal
- **React Query**: All data fetching uses TanStack React Query with stale times and refetch intervals
- **Theme System**: Multiple themes via `ThemeContext`, selectable from sidebar

---

## 13. OpenTelemetry Configuration

### Overview

Zentra has full OpenTelemetry support, both as a **producer** (self-monitoring) and as a **consumer** (ingest from external apps).

### Self-Monitoring (Producer)

Zentra instruments itself using the OpenTelemetry SDK:

```
zentra-api main.rs
  → init_telemetry("zentra-api")  (zentra-core/src/telemetry.rs)
  → Creates OpenTelemetry TracerProvider with:
      - service.name = "zentra-api"
      - service.version = from Cargo.toml
      - service.namespace = "zentra"
      - service.instance.id = UUID v7
      - deployment.environment from env
      - ParentBased(TraceIdRatioBased) sampler
  → Exports spans via OTLP gRPC to OTEL_EXPORTER_OTLP_ENDPOINT
  → Bridges tracing events to OTLP LogRecords via opentelemetry-appender-tracing
```

**Key env vars for self-monitoring:**

```env
OTEL_EXPORTER_OTLP_ENDPOINT=http://collector:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
ZENTRA_OTEL_SELF_ENABLED=true
ZENTRA_OTEL_SAMPLE_RATIO=0.1
```

### OTLP Ingest (Consumer)

Zentra can receive telemetry from any OTLP-compatible application:

**HTTP Ingest Endpoints:**

| Endpoint | Content Types | Description |
|----------|--------------|-------------|
| `POST /v1/traces` | `application/json`, `application/x-protobuf` | Trace spans |
| `POST /v1/metrics` | `application/json`, `application/x-protobuf` | Metric data points |
| `POST /v1/logs` | `application/json`, `application/x-protobuf` | Log records |
| `POST /v1/profiles` | `application/json`, `application/x-protobuf` | Profiling data |

**gRPC Ingest:**

| Port | Protocol | Signals |
|------|----------|---------|
| 4317 | gRPC (HTTP/2) | Traces, Metrics, Logs, Profiles |

**Authentication:** All ingest endpoints require an OTLP API key in the `x-otlp-api-key` header:

```bash
curl -X POST http://zentra:5010/v1/traces \
  -H "Content-Type: application/json" \
  -H "x-otlp-api-key: YOUR_API_KEY" \
  -d @traces.json
```

### Async Ingest Pipeline

```
Client → HTTP/gRPC handler → mpsc channel (10k buffer) → Background worker
                                                              │
                                                    ┌────────┴────────┐
                                                    │  Batch INSERT   │
                                                    │  via UNNEST     │
                                                    │  (1024 per batch)│
                                                    └────────┬────────┘
                                                              │
                                                    ┌────────┴────────┐
                                                    │  PostgreSQL +   │
                                                    │  TimescaleDB    │
                                                    │  (hypertables)  │
                                                    └────────┬────────┘
                                                              │
                                                    On failure (3 retries):
                                                    ┌────────┴────────┐
                                                    │  Dead Letter     │
                                                    │  Queue (DLQ)     │
                                                    │  otlp_dead_letter│
                                                    └─────────────────┘
```

### Quota Enforcement

OTLP ingest supports per-API-key quotas:

| Setting | Description |
|---------|-------------|
| `rate_limit_rpm` | Max requests per minute per API key |
| `max_payload_bytes` | Max request body size per API key |

When exceeded, returns `429 Too Many Requests` or `413 Payload Too Large`.

### OTel Collector Configuration

For deployments with an external OTel Collector, use `docker/otel-collector-config.yaml`:

```yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: "0.0.0.0:4317"
      http:
        endpoint: "0.0.0.0:4318"

processors:
  memory_limiter:
    check_interval: 5s
    limit_mib: 512
    spike_limit_mib: 128
  batch:
    send_batch_size: 1024
    timeout: 5s
  tail_sampling:
    decision_wait: 10s
    policies:
      - name: errors-policy
        type: status_code
        status_code: { status_codes: [ERROR] }
      - name: latency-policy
        type: latency
        latency: { threshold_ms: 5000 }
      - name: probabilistic-policy
        type: probabilistic
        probabilistic: { sampling_percentage: 10 }

exporters:
  otlphttp:
    endpoint: "http://zentra-api:5006"
    tls: { insecure: true }
  prometheus:
    endpoint: "0.0.0.0:8889"

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, tail_sampling, batch]
      exporters: [otlphttp]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp, prometheus]
    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp]
```

### Instrumenting Your Application

**Python (Flask/Django):**

```python
from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter

provider = TracerProvider()
exporter = OTLPSpanExporter(
    endpoint="http://zentra-host:4317",
    headers={"x-otlp-api-key": "YOUR_API_KEY"}
)
provider.add_span_processor(BatchSpanProcessor(exporter))
trace.set_tracer_provider(provider)
```

**Node.js:**

```javascript
const { OTLPTraceExporter } = require('@opentelemetry/exporter-trace-otlp-grpc');
const exporter = new OTLPTraceExporter({
  url: 'http://zentra-host:4317',
  headers: { 'x-otlp-api-key': 'YOUR_API_KEY' }
});
```

**Java (Spring Boot):**

```properties
# application.properties
otel.exporter.otlp.endpoint=http://zentra-host:4317
otel.exporter.otlp.headers=x-otlp-api-key=YOUR_API_KEY
otel.service.name=my-java-app
```

### Frontend RUM (Real User Monitoring)

Zentra's frontend includes built-in OpenTelemetry RUM (`frontend/src/lib/otel/init.ts`):

- WebTracerProvider with FetchInstrumentation
- DocumentLoad and UserInteraction auto-instrumentation
- W3C `traceparent` header propagation
- Configurable via `VITE_OTEL_ENDPOINT` env var

### OTLP Security (API Key Management)

Manage OTLP API keys via the UI at `/otel-security` or the API:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/otel/security/api-keys` | GET/POST | List/create API keys |
| `/api/otel/security/api-keys/:id` | DELETE | Revoke an API key |
| `/api/otel/security/audit` | GET | Audit log of API key usage |

### Background Jobs

| Job | Interval | Description |
|-----|----------|-------------|
| MV Refresh | 60s | Refreshes `span_red_metrics_1m` materialized view (advisory lock 48201) |
| Alert Eval | 5 min | Evaluates OTel alert rules against span metrics (lock 48202) |
| Cache Prune | 1 hour | Prunes stale OTLP API key cache entries (lock 48203) |

### TimescaleDB Storage

| Table | Hypertable | Retention | Compression |
|-------|-----------|-----------|-------------|
| `traces` | Yes (timestamp) | 90 days | Enabled |
| `otlp_metrics` | Yes | 90 days | Enabled |
| `otlp_logs` | Yes | 30 days | Enabled |
| `span_events` | No | — | — |
| `span_links` | No | — | — |
| `otlp_dead_letter` | No | — | — |

---

## 14. Database Schema

### Key Tables

| Table | Purpose |
|-------|---------|
| `users` | User accounts (PK: `user_id`) |
| `user_module_access` | RBAC module assignments |
| `targets` | Monitoring targets (servers, devices) |
| `alerts` | Active and historical alerts |
| `alert_rules` | Alert rule definitions |
| `notification_channels` | Email, Slack, PagerDuty, etc. |
| `traces` | OTLP trace spans (TimescaleDB hypertable) |
| `otlp_metrics` | OTLP metric data points |
| `otlp_logs` | OTLP log records |
| `otlp_api_keys` | OTLP ingest API keys |
| `otlp_dead_letter` | Failed ingest payloads |
| `system_settings` | Key-value system configuration |

### Migrations

SQL migrations are stored in `/opt/zentra/migrations/` inside the container and in `migrations/` in the source. They are applied automatically by `zentra-server` on startup.

Naming convention: `YYYYMMDD######_description.sql` (e.g., `20250101000070_traces_base_schema.sql`)

---

## 15. Monitoring Targets

### Adding a Target

1. Navigate to **Infrastructure → Servers**
2. Click **Add Target**
3. Fill in: Hostname/IP, SSH port, SSH credentials
4. Click **Verify SSH** to test connectivity
5. Save

### Target Types

| Type | Protocol | Metrics Collected |
|------|----------|------------------|
| Linux Server | SSH | CPU, memory, disk, network, processes |
| Windows Server | WMI/SSH | CPU, memory, disk, services |
| Network Device | SNMP v2c/v3 | Interfaces, traffic, CPU, memory |
| Oracle Database | OCI + SSH | Tablespaces, sessions, waits, ASM |
| PostgreSQL | SQL | Connections, queries, replication |
| Storage Array | SSH/API | Volumes, pools, disks, IOPS |

### SSH Key Setup

The container auto-generates SSH keys on first boot at `/root/.ssh/id_rsa`. To set up monitoring:

```bash
# Get the public key from the container
docker exec zentra cat /root/.ssh/id_rsa.pub

# Add it to the target server
ssh-copy-id -i <key> user@target-server
```

Or use `sshpass` with password-based auth (configured per target in the UI).

---

## 16. Alerting System

### Alert Flow

```
Metric Collection → Rule Evaluation (zentra-stream) → Alert Creation
       ↓                                                     ↓
  Notification Router                              Alert UI (/alerts)
       ↓
  Channels (Email, Slack, PagerDuty, Webhook, Teams)
```

### Alert States

| State | Description |
|-------|-------------|
| `firing` | Alert condition is currently true |
| `acknowledged` | Operator has seen the alert |
| `resolved` | Alert condition cleared (auto or manual) |
| `silenced` | Matched by a silence rule |

### Creating Alert Rules

1. Go to **Alerts & Events → Rules**
2. Click **Create Rule**
3. Configure: metric, condition, threshold, duration, severity
4. Assign notification channels
5. Save

---

## 17. AI & Analytics

| Feature | Route | Description |
|---------|-------|-------------|
| **AI Insights** | `/ai-insights` | AI-generated operational insights |
| **Predictive Alerts** | `/predictive-alerts` | Forecast capacity issues before they occur |
| **Root Cause Analysis** | `/rca` | Automated RCA for incidents |
| **RunBooks** | `/runbooks` | Automated remediation playbooks |
| **LLM Observability** | `/llm-observability` | Monitor LLM/GenAI workloads (OTel gen_ai.* semconv) |
| **Profiling** | `/profiling` | Application profiling with flamegraph visualization |

---

## 18. Security Module

| Feature | Route | Description |
|---------|-------|-------------|
| **Dashboard** | `/security/dashboard` | Security posture overview |
| **Threat Hunting** | `/security/hunt` | Interactive threat investigation |
| **Firewall** | `/security/firewall` | Firewall log analysis and rules |
| **IDS/IPS** | via API | Intrusion detection/prevention rules and alerts |
| **File Analysis** | via API | File integrity monitoring |

---

## 19. Network Monitoring

| Feature | Route | Description |
|---------|-------|-------------|
| **Network Dashboard** | `/network` | Device status, topology map |
| **Deep Analysis** | via API | Packet-level analysis |
| **Device Discovery** | via API | Auto-discover network devices via SNMP |
| **NetFlow** | via API | Flow analysis (NetFlow/sFlow/IPFIX) |

### SNMP Configuration

Target devices must have SNMP enabled. Zentra supports:
- SNMP v2c (community string)
- SNMP v3 (user/auth/priv)

---

## 20. Middleware Monitoring

Middleware services are auto-discovered and dynamically added to the sidebar:

| Service | Metrics |
|---------|---------|
| **Nginx** | Active connections, requests/sec, status codes |
| **Apache** | Workers, requests, scoreboard |
| **Tomcat** | Threads, sessions, JVM memory |
| **HAProxy** | Backends, health, queue depth |
| **Redis** | Memory, keyspace, commands/sec |
| **Elasticsearch** | Indices, shards, cluster health |
| **Kafka** | Topics, partitions, consumer lag |
| **Docker** | Containers, CPU, memory per container |
| **Kubernetes** | Pods, deployments, resource usage |

---

## 21. Connection Pooling

Zentra manages multiple connection pools:

| Pool | Type | Configuration |
|------|------|--------------|
| **Main PG** | SQLx PgPool | 200 max connections |
| **TimescaleDB** | SQLx PgPool | Shared with main |
| **Target DB Cache** | LRU DashMap | Max 64 targets, 5-min TTL |
| **Oracle OCI** | Native pool registry | Per-target pools |
| **SSH Mux** | ControlMaster | ControlPersist=600s, ServerAlive=30s |
| **HTTP Shared** | reqwest::Client | pool_idle_per_host=32, idle_timeout=90s |

Monitor pool health at `GET /api/system/pool-status`.

---

## 22. Backup & Recovery

### Database Backup

```bash
# From inside the container (before shell lockdown)
docker exec zentra su - postgres -c "pg_dump -p 5434 zentra | gzip > /var/lib/postgresql/data/zentra-backup.sql.gz"

# Copy backup out
docker cp zentra:/var/lib/postgresql/data/zentra-backup.sql.gz ./zentra-backup.sql.gz
```

### Volume Backup

```bash
# Stop container
docker stop zentra

# Backup the volume
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \
  tar czf /backup/zentra-data-backup.tar.gz -C /data .

# Restart
docker start zentra
```

### Recovery

```bash
# Remove old volume
docker volume rm zentra-data

# Restore from backup
docker volume create zentra-data
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \
  tar xzf /backup/zentra-data-backup.tar.gz -C /data

# Start container
docker run -d -p 5010:5010 -v zentra-data:/var/lib/postgresql/data zentra:latest
```

---

## 23. Troubleshooting

### Common Issues

| Problem | Cause | Solution |
|---------|-------|---------|
| "License expired" warning | Trial period exceeded | Rebuild image or reset volume |
| "Integrity verification failed" | Image files modified | Use official image tarball |
| Container won't start | DB corruption or port conflict | Check `docker logs zentra` |
| API returns 402 | Trial expired (degraded mode) | Only Dashboard is available; get a license |
| "docker exec" fails | Shells removed by lockdown (D11) | By design — use `ZENTRA_SKIP_LOCKDOWN=1` for debug |
| Cannot install packages | Package managers removed | By design — rebuild image instead |
| SSH monitoring fails | SSH keys not configured | Copy container's public key to target |
| OTLP ingest returns 401 | Invalid or missing API key | Generate key at `/otel-security` |
| OTLP ingest returns 429 | Rate limit exceeded | Increase `rate_limit_rpm` for the API key |
| Slow queries | Missing indexes or large data | Check TimescaleDB compression/retention policies |

### Viewing Logs

```bash
# Container logs (stdout)
docker logs zentra

# If lockdown hasn't fired yet (within 5s of startup):
docker exec zentra cat /opt/zentra/logs/api.log
docker exec zentra cat /opt/zentra/logs/nginx_access.log
docker exec zentra cat /opt/zentra/logs/nginx_error.log
docker exec zentra supervisorctl status
```

### Health Checks

```bash
# API health
curl -sf http://localhost:5010/api/health

# Database health
curl -sf http://localhost:5010/api/system/db

# All services status (before lockdown)
docker exec zentra supervisorctl status
```

---

*End of Runbook — Zentra Team — Internal Use Only*
