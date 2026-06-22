# Zentra — Operations Guide
## Application Architecture, Configuration & Agent Deployment

---

## Table of Contents

1. [How Zentra Works](#1-how-zentra-works)
2. [Application Installation](#2-application-installation)
3. [Application Configuration Reference](#3-application-configuration-reference)
4. [First Login & Initial Setup](#4-first-login--initial-setup)
5. [Agent Overview](#5-agent-overview)
6. [Agent Modes](#6-agent-modes)
7. [Agent Installation — Linux](#7-agent-installation--linux)
8. [Agent Installation — Windows](#8-agent-installation--windows)
9. [Agent Configuration Reference](#9-agent-configuration-reference)
10. [Registering an Agent in the UI](#10-registering-an-agent-in-the-ui)
11. [Simulation Lab](#11-simulation-lab)
12. [Troubleshooting](#12-troubleshooting)

---

## 1. How Zentra Works

```
┌──────────────────────────────────────────────────────────────────────┐
│  Zentra Docker Container (single image, all-in-one)                  │
│                                                                       │
│  ┌──────────┐   proxy   ┌──────────────────┐   SQL    ┌───────────┐  │
│  │  Nginx   │ ────────► │  zentra-server   │ ───────► │ Postgres  │  │
│  │ port 5010│           │  (Rust API)      │          │ + Timescale│  │
│  └──────────┘           │  port 5006       │          └───────────┘  │
│                         └──────────────────┘                         │
│              React SPA served from /opt/zentra/frontend/dist         │
└──────────────────────────────────────────────────────────────────────┘
         ▲                           ▲
         │ Browser (port 5010)       │ Agent pushes metrics/logs
         │                           │ (HTTP POST, port 5006)
   Your Windows / Mac          ┌─────┴───────┐
   or any browser              │ zentra-agent│  (separate binary,
                               │ on each host│   runs anywhere)
                               └─────────────┘
```

**Key points:**
- Zentra runs as a **single Docker container** containing PostgreSQL, the Rust API server, and the React frontend served via Nginx
- Port **5010** is the only externally exposed port (Nginx → frontend + API proxy)
- The API (`zentra-server`) listens internally on **5006** — never exposed directly
- PostgreSQL listens on **5434** — internal only, never exposed
- `zentra-agent` is a **separate binary** installed on each server you want to monitor
- Agents communicate with Zentra over HTTP/HTTPS to port **5006** (through Docker port mapping)

---

## 2. Application Installation

### Windows (install.ps1)

**Prerequisites:** Docker Desktop installed and running.

```powershell
# Run from the folder containing install.ps1 and zentra-latest.tar.gz
powershell -ExecutionPolicy Bypass -File install.ps1
```

**What it does (in order):**
1. Checks Docker is installed and running
2. Removes any previous `zentra` container, volume, and image (clean slate)
3. Loads `zentra-latest.tar.gz` from same folder — or pulls from GHCR if not found
4. Generates `C:\zentra\.env` with random JWT secret and DB password
5. Starts the container: `docker run -d -p 5010:5010 --env-file C:\zentra\.env zentra:latest`
6. Waits up to 60s for health check at `http://localhost:5010/api/health`
7. Prints access URL using the server's LAN IP
8. **Optionally** launches the Simulation Lab (prompts Y/N)

### Linux (install.sh)

```bash
sudo bash install.sh
```

Same flow as above — generates `/opt/zentra/.env`, starts container on port `5010`.

### Full Deployment with Simulation (deploy-all.ps1)

```powershell
powershell -ExecutionPolicy Bypass -File deploy-all.ps1
```

Runs `install.ps1` + `simulation/setup.ps1` in sequence, then distributes SSH keys from the Zentra container to all 12 simulation nodes for passwordless monitoring.

---

## 3. Application Configuration Reference

Configuration is passed via the `.env` file (auto-generated at install, located at `C:\zentra\.env` on Windows or `/opt/zentra/.env` on Linux).

### Core Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZENTRA_JWT_SECRET` | auto-generated | JWT signing secret — **must be 32+ chars**. Change this! |
| `ZENTRA_MASTER_KEY` | auto-generated | Encryption key for stored credentials (SSH passwords, SNMP strings) |
| `ZENTRA_API_PORT` | `5006` | Internal API port (inside container, not exposed) |
| `ZENTRA_ALLOW_ORIGINS` | `*` | CORS allowed origins. Use `*` for open access or restrict to `http://yourserver:5010` |
| `ZENTRA_ORG_ID` | `00000000-0000-0000-0000-000000000001` | Organization UUID — leave as default for single-org installs |
| `ZENTRA_SKIP_LOCKDOWN` | `0` | Set to `1` to keep shells available (needed for `docker exec` in simulation mode) |

### Database Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `POSTGRES_USER` | `postgres` | PostgreSQL superuser |
| `POSTGRES_PASSWORD` | auto-generated | DB password — **store securely** |
| `POSTGRES_DB` | `zentra` | Database name |
| `POSTGRES_PORT` | `5434` | Internal PostgreSQL port |

### Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level: `trace`, `debug`, `info`, `warn`, `error` |

### Example .env

```env
POSTGRES_USER=postgres
POSTGRES_PASSWORD=a3f8c2d1e9b7...   ← auto-generated, do not share
POSTGRES_DB=zentra
POSTGRES_PORT=5434

ZENTRA_JWT_SECRET=7d2f1a...         ← auto-generated 64-char hex
ZENTRA_MASTER_KEY=8e3b4c...         ← auto-generated 64-char hex
ZENTRA_ALLOW_INSECURE_JWT=1
ZENTRA_API_PORT=5006
ZENTRA_ALLOW_ORIGINS=*
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
ZENTRA_SKIP_LOCKDOWN=1

RUST_LOG=info
```

> **Note:** The `.env` file is regenerated on each fresh install with new random secrets. If you upgrade or re-install, your old secrets are discarded and the database is wiped. To preserve data across reinstalls, back up the volume: `docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine tar czf /backup/zentra-backup.tar.gz /data`

---

## 4. First Login & Initial Setup

1. Open browser → `http://<server-ip>:5010`
2. Login: `admin@Zentra.local` / `changeme`
3. **Immediately change the admin password** (Settings → Users → Edit)

### After Login

| Step | Where | What to do |
|------|-------|------------|
| **Add monitoring targets** | Infrastructure → Servers | Add servers by IP, hostname, SNMP community |
| **Configure SNMP** | Targets → Add Target | Set community string, SNMP version (v1/v2c/v3) |
| **Add SSH credentials** | Targets → Add Target | Username/password or SSH key path |
| **Register agents** | Fleet → Agents | Generate API key, install agent on host |
| **Create dashboards** | Dashboards | Drag-and-drop panels for metrics |
| **Set up alerts** | Alerts → Rules | Threshold-based, anomaly, or log pattern alerts |

---

## 5. Agent Overview

The `zentra-agent` is a **lightweight Rust binary** (~13MB) that runs on each server you want to monitor. It handles:

- **Metric collection** — CPU, memory, disk, network, process stats
- **Log collection** — syslog, Windows Event Log, application logs
- **Plugin execution** — SNMP polling, SSH commands, Oracle OCI queries
- **Heartbeat** — reports health to Zentra every 30 seconds
- **Offline buffering** — if Zentra is unreachable, metrics are buffered locally and flushed when reconnected

---

## 6. Agent Modes

The agent runs in one of two modes, controlled by `ZENTRA_AGENT_MODE`:

### LOCAL mode (default — same machine as Zentra)

```
zentra-agent ──► PostgreSQL (direct DB connection)
```

- Agent writes directly to the Zentra database
- Requires `DATABASE_URL` pointing to the Zentra PostgreSQL
- Used when the agent runs **on the same host** as the Zentra container
- No API key needed

### REMOTE mode (any external server)

```
zentra-agent ──► HTTP POST ──► zentra-server:5006 ──► PostgreSQL
```

- Agent sends metrics over HTTP to the Zentra API
- Requires `ZENTRA_AGENT_MODE=remote`, an API endpoint, and an API key
- **This is the standard mode for monitoring external servers**
- Supports offline buffering if the Zentra server is temporarily unreachable

---

## 7. Agent Installation — Linux

### Step 1 — Copy the binary

```bash
# Create agent directory
mkdir -p /opt/zentra-agent/buffer /opt/zentra-agent/logs

# Copy the binary (from your Zentra deployment package)
cp zentra-agent /opt/zentra-agent/zentra-agent
chmod +x /opt/zentra-agent/zentra-agent
```

### Step 2 — Create environment config

```bash
cat > /etc/zentra-agent.env << 'EOF'
ZENTRA_AGENT_MODE=remote
ZENTRA_API_ENDPOINT=http://<zentra-server-ip>:5006
ZENTRA_AGENT_API_KEY=isok_<your-api-key-from-ui>
ZENTRA_DC_NAME=production-dc-01
ZENTRA_BUFFER_DIR=/opt/zentra-agent/buffer
RUST_LOG=info
EOF

chmod 600 /etc/zentra-agent.env
```

### Step 3 — Create systemd service

```bash
cat > /etc/systemd/system/zentra-agent.service << 'EOF'
[Unit]
Description=Zentra Monitoring Agent
After=network.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=/etc/zentra-agent.env
ExecStart=/opt/zentra-agent/zentra-agent
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=zentra-agent

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable zentra-agent
systemctl start zentra-agent
```

### Step 4 — Verify

```bash
systemctl status zentra-agent
journalctl -u zentra-agent -f
```

You should see:
```
Agent running in REMOTE mode (no database connection)
Heartbeat loop started
Remote target fetcher started
```

---

## 8. Agent Installation — Windows

### Step 1 — Copy the binary

```powershell
New-Item -ItemType Directory -Force "C:\zentra-agent\buffer"
New-Item -ItemType Directory -Force "C:\zentra-agent\logs"
Copy-Item zentra-agent.exe "C:\zentra-agent\zentra-agent.exe"
```

### Step 2 — Create environment config

```powershell
@"
ZENTRA_AGENT_MODE=remote
ZENTRA_API_ENDPOINT=http://<zentra-server-ip>:5006
ZENTRA_AGENT_API_KEY=isok_<your-api-key-from-ui>
ZENTRA_DC_NAME=windows-dc-01
ZENTRA_BUFFER_DIR=C:\zentra-agent\buffer
RUST_LOG=info
"@ | Out-File -FilePath "C:\zentra-agent\agent.env" -Encoding utf8
```

### Step 3 — Install as Windows Service (using NSSM)

```powershell
# Download NSSM from https://nssm.cc
nssm install ZentraAgent "C:\zentra-agent\zentra-agent.exe"
nssm set ZentraAgent AppEnvironmentExtra `
    "ZENTRA_AGENT_MODE=remote" `
    "ZENTRA_API_ENDPOINT=http://<zentra-server-ip>:5006" `
    "ZENTRA_AGENT_API_KEY=isok_<your-api-key>" `
    "ZENTRA_DC_NAME=windows-server-01" `
    "ZENTRA_BUFFER_DIR=C:\zentra-agent\buffer"
nssm set ZentraAgent AppStdout "C:\zentra-agent\logs\agent.log"
nssm set ZentraAgent AppStderr "C:\zentra-agent\logs\agent-error.log"
nssm start ZentraAgent
```

---

## 9. Agent Configuration Reference

All agent configuration is via environment variables (or `.env` file loaded at startup):

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `ZENTRA_AGENT_MODE` | No | `local` | `local` or `remote` |
| `ZENTRA_API_ENDPOINT` | **Yes** (remote) | — | Full URL of Zentra API: `http://<ip>:5006` |
| `ZENTRA_AGENT_API_KEY` | **Yes** (remote) | — | API key from Zentra UI, must start with `isok_` |
| `ZENTRA_DC_NAME` | No | hostname | Label shown in Fleet → Agents view |
| `ZENTRA_BUFFER_DIR` | No | `/var/lib/zentra-agent/buffer` | Local disk buffer for offline metric batches |
| `ZENTRA_COLLECTION_INTERVAL` | No | `60` | Metric collection interval in seconds |
| `ZENTRA_ORG_ID` | No | nil UUID | Organization UUID (for multi-tenant setups) |
| `DATABASE_URL` | **Yes** (local only) | — | PostgreSQL connection string for LOCAL mode |
| `RUST_LOG` | No | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |

### API Key Format

Keys are generated by Zentra and always start with `isok_` followed by 32 alphanumeric characters:

```
isok_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6
```

Keys are stored **hashed** (SHA-256) in the database. The plain-text key is only shown **once** at generation time — copy it immediately.

---

## 10. Registering an Agent in the UI

1. Login to Zentra → **Fleet** → **Agents**
2. Click **Register New Agent**
3. Fill in:
   - **Datacenter Name** — e.g. `prod-dc-london` (used as the `ZENTRA_DC_NAME`)
   - **Description** — optional label
   - **Region** — optional grouping label
4. Click **Generate Key** → copy the `isok_...` key immediately (shown only once)
5. Use this key as `ZENTRA_AGENT_API_KEY` in the agent's env config on the target server
6. Start the agent — it appears in the Fleet view within 30 seconds (first heartbeat)

### Agent Status in Fleet View

| Status | Meaning |
|--------|---------|
| **Online** | Heartbeat received within last 2 minutes |
| **Degraded** | Heartbeat delayed 2–10 minutes |
| **Offline** | No heartbeat for 10+ minutes |

---

## 11. Simulation Lab

The simulation lab creates 12 Docker containers that simulate a real datacenter. Useful for testing dashboards, alerts, and agent monitoring before deploying to production.

### Topology

| Container | IP | Services | Monitored via |
|-----------|-----|----------|---------------|
| `srv-linux-01` | 10.100.0.11 | Rocky Linux, SNMP, SSH, Nginx | Agent (REMOTE) + SNMP + SSH |
| `srv-linux-02` | 10.100.0.12 | Ubuntu 22.04, SNMP, SSH, Nginx | Agent (REMOTE) + SNMP + SSH |
| `srv-win-01` | 10.100.0.13 | Windows (simulated), SNMP | SNMP + SSH |
| `srv-win-02` | 10.100.0.14 | Windows (simulated), SNMP | SNMP + SSH |
| `sw-core-01` | 10.100.0.15 | Cisco Catalyst (simulated) | SNMP |
| `sw-access-01` | 10.100.0.16 | Arista 7050X (simulated) | SNMP |
| `fw-01` | 10.100.0.17 | Palo Alto PA-3260 (simulated) | SNMP |
| `stor-01` | 10.100.0.18 | Dell PowerStore SAN (simulated) | SNMP + SSH |
| `mw-nginx-01` | 10.100.0.19 | Nginx Reverse Proxy | SNMP + HTTP |
| `mw-apache-01` | 10.100.0.20 | Apache HTTP Server | SNMP + HTTP |
| `mw-tomcat-01` | 10.100.0.21 | Apache Tomcat 9 | SNMP + HTTP |
| `db-oracle-01` | 10.100.0.22 | Oracle 21c XE | OCI + SNMP |

### SSH Credentials (for manual access)

- **Username:** `root`
- **Password:** `zentra123`
- **SSH key:** distributed automatically by `deploy-all.ps1` to `/root/.ssh/authorized_keys`

### Agent binary

The `zentra-agent` binary must be present at `docker/simulation/bin/zentra-agent` before running `setup.ps1`. It is mounted into the Linux containers at `/opt/zentra-agent/zentra-agent` and started automatically.

The agent connects to Zentra via the Docker bridge gateway IP (auto-detected by the setup script as `__ZENTRA_HOST__`).

---

## 12. Troubleshooting

### App not accessible from browser

```powershell
# Check container is running
docker ps | findstr zentra

# Check which port is bound
docker port zentra

# Test health endpoint from the server itself
curl http://localhost:5010/api/health

# Check logs
docker logs zentra --tail 50
```

**Common causes:**
- Windows Firewall blocking port 5010 → Allow inbound rule for TCP 5010
- Container exited on startup → `docker logs zentra` for error details
- Wrong IP detected in summary → use the IP shown by `ipconfig` directly

### Agent not appearing in Fleet view

```bash
# Check agent is running
systemctl status zentra-agent

# Check agent logs
journalctl -u zentra-agent -n 50

# Test connectivity to Zentra API from agent host
curl http://<zentra-ip>:5006/api/health
```

**Common causes:**
- Wrong `ZENTRA_API_ENDPOINT` (check the IP and port)
- API key typo — must start with `isok_`
- Firewall blocking outbound TCP 5006 from the agent host

### SSH key distribution failing in deploy-all.ps1

The Zentra container removes its shells 5 seconds after startup (lockdown). `deploy-all.ps1` requires `docker exec` to work.

**Fix:** Ensure `ZENTRA_SKIP_LOCKDOWN=1` is in `C:\zentra\.env`. This is set automatically by `install.ps1` — if missing, stop the container, add the line, and restart:

```powershell
Add-Content "C:\zentra\.env" "`nZENTRA_SKIP_LOCKDOWN=1"
docker restart zentra
```

Then re-run `deploy-all.ps1`.

### Simulation nodes not getting SSH keys

```powershell
# Manually push key from Zentra container to a sim node
docker exec zentra bash -c "cat /root/.ssh/id_rsa.pub | sshpass -p zentra123 ssh -o StrictHostKeyChecking=no root@10.100.0.11 'cat >> /root/.ssh/authorized_keys'"

# Test passwordless SSH
docker exec zentra ssh root@10.100.0.11 hostname
```

If `bash` is not found → `ZENTRA_SKIP_LOCKDOWN=1` was not set when the container started (see above).
