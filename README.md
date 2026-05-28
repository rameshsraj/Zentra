# Zentra — Installation & Configuration Guide

<p align="center">
  <code>ghcr.io/rameshsraj/zentra:latest</code>
</p>

Zentra is an all-in-one infrastructure monitoring platform packaged as a single Docker image containing PostgreSQL 16 + TimescaleDB, a Rust API backend, React frontend, and Nginx.

---

## Table of Contents

- [1. Prerequisites](#1-prerequisites)
- [2. Install Docker](#2-install-docker)
- [3. Pull the Zentra Image](#3-pull-the-zentra-image)
- [4. Configure Persistent Storage](#4-configure-persistent-storage)
- [5. Configure Environment Variables](#5-configure-environment-variables)
- [6. Run the Container](#6-run-the-container)
- [7. Verify the Installation](#7-verify-the-installation)
- [8. Access the Application](#8-access-the-application)
- [9. Stop, Start & Restart](#9-stop-start--restart)
- [10. Backup & Restore Data](#10-backup--restore-data)
- [11. Update to a New Version](#11-update-to-a-new-version)
- [12. Troubleshooting](#12-troubleshooting)

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

## 3. Pull the Zentra Image

The image is hosted on GitHub Container Registry (GHCR).

```bash
# If the package is private, authenticate first:
echo "YOUR_GITHUB_PAT" | docker login ghcr.io -u YOUR_GITHUB_USERNAME --password-stdin

# Pull the image
docker pull ghcr.io/rameshsraj/zentra:latest
```

To generate a GitHub PAT, go to: https://github.com/settings/tokens/new?scopes=read:packages

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

```bash
docker run -d \
  --name zentra \
  --restart unless-stopped \
  --env-file /opt/zentra/.env \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  -v /opt/zentra/logs:/opt/zentra/logs \
  ghcr.io/rameshsraj/zentra:latest
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

### Minimal Development Command

```bash
docker run -d \
  --name zentra \
  -e ZENTRA_JWT_SECRET="dev-only-secret-key-32-chars-minimum" \
  -p 5010:5010 \
  -v zentra-data:/var/lib/postgresql/data \
  ghcr.io/rameshsraj/zentra:latest
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
| **Password** | *(set during first boot)* |

> **Change the default password immediately** after first login. Go to **Admin > My Profile** in the left sidebar.

### Application Sections

After login, the sidebar provides access to all modules:

| Section | What It Does |
|---------|-------------|
| **Command Center** | Overview dashboard with health scores, alert counts, target status |
| **Infrastructure** | Monitor servers (SSH), storage arrays, network devices (SNMP) |
| **Database** | Monitor PostgreSQL, MySQL, Oracle, MongoDB, Redis |
| **Middleware** | Monitor Nginx, Apache, Tomcat, HAProxy, Elasticsearch, Redis, Memcached, RabbitMQ, Kafka, Docker, Kubernetes |
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
  ghcr.io/rameshsraj/zentra:latest
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
  ghcr.io/rameshsraj/zentra:latest

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

## License

Proprietary — Zentra Team

### Production Releases
Production binaries MUST be built via the `release-prod` target using the Makefile to enable runtime licensing enforcement. A binary built without this compile-time feature will refuse to enforce licensing.
```bash
make release-prod
```
