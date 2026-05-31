# Zentra — Installation & Configuration Guide

Zentra is an all-in-one infrastructure monitoring platform packaged as a single Docker image.

**What's inside the image:**
- Rocky Linux 8 (base OS)
- PostgreSQL 16 + TimescaleDB (database)
- Zentra API Server (compiled Rust binary)
- Zentra Frontend (built React app)
- Nginx (web server + reverse proxy)
- Supervisord (process manager)

**What's NOT in the image:** No source code, no credentials, no pre-existing data.

All configuration is passed at runtime via environment variables.

---

## Table of Contents

- [1. Prerequisites](#1-prerequisites)
- [2. Install Docker](#2-install-docker)
- [3. Load or Pull the Zentra Image](#3-load-or-pull-the-zentra-image)
- [4. Configure Persistent Storage](#4-configure-persistent-storage)
- [5. Configure Environment Variables](#5-configure-environment-variables)
- [6. Run the Container](#6-run-the-container)
- [7. Verify the Installation](#7-verify-the-installation)
- [8. Access the Application](#8-access-the-application)
- [9. Stop, Start & Restart](#9-stop-start--restart)
- [10. Backup & Restore Data](#10-backup--restore-data)
- [11. Update to a New Version](#11-update-to-a-new-version)
- [12. Troubleshooting](#12-troubleshooting)
- [13. Full Deployment (App + Simulation)](#13-full-deployment-app--simulation)
- [14. Datacenter Simulation](#14-datacenter-simulation)
- [15. SSH Key Setup](#15-ssh-key-setup)
- [16. Traffic Generator](#16-traffic-generator)
- [17. Building the Docker Image](#17-building-the-docker-image)
- [18. Security Considerations](#18-security-considerations)

---

## Quick Install (Automated)

For a fully automated setup, use the provided install scripts. Place the script in the same directory as `zentra-latest.tar.gz` and run:

**Linux:**
```bash
chmod +x install.sh
./install.sh
```

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -File install.ps1
```

The script will:
1. Check that Docker is installed and running
2. Load the image from `zentra-latest.tar.gz` (or pull from GHCR)
3. Generate a secure `.env` file with random JWT secret and DB password
4. Create the container with persistent storage and auto-restart
5. Wait for all services to pass health checks
6. Print the URL and login credentials

> For manual installation or customization, follow the steps below.

---

## 1. Prerequisites

| Requirement | Minimum |
|-------------|---------|
| **OS** | Linux x86_64, Windows 10/11, or macOS |
| **Docker** | >= 20.10 (Docker Desktop on Windows/macOS) |
| **RAM** | 2 GB (4 GB recommended) |
| **Disk** | 1 GB for image + space for database growth |
| **Ports** | 5010 (web UI) must be available |

---

## 2. Install Docker

### Linux

```bash
# RHEL / Rocky / CentOS
sudo dnf install -y docker
sudo systemctl enable --now docker

# Ubuntu / Debian
sudo apt-get update && sudo apt-get install -y docker.io
sudo systemctl enable --now docker

# Verify
docker --version
```

### Windows

1. Download **Docker Desktop** from https://www.docker.com/products/docker-desktop
2. Run the installer and follow the prompts
3. Ensure **WSL 2** backend is enabled (recommended) — Docker Desktop will prompt you to install it if needed
4. After installation, open a **PowerShell** or **Command Prompt** and verify:

```powershell
docker --version
```

> **Note:** All `docker` commands in this guide work the same on Windows (PowerShell/CMD) and Linux. For multi-line commands, replace `\` (line continuation) with `` ` `` in PowerShell.

---

## 3. Load or Pull the Zentra Image

### Option A: Load from Tarball (offline / air-gapped)

If you received a `zentra-latest.tar.gz` file:

**Linux:**
```bash
docker load -i zentra-latest.tar.gz
```

**Windows (PowerShell):**
```powershell
docker load -i zentra-latest.tar.gz
```

Verify it loaded:
```bash
docker images zentra
# Should show: zentra   latest   <image-id>   <size>
```

### Option B: Pull from GitHub Container Registry (GHCR)

```bash
# If the package is private, authenticate first:
echo "YOUR_GITHUB_PAT" | docker login ghcr.io -u YOUR_GITHUB_USERNAME --password-stdin

# Pull the image
docker pull ghcr.io/rameshsraj/zentra:latest
```

To generate a GitHub PAT: https://github.com/settings/tokens/new?scopes=read:packages

---

## 4. Configure Persistent Storage

**This is critical.** Without persistent storage, all your data (users, targets, metrics, dashboards, alerts) will be lost when the container stops or is removed.

### Option A: Docker Named Volume (recommended)

Docker manages the volume automatically. Data survives container stops, restarts, and removals.

```bash
# Create the volume (Docker creates it automatically on first run, but you can pre-create it)
docker volume create zentra-data

# Use it when running the container
docker run -d \
  --name zentra \
  -v zentra-data:/var/lib/postgresql/data \
  ...
```

**Where is the data stored on disk?**

```bash
# Find the volume's mount point
docker volume inspect zentra-data --format '{{ .Mountpoint }}'
# Typically: /var/lib/docker/volumes/zentra-data/_data
```

### Option B: Host Directory Bind Mount

Maps a specific directory on your host machine to the database inside the container. Useful when you want full control over the storage location.

**Linux:**
```bash
sudo mkdir -p /opt/zentra/pgdata
sudo chown -R 26:26 /opt/zentra/pgdata   # UID 26 = postgres user in the container

docker run -d \
  --name zentra \
  -v /opt/zentra/pgdata:/var/lib/postgresql/data \
  ...
```

**Windows (PowerShell):**
```powershell
mkdir C:\zentra\pgdata

docker run -d `
  --name zentra `
  -v C:\zentra\pgdata:/var/lib/postgresql/data `
  ...
```

### Option C: Persist Application Logs Too

By default, application logs are inside the container and lost on removal. Mount a host directory to keep them:

**Linux:**
```bash
sudo mkdir -p /opt/zentra/logs

docker run -d \
  --name zentra \
  -v zentra-data:/var/lib/postgresql/data \
  -v /opt/zentra/logs:/opt/zentra/logs \
  ...
```

**Windows (PowerShell):**
```powershell
mkdir C:\zentra\logs

docker run -d `
  --name zentra `
  -v zentra-data:/var/lib/postgresql/data `
  -v C:\zentra\logs:/opt/zentra/logs `
  ...
```

### What Gets Persisted

| Data | Location in Container | Persisted with `-v` |
|------|----------------------|---------------------|
| Database (users, targets, metrics, dashboards, alerts, audit logs) | `/var/lib/postgresql/data` | **Yes** — this is the volume |
| API logs | `/opt/zentra/logs/api.log` | Only if you mount `/opt/zentra/logs` |
| Nginx logs | `/opt/zentra/logs/nginx_*.log` | Only if you mount `/opt/zentra/logs` |

### Important Notes

- **`docker stop zentra`** — Data is safe. Volume is preserved.
- **`docker rm zentra`** — Data is safe. Named volumes are NOT deleted with the container.
- **`docker volume rm zentra-data`** — **DESTRUCTIVE!** This deletes all data permanently.
- **`docker system prune --volumes`** — **DANGEROUS!** This deletes all unused volumes, including `zentra-data` if the container isn't running.

---

## 5. Configure Environment Variables

### Quick: Inline Variables

Pass variables directly on the command line:

```bash
docker run -d \
  --name zentra \
  -e ZENTRA_JWT_SECRET="my-secret-key-must-be-at-least-32-characters-long" \
  -e POSTGRES_PASSWORD="<your-db-password>" \
  ...
```

### Recommended: Environment File

Create a `.env` file with the following contents:

```env
# === PostgreSQL ===
POSTGRES_USER=postgres
POSTGRES_PASSWORD=<your-db-password>
POSTGRES_DB=zentra
POSTGRES_PORT=5434

# === Zentra API ===
ZENTRA_JWT_SECRET=change-this-to-a-random-string-of-at-least-32-characters
ZENTRA_API_PORT=5006
ZENTRA_ALLOW_ORIGINS=*
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001

# === Logging ===
RUST_LOG=info
```

Save this file as:
- **Linux:** `/opt/zentra/.env` (then run `chmod 600 /opt/zentra/.env`)
- **Windows:** `C:\zentra\.env`

Then reference it when running:

**Linux:**
```bash
docker run -d \
  --name zentra \
  --env-file /opt/zentra/.env \
  ...
```

**Windows (PowerShell):**
```powershell
docker run -d `
  --name zentra `
  --env-file C:\zentra\.env `
  ...
```

### All Configuration Variables

| Variable | Default | Required | Description |
|----------|---------|----------|-------------|
| `ZENTRA_JWT_SECRET` | — | **Yes** | JWT signing key. Must be >= 32 characters |
| `POSTGRES_PASSWORD` | — | No | Database password |
| `POSTGRES_USER` | `postgres` | No | Database username |
| `POSTGRES_DB` | `zentra` | No | Database name |
| `POSTGRES_PORT` | `5434` | No | PostgreSQL port inside the container |
| `ZENTRA_API_PORT` | `5006` | No | API port inside the container |
| `ZENTRA_ALLOW_ORIGINS` | `*` | No | CORS origins (comma-separated) |
| `ZENTRA_ORG_ID` | `00000000-...0001` | No | Default organization UUID |
| `RUST_LOG` | `info` | No | Log level: `trace`, `debug`, `info`, `warn`, `error` |

### Generate a Secure JWT Secret

```bash
openssl rand -hex 32
# Example output: a4f8c2e91b3d7f6a0e5c8b2d4f9a1c3e7b5d0f2a8e6c4b9d1f3a5c7e9b0d2f
```

---

## 6. Run the Container

### Full Production Command

**Linux:**
```bash
docker run -d \
  --name zentra \
  --restart unless-stopped \
  --env-file /opt/zentra/.env \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  -v /opt/zentra/logs:/opt/zentra/logs \
  zentra:latest
```

**Windows (PowerShell):**
```powershell
docker run -d `
  --name zentra `
  --restart unless-stopped `
  --env-file C:\zentra\.env `
  -p 5010:5010 `
  -v zentra-data:/var/lib/postgresql/data `
  -v C:\zentra\logs:/opt/zentra/logs `
  zentra:latest
```

**Explanation of each flag:**

| Flag | Purpose |
|------|---------|
| `-d` | Run in background (detached) |
| `--name zentra` | Name the container `zentra` |
| `--restart unless-stopped` | Auto-restart on crash or server reboot |
| `--env-file /opt/zentra/.env` | Load configuration from file |
| `-p 5010:5010` | Expose the web UI on port 5010 |
| `-v zentra-data:/var/lib/postgresql/data` | **Persistent database storage** |
| `-v /opt/zentra/logs:/opt/zentra/logs` | Persistent application logs |

### Minimal Quick-Start Command

**Linux:**
```bash
docker run -d \
  --name zentra \
  -e ZENTRA_JWT_SECRET="dev-only-secret-key-32-chars-minimum" \
  -e POSTGRES_PASSWORD="changeme" \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

**Windows (PowerShell):**
```powershell
docker run -d `
  --name zentra `
  -e ZENTRA_JWT_SECRET="dev-only-secret-key-32-chars-minimum" `
  -e POSTGRES_PASSWORD="changeme" `
  -p 5010:5010 `
  -v zentra-data:/var/lib/postgresql/data `
  zentra:latest
```

---

## 7. Verify the Installation

### Check container is running

```bash
docker ps | grep zentra
# Should show: STATUS = Up X seconds, PORTS = 0.0.0.0:5010->5010/tcp
```

### Check startup logs

```bash
docker logs zentra
```

Expected output (first run):
```
==============================================
  Zentra — Starting up
  PostgreSQL: postgres@localhost:5434/zentra
  API Port:   5006
==============================================
[entrypoint] Initializing PostgreSQL 16 data directory...
[entrypoint] Creating database 'zentra'...
[entrypoint] Enabling TimescaleDB extension...
[entrypoint] PostgreSQL initialization complete.
[entrypoint] Starting all services via supervisord...
2026-XX-XX INFO spawned: 'postgresql' with pid ...
2026-XX-XX INFO spawned: 'zentra-server' with pid ...
2026-XX-XX INFO spawned: 'nginx' with pid ...
2026-XX-XX INFO success: postgresql entered RUNNING state
2026-XX-XX INFO success: zentra-server entered RUNNING state
2026-XX-XX INFO success: nginx entered RUNNING state
```

### Check health endpoint

```bash
curl -s http://localhost:5010/api/health
```

Expected:
```json
{"database":true,"status":"healthy","subscribers":0,"version":"1.0.0"}
```

### Check Docker health status

```bash
docker inspect --format='{{.State.Health.Status}}' zentra
# Returns: healthy
```

### Check all internal services

```bash
docker exec zentra supervisorctl status
# nginx                            RUNNING   pid 254, uptime 0:01:23
# postgresql                       RUNNING   pid 252, uptime 0:01:23
# zentra-server                    RUNNING   pid 253, uptime 0:01:23
```

---

## 8. Access the Application

### Web UI

Open your browser and navigate to:

```
http://<server-ip>:5010
```

Replace `<server-ip>` with:
- `localhost` if running locally
- Your server's IP address (e.g., `192.168.1.100`)
- Your domain name if configured

### Login

| Field | Value |
|-------|-------|
| **Email** | `admin@Zentra.local` |
| **Password** | `changeme` |

> **Important:** Change the default password immediately after first login. Go to **Admin > My Profile** in the left sidebar.

### Application Sections

After login, the sidebar provides access to all modules:

| Section | What It Does |
|---------|-------------|
| **Command Center** | Overview dashboard with health scores, alert counts, target status |
| **Infrastructure** | Monitor servers (SSH), storage arrays, network devices (SNMP) |
| **Database** | Monitor Oracle databases |
| **Middleware** | Monitor Nginx, Apache, Tomcat |
| **Security** | Security dashboard, threat hunting, firewall monitoring |
| **Logs** | Log explorer, sources, alerts, analytics |
| **Alerts & Events** | Active alerts, alert rules, notification channels |
| **AI & Analytics** | AI insights, predictive alerts, root cause analysis, runbooks |
| **Admin** | User management, groups, roles (RBAC), profile settings |

### Adding Your First Monitored Server

1. Go to **Infrastructure > Servers**
2. Click **Add Server**
3. Enter the server's IP, SSH username, and credentials
4. Zentra will auto-discover all running services and offer to register them as targets
5. Metrics collection begins automatically every 5 minutes

---

## 9. Stop, Start & Restart

```bash
# Stop (data is preserved)
docker stop zentra

# Start again (data is intact)
docker start zentra

# Restart
docker restart zentra

# View logs after restart
docker logs -f zentra

# Remove container (data volume still safe)
docker stop zentra && docker rm zentra

# Re-create from same volume (all data restored)
docker run -d \
  --name zentra \
  --restart unless-stopped \
  --env-file /opt/zentra/.env \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

---

## 10. Backup & Restore Data

### Backup the database

```bash
# SQL dump (compressed)
docker exec zentra su - postgres -c "pg_dump -p 5434 -d zentra" \
  | gzip > zentra_backup_$(date +%Y%m%d_%H%M%S).sql.gz

# Custom format (faster restore)
docker exec zentra su - postgres -c "pg_dump -p 5434 -d zentra -Fc" \
  > zentra_backup_$(date +%Y%m%d_%H%M%S).dump
```

### Restore the database

```bash
# From SQL
gunzip -c zentra_backup_20260514.sql.gz | docker exec -i zentra su - postgres -c \
  "psql -p 5434 -d zentra"

# From custom format
cat zentra_backup_20260514.dump | docker exec -i zentra su - postgres -c \
  "pg_restore -p 5434 -d zentra --clean --if-exists"
```

### Backup the entire volume

```bash
docker run --rm \
  -v zentra-data:/data:ro \
  -v $(pwd):/backup \
  alpine tar czf /backup/zentra-volume-$(date +%Y%m%d).tar.gz -C /data .
```

### Automated daily backup (cron)

```bash
# Add to crontab
crontab -e

# Add this line (backup at 2 AM daily):
0 2 * * * docker exec zentra su - postgres -c "pg_dump -p 5434 -d zentra -Fc" > /opt/backups/zentra_$(date +\%Y\%m\%d).dump
```

---

## 11. Update to a New Version

Your data is safe in the volume. Updates are simple:

```bash
# 1. Pull the new image
docker pull ghcr.io/rameshsraj/zentra:latest

# 2. Stop and remove the old container
docker stop zentra && docker rm zentra

# 3. Start with the new image (same volume = same data)
docker run -d \
  --name zentra \
  --restart unless-stopped \
  --env-file /opt/zentra/.env \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  -v /opt/zentra/logs:/opt/zentra/logs \
  zentra:latest

# Database migrations run automatically on startup
```

---

## 12. Troubleshooting

### Container won't start

```bash
docker logs zentra                           # Check startup output
docker inspect zentra | grep -i status       # Check state
ss -tlnp | grep 5010                         # Check if port is already in use
```

### Reset admin password

Contact your system administrator or refer to internal documentation for password reset procedures.

### Restart individual services

```bash
docker exec zentra supervisorctl restart postgresql
docker exec zentra supervisorctl restart zentra-server
docker exec zentra supervisorctl restart nginx
```

### Check database size

```bash
docker exec zentra su - postgres -c \
  "psql -p 5434 -d zentra -c \"SELECT pg_size_pretty(pg_database_size('zentra'));\""
```

### Complete reset (fresh start)

```bash
docker stop zentra && docker rm zentra
docker volume rm zentra-data
# Run again — database will be re-initialized from scratch
```

### Shell access

```bash
docker exec -it zentra bash
```

---

## 13. Full Deployment (App + Simulation)

The `deploy-all` wrapper script installs the Zentra application **and** deploys the full 12-node datacenter simulation in one command. It also connects the Zentra container to the simulation network so Zentra can monitor all nodes.

**Windows (PowerShell):**
```powershell
powershell -ExecutionPolicy Bypass -File deploy-all.ps1
```

**Linux:**
```bash
chmod +x deploy-all.sh
sudo ./deploy-all.sh
```

**What the wrapper does:**

| Phase | Script | Description |
|-------|--------|-------------|
| 1 | `install.ps1` / `install.sh` | Loads image, creates `.env`, starts Zentra container |
| 2 | `simulation/setup.ps1` / `simulation/setup.sh` | Stops old containers, generates compose config, pulls images, starts 12 nodes |
| 3 | Network connect | Joins the Zentra container to the simulation Docker bridge network |
| 4 | Summary | Displays combined status of all services, IPs, ports, and credentials |

After deployment, the Zentra Web UI is accessible at `http://<host-ip>:5010`.

> **Note:** On Windows, Docker Desktop does not support `macvlan`. The simulation uses a dedicated Docker bridge network (`10.100.0.0/24`). These IPs are reachable **only from inside other Docker containers** on the same network — not from the Windows host. Use the IPs shown in the summary when adding targets in Zentra.

---

## 14. Datacenter Simulation

The `simulation/` folder contains a Docker Compose-based datacenter simulation with 12 nodes that represent a realistic IT environment for Zentra to monitor.

### Running Standalone

If you already have Zentra running and only want to deploy the simulation:

**Windows:**
```powershell
powershell -ExecutionPolicy Bypass -File simulation\setup.ps1
```

**Linux:**
```bash
cd simulation
chmod +x setup.sh
sudo ./setup.sh
```

### Simulated Nodes

All 12 containers run SSH (port 22) with credentials `root` / `zentra123`.

| Container | IP (Windows) | Ports | Base Image | Role |
|-----------|-------------|-------|-----------|------|
| `srv-linux-01` | 10.100.0.11 | 22, 161 | Rocky Linux 8 | Web Server |
| `srv-linux-02` | 10.100.0.12 | 22, 161 | Ubuntu 22.04 | App Server |
| `srv-win-01` | 10.100.0.13 | 22, 161 | Alpine (SNMPD) | Windows Server 2022 (AD/DNS) |
| `srv-win-02` | 10.100.0.14 | 22, 161 | Alpine (SNMPD) | Windows Server 2019 (Files) |
| `sw-core-01` | 10.100.0.15 | 22, 161 | Alpine (SNMPD) | Cisco Catalyst 9300 (Core Switch) |
| `sw-access-01` | 10.100.0.16 | 22, 161 | Alpine (SNMPD) | Arista 7050X (Access Switch) |
| `fw-01` | 10.100.0.17 | 22, 161 | Alpine (SNMPD) | Palo Alto PA-3260 (Firewall) |
| `stor-01` | 10.100.0.18 | 22, 161 | Alpine (SNMPD) | Dell PowerStore 500T (SAN) |
| `mw-nginx-01` | 10.100.0.19 | 22, 80, 8080 | Nginx Alpine | Nginx Reverse Proxy |
| `mw-apache-01` | 10.100.0.20 | 22, 80, 8080 | Apache Alpine | Apache HTTP Server |
| `mw-tomcat-01` | 10.100.0.21 | 22, 8080 | Tomcat 9 JDK17 | Apache Tomcat 9 |
| `db-oracle-01` | 10.100.0.22 | 22, 1521 | Oracle XE 21-slim | Oracle 21c XE Database |

> **Linux note:** On Linux with `macvlan`, IPs are assigned from the host subnet (e.g., `192.168.1.201+`), and the containers are directly reachable on the LAN.

### Credentials

| Service | Username | Password | Notes |
|---------|----------|----------|-------|
| SSH (all containers) | `root` | `zentra123` | Port 22 |
| SNMP | — | Community: `public` | Port 161, v2c, read-only |
| Tomcat Manager | `zentra` | `zentra123` | `http://<ip>:8080/manager` |
| Oracle SYS | `SYS` | `zentra123` | SID: `XE` |
| Oracle Monitor | `zentra_mon` | `zentra123` | Service: `ZENTRADB` |
| Nginx Status | — | — | `http://<ip>:8080/nginx_status` |
| Apache Status | — | — | `http://<ip>:8080/server-status` |

### Network Architecture

```
                          Docker Bridge: 10.100.0.0/24
                          ┌──────────────────────────────────────┐
                          │                                      │
  ┌─────────────┐         │  ┌──────────┐  ┌──────────┐        │
  │   Zentra    │◄──net──►│  │srv-lin-01│  │srv-lin-02│        │
  │ (App + DB)  │ connect │  │  .11     │  │  .12     │        │
  │ :5010       │         │  └──────────┘  └──────────┘        │
  └─────────────┘         │  ┌──────────┐  ┌──────────┐        │
                          │  │srv-win-01│  │srv-win-02│        │
                          │  │  .13     │  │  .14     │        │
                          │  └──────────┘  └──────────┘        │
                          │  ┌──────────┐  ┌──────────┐        │
                          │  │sw-core-01│  │sw-acc-01 │        │
                          │  │  .15     │  │  .16     │        │
                          │  └──────────┘  └──────────┘        │
                          │  ┌──────────┐  ┌──────────┐        │
                          │  │  fw-01   │  │ stor-01  │        │
                          │  │  .17     │  │  .18     │        │
                          │  └──────────┘  └──────────┘        │
                          │  ┌──────────┐  ┌──────────┐        │
                          │  │mw-nginx  │  │mw-apache │        │
                          │  │  .19     │  │  .20     │        │
                          │  └──────────┘  └──────────┘        │
                          │  ┌──────────┐  ┌──────────┐        │
                          │  │mw-tomcat │  │db-oracle │        │
                          │  │  .21     │  │  .22     │        │
                          │  └──────────┘  └──────────┘        │
                          └──────────────────────────────────────┘
```

### Stopping the Simulation

```bash
docker compose -f simulation/docker-compose.generated.yml down
```

### Removing All Simulation Data

```bash
docker compose -f simulation/docker-compose.generated.yml down -v
```

---

## 15. SSH Key Setup

The Zentra container includes `openssh-clients` and `sshpass` for connecting to monitored servers. SSH keys are generated automatically at first startup.

### How It Works

1. **Container start** — `docker-entrypoint.sh` generates an RSA 4096-bit key pair at `/root/.ssh/id_rsa` (if not already present)
2. **SSH config** — `StrictHostKeyChecking no` is set to allow first-time connections without manual host key confirmation
3. **Key persistence** — keys are generated inside the container and persist across restarts

### Setting Up SSH Key Access via Zentra UI

1. Go to **Infrastructure > Servers** and click **Add Server**
2. Enter the target IP, username (`root`), and password (`zentra123`)
3. Click **Setup SSH Key** — Zentra uses `sshpass` + `ssh-copy-id` to push the public key
4. Once keys are installed, Zentra connects using key-based SSH (no password needed)

### Manual SSH Key Setup

If you prefer to set up keys manually from inside the Zentra container:

```bash
# Shell into the Zentra container
docker exec -it zentra bash

# Copy key to a simulation node
sshpass -p "zentra123" ssh-copy-id -o StrictHostKeyChecking=no root@10.100.0.11

# Test passwordless connection
ssh root@10.100.0.11 "hostname && uptime"
```

### Verifying SSH Access

```bash
# From the host — test via docker exec
docker exec zentra ssh root@10.100.0.11 "hostname"

# Test all simulation nodes
for ip in $(seq 11 22); do
  docker exec zentra ssh -o ConnectTimeout=3 root@10.100.0.$ip "hostname" 2>/dev/null && echo "  -> OK" || echo "  -> FAIL"
done
```

---

## 16. Traffic Generator

Traffic simulation scripts generate realistic monitoring activity across all 12 simulation nodes. This is useful for:
- Populating Zentra dashboards with live data
- Testing alert rules and thresholds
- Demonstrating Zentra's monitoring capabilities

### Running the Traffic Generator

**Windows:**
```powershell
powershell -ExecutionPolicy Bypass -File simulation\traffic-sim.ps1
```

**Linux:**
```bash
chmod +x simulation/traffic-sim.sh
sudo ./simulation/traffic-sim.sh
```

Press `Ctrl+C` to stop.

### What It Does

Each cycle (every 15 seconds) the traffic generator:

| Protocol | Targets | Action |
|----------|---------|--------|
| **SNMP** | All SNMP-enabled nodes | `snmpwalk` / `snmpget` on system, CPU, memory, interfaces, storage OIDs |
| **HTTP** | Nginx, Apache, Tomcat | GET requests to web pages, status endpoints, and manager pages |
| **SSH** | Linux servers | Runs `uptime`, `df -h`, `free -m`, `ps aux`, `netstat` via docker exec |
| **Oracle** | db-oracle-01 | SQL queries (`SELECT sysdate`, tablespace usage) or TCP port check |

Every 5th cycle includes a **burst mode** with heavier traffic (additional OID walks, multiple HTTP requests, more SSH commands).

### Prerequisites

| Tool | Required For | Installation |
|------|-------------|-------------|
| `snmpwalk` / `snmpget` | SNMP traffic | **Linux:** `dnf install net-snmp-utils` or `apt install snmp` **Windows:** Net-SNMP from https://net-snmp.sourceforge.io/ |
| `sqlplus` | Oracle DB traffic | Oracle Instant Client (optional — falls back to TCP check) |
| `curl` / `Invoke-WebRequest` | HTTP traffic | Built-in on both platforms |
| `sshpass` | SSH traffic (Linux only) | `dnf install sshpass` or `apt install sshpass` |

> **Note:** The traffic generator works with whatever tools are available. Missing tools are detected at startup and those traffic types are skipped.

---

## File Structure

```
docker/
├── README.md                          # This file
├── Dockerfile                         # Zentra container image definition
├── docker-entrypoint.sh               # Container startup script (DB init, SSH keys)
├── supervisord.conf                   # Process manager configuration
├── nginx.conf                         # Nginx reverse proxy config
├── .env.example                       # Environment variable template
├── zentra-latest.tar.gz               # Pre-built Docker image archive
│
├── install.sh                         # Zentra installer (Linux)
├── install.ps1                        # Zentra installer (Windows)
├── deploy-all.sh                      # Full deployment wrapper (Linux)
├── deploy-all.ps1                     # Full deployment wrapper (Windows)
│
└── simulation/
    ├── docker-compose.yml             # Compose template (placeholders for IPs)
    ├── setup.sh                       # Simulation setup (Linux)
    ├── setup.ps1                      # Simulation setup (Windows)
    ├── traffic-sim.sh                 # Traffic generator (Linux)
    ├── traffic-sim.ps1                # Traffic generator (Windows)
    │
    └── configs/
        ├── linux-server/
        │   ├── setup-linux.sh         # Rocky Linux entrypoint (SNMP + SSH + Nginx)
        │   ├── setup-linux-ubuntu.sh  # Ubuntu entrypoint (SNMP + SSH)
        │   ├── snmpd-rocky.conf       # SNMP config for Rocky Linux
        │   └── snmpd-ubuntu.conf      # SNMP config for Ubuntu
        ├── windows-server/
        │   ├── setup-snmpd-ssh.sh     # Alpine SNMPD + SSH entrypoint
        │   ├── snmpd-win2022.conf     # SNMP config (Windows 2022 simulation)
        │   └── snmpd-win2019.conf     # SNMP config (Windows 2019 simulation)
        ├── switches/
        │   ├── snmpd-cisco-core.conf  # SNMP config (Cisco core switch)
        │   └── snmpd-arista-access.conf # SNMP config (Arista access switch)
        ├── firewall/
        │   └── snmpd-paloalto.conf    # SNMP config (Palo Alto firewall)
        ├── storage/
        │   └── snmpd-dell-powerstore.conf # SNMP config (Dell PowerStore)
        ├── middleware/
        │   ├── setup-nginx-ssh.sh     # Nginx + SSH entrypoint
        │   ├── setup-tomcat-ssh.sh    # Tomcat + SSH entrypoint
        │   ├── httpd-entry.sh         # Apache + SSH entrypoint
        │   ├── nginx-sim.conf         # Nginx site config
        │   ├── nginx-status.conf      # Nginx status endpoint
        │   ├── httpd-sim.conf         # Apache status config
        │   ├── tomcat-users.xml       # Tomcat manager credentials
        │   └── tomcat-context.xml     # Tomcat manager access config
        └── oracle/
            ├── setup-oracle-ssh.sh    # Oracle + SSH entrypoint
            └── init-oracle.sql        # DB init (monitoring user + sample schema)
```

---

## 17. Building the Docker Image

This section is for engineers who need to rebuild the Docker image from source.

### Prerequisites on the Build Server (`10.88.11.162`)

| Requirement | Version |
|---|---|
| Rust toolchain | `rustc 1.95+`, `cargo` |
| Node.js | 18+ with npm |
| Docker | Installed and running |
| Network | Access to pull `rockylinux:8` base image and RPM repos |

### Build Steps

```bash
# SSH into the build server
ssh root@10.88.11.162
cd /store/Zentra

# 1. Build frontend
cd frontend && npm ci && npx vite build && cd ..

# 2. Build backend (release)
cargo build --release --bin zentra-server

# 3. Build Docker image
docker build -f docker/Dockerfile -t zentra:latest .

# 4. Export image as tarball
docker save zentra:latest | gzip > docker/zentra-latest.tar.gz
ls -lh docker/zentra-latest.tar.gz
```

### Validate the Build

```bash
# Check build info
docker run --rm zentra:latest cat /opt/zentra/.build-info

# Verify binary exists and is stripped
docker run --rm zentra:latest ls -la /usr/local/bin/zentra-server
docker run --rm zentra:latest file /usr/local/bin/zentra-server

# Verify frontend dist
docker run --rm zentra:latest ls /opt/zentra/frontend/dist/index.html

# Verify no source code leaked
docker run --rm zentra:latest sh -c 'find / -name "*.rs" -o -name "*.toml" -o -name "Cargo.lock" -o -name ".git" -o -name "node_modules" 2>/dev/null | head -20'
# Must return empty

# Run full validation suite
bash docker/validate.sh
```

### Environment-Specific Examples

**Quick local test (zero config):**
```bash
docker run -d -p 5010:5010 \
  --security-opt=no-new-privileges:true \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

**Production (hardened, with persistent storage + env file):**
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

**Air-gapped deployment:**
```bash
# On a machine with no internet
docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 --name zentra \
  --security-opt=no-new-privileges:true \
  -v zentra-data:/var/lib/postgresql/data \
  zentra:latest
```

### Downloading the Image Locally

```powershell
# From your Windows dev machine
pscp -pw skyline root@10.88.11.162:/store/Zentra/docker/zentra-latest.tar.gz C:\git\Zentra\docker\zentra-latest.tar.gz
```

### Renewing / Extending Licenses

The Docker image includes a 3-day trial license that activates on first boot.
After the trial expires, the container continues to run in **degraded mode** — only the Dashboard is displayed and all other features are disabled.
To renew:

1. **Rebuild and redeploy** — Build a fresh image and deploy it. The license resets on first boot with a new volume.
2. **Reset the volume** — `docker volume rm zentra-data` and re-run the container (data will be lost).
3. **Contact Zentra team** — For extended or permanent licenses.

---

## 18. Security Considerations

The Docker image includes multiple layers of hardening:

| Measure | Description |
|---|---|
| **No source code** | Image contains only compiled binary, frontend dist, and SQL migrations. No `.rs`, `.toml`, `.git`, `node_modules`. |
| **Stripped binary** | Debug symbols removed from `zentra-server` to reduce size and prevent reverse engineering. |
| **Non-root runtime** | `zentra-server` runs as dedicated `zentra` user (no shell access). |
| **Restricted DB user** | API connects as `zentra_app` (DML-only), not the `postgres` superuser. |
| **Internal-only API** | Backend binds to `127.0.0.1:5006` — only accessible via Nginx proxy on port 5010. |
| **Internal-only DB** | PostgreSQL binds to `127.0.0.1:5434` — not exposed outside the container. |
| **Single port exposure** | Only port 5010 is `EXPOSE`d. No direct access to API or DB ports. |
| **Tamper protection** | SHA-256 integrity manifest verifies critical files on every startup. |
| **License enforcement** | Signed 3-day trial license stored in the PG volume, verified independently by the Rust binary (HMAC + binary hash). After expiry, runs in degraded mode (Dashboard only). |
| **Immutable files** | Config, license, binary, migrations, and frontend files are locked with `chattr +i` after startup. Even root cannot modify them. |
| **No shells** | All shell binaries (`bash`, `sh`, `csh`, `zsh`, etc.) are removed 5 seconds after startup. `docker exec` becomes useless. |
| **No package managers** | `dnf`, `yum`, `rpm` are removed at startup. No software can be installed at runtime. |
| **No editors** | `vi`, `vim`, `nano`, `ed`, `perl`, `lua`, `ruby` all removed. |
| **No dangerous tools** | `wget`, `ftp`, `telnet`, `dd`, `strings`, `gcc`, `make`, `strace`, `gdb` all removed. |
| **chattr removed** | After making files immutable, `chattr` itself is deleted — immutable bits can never be reversed. |
| **SSH preserved** | Only `sshpass`, `ssh`, `scp` are kept (required for monitoring target servers). |
| **Rate limiting** | Nginx rate-limits login endpoint (5 req/min per IP). |
| **Security headers** | CSP, X-Frame-Options, X-Content-Type-Options, Referrer-Policy all set. |
| **Core dumps disabled** | Hard and soft core limits set to 0. SUID/SGID bits cleared on all binaries. |
| **Blocked endpoints** | `/api/system/*` and `/api/pool-status` blocked from external access via Nginx. |
| **Binary-verified license** | Rust backend independently reads the license file from disk, verifies the HMAC against its own SHA-256 hash, and checks expiry. Env var spoofing is ignored in Docker. |
| **Anti-deletion** | If the license file is missing but the DB directory exists, the backend treats it as a bypass attempt and enforces degraded mode. |

### Recommended Hardened Run Command

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

> **Note:** The `ZENTRA_SKIP_LOCKDOWN=1` env var disables the self-lockdown phase (for debugging only — never use in production).

---

## License

Proprietary — Zentra Team
