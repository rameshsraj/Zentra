# Zentra — GCP Deployment Runbook
## Bare-Metal / VM Deployment on Debian 12

This runbook covers end-to-end deployment of Zentra on a fresh **Debian 12 (Bookworm)** GCP VM using pre-built binaries from the private `rameshsraj/zentra-release` GitHub repo, plus agent installation on monitored hosts.

---

## Architecture

```
┌─────────────────────────────────────────────┐
│  GCP VM (Debian 12, /store/zentra)          │
│                                              │
│  nginx:5010 ──► zentra-server:5006 ──► PG   │
│  (frontend)     (Rust API)         :5434     │
└─────────────────────────────────────────────┘
            ▲                  ▲
            │ Browser          │ Agent POST (isok_ key)
   User on laptop       ┌──────┴──────┐
                        │ zentra-agent│ on each monitored host
                        └─────────────┘
```

---

## Prerequisites

| Requirement | Details |
|-------------|---------|
| GCP VM | Debian 12, e2-standard-2+ (2 vCPU, 8 GB RAM) |
| Disk | `/store` mount, 50 GB+ |
| Network tag | `http-server` on the instance |
| Firewall rule | TCP 5010 open from `0.0.0.0/0` to tag `http-server` |
| Optional firewall | TCP 5006 if agents are outside the GCP VPC |
| GitHub PAT | `repo` scope for `rameshsraj/zentra-release` and `rameshsraj/zentra-agent` |

---

## Step 1 — GCP Firewall Rule (one-time)

```bash
gcloud compute firewall-rules create allow-zentra-5010 \
  --allow tcp:5010 \
  --source-ranges 0.0.0.0/0 \
  --target-tags http-server
```

If agents will connect from outside the VPC, also open TCP 5006.

---

## Step 2 — Pull Release on the GCP VM

```bash
sudo mkdir -p /store/zentra
sudo chown $USER:$USER /store/zentra

git clone https://<YOUR_GITHUB_PAT>@github.com/rameshsraj/zentra-release.git /store/zentra
ls /store/zentra/bin/zentra-server /store/zentra/frontend/dist/index.html
```

---

## Step 3 — Run Setup Script

```bash
cd /store/zentra
sudo bash deploy/setup-debian12.sh
```

The script performs 6 steps:

| Step | Action |
|------|--------|
| 1 | Install nginx, PostgreSQL 16, TimescaleDB, **libssl1.1** (OpenSSL 1.1 compat) |
| 2 | Initialise PostgreSQL at `/store/zentra/pgdata`, port 5434 |
| 3 | Create `zentra` service user, generate `.env` |
| 4 | Configure nginx reverse proxy (5010 → 5006), fix bundled paths |
| 5 | Create and start `zentra-api` systemd service |
| 6 | Health-check loop (15 × 3 s) |

---

## Step 4 — Post-install DB Fix (one-time)

PostgreSQL migration `27` (row-level security) needs superuser to set `app.current_org_id`. Grant it:

```bash
sudo -u postgres psql -p 5434 -d zentra -c "ALTER ROLE zentra_app SUPERUSER;"
sudo systemctl restart zentra-api
sleep 5

# Confirm migrations applied (should show v80 or later)
sudo -u postgres psql -p 5434 -d zentra -c \
  "SELECT version, description FROM _sqlx_migrations ORDER BY version DESC LIMIT 5;"
```

> If the deployed binary predates migration `80`, apply it manually:
> ```bash
> sudo -u postgres psql -p 5434 -d zentra \
>   -f /store/zentra/migrations/20250101000080_otlp_api_keys_columns.sql
> ```

---

## Step 5 — Verify Services

```bash
systemctl status zentra-api --no-pager
systemctl status postgresql@16-main --no-pager
systemctl status nginx --no-pager

curl -s http://127.0.0.1:5006/api/health
# Expected: {"database":true,"status":"healthy",...}

curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:5010/
# Expected: 200
```

---

## Step 6 — First Login

Open: **`http://<EXTERNAL_IP>:5010`**

| Field | Value |
|-------|-------|
| Email | `admin@Zentra.local` |
| Password | `admin123` |

Change the password immediately at **Settings → Profile → Change Password**.

---

## Step 7 — Generate an Agent API Key

Each agent / datacenter needs its own `isok_*` key.

### Via the UI
1. Login as admin
2. **Fleet → Agents → Register New Agent**
3. Enter:
   - **Datacenter name** — e.g. `gcp-prod`
   - **Description** — optional
4. Click **Generate Key**
5. **Copy the `isok_...` key immediately** — shown only once

### Via curl (from the GCP VM)
```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:5006/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@Zentra.local","password":"admin123"}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")

curl -s -X POST http://127.0.0.1:5006/api/agents/register \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"datacenter_name":"gcp-prod","description":"GCP VM agent"}' \
  | python3 -m json.tool
```

Response:
```json
{
  "agent_id": "019ef524-05e8-72e3-...",
  "api_key": "isok_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "warning": "Store this key securely. It will not be shown again."
}
```

**Save the `api_key`** to a password manager or secrets vault now.

---

## Step 8 — Install an Agent on a Monitored Host

```bash
# On the host you want to monitor (Linux)
sudo mkdir -p /store/zentra-agent
git clone https://<YOUR_GITHUB_PAT>@github.com/rameshsraj/zentra-agent.git /store/zentra-agent
cd /store/zentra-agent
sudo bash deploy/setup-agent.sh
```

The setup script prompts for:

| Prompt | Example |
|--------|---------|
| API endpoint | `http://<GCP_VM_EXTERNAL_IP>:5006` |
| Datacenter name | `gcp-prod` (must match Step 7) |
| Agent API key | `isok_xxxxxxxx...` |

It installs SSH/SNMP tools, generates SSH keys, writes `.env`, and starts the `zentra-agent` systemd service.

**Verify:**
```bash
systemctl status zentra-agent --no-pager
tail -f /store/zentra-agent/logs/agent.log
```

The agent will appear in **Fleet → Agents** in the Zentra UI within ~30 s with status `active`.

### Windows / Docker agent
For Windows hosts, use `deploy/run-agent-docker.ps1` from the agent repo — it loads the agent Docker image and runs it as a container with the same env vars.

---

## Step 9 — Add Monitoring Targets

Once an agent is `active`, add targets in the UI:

| Type | Path | Required info |
|------|------|---------------|
| Linux server | **Infrastructure → Servers → Add Server** | SSH host, port, user, key/password, SNMP community |
| Nginx | **Middleware → Add Service** (type: nginx) | `stub_status` URL |
| Apache | **Middleware → Add Service** (type: apache) | `server-status` URL |
| Oracle | **Database → Oracle → Add Connection** | Host, port, service name, user, password |
| Network device | **Infrastructure → Network → Add Device** | IP, SNMP community |

Each target is assigned a **datacenter**; the matching agent picks it up on its next poll (~30 s).

The Middleware sidebar sub-menus (Nginx, Apache, …) are populated dynamically — they appear only after a target of that type is added and the agent has reported data.

---

## Step 10 — Spin up the Windows Local Lab (Optional)

The local lab runs 3 simulated Linux hosts + a Zentra agent in Docker Desktop on your Windows laptop. The agent connects to your GCP Zentra server, giving you instant data to explore in the UI without provisioning real infrastructure.

### What it provides

| Container | Image | Purpose |
|-----------|-------|---------|
| `lab-linux-01` | debian:12 | Plain Linux host with SSH + SNMP |
| `lab-nginx-01` | debian:12 | Nginx with `stub_status` |
| `lab-httpd-01` | debian:12 | Apache with `server-status` |
| `zentra-agent` | `zentra-agent:latest` | Polls the 3 hosts and pushes to GCP |

### Prerequisites on Windows

- **Docker Desktop** running (WSL2 backend)
- **PowerShell 7+**
- **Agent image tarball** `zentra-agent-latest.tar.gz` (built on `smarsh-nexus`)

### 1. Copy the agent tarball to Windows

On the **build server** (`smarsh-nexus`), export the agent image:
```bash
docker save zentra-agent:latest | gzip > /tmp/zentra-agent-latest.tar.gz
```

Copy it to your Windows machine (any method — SCP, shared folder, USB):
```powershell
# From PowerShell on Windows
scp root@10.88.11.162:/tmp/zentra-agent-latest.tar.gz `
    "$env:USERPROFILE\Projects\Zentra\docker\local-lab\zentra-agent-latest.tar.gz"
```

### 2. Generate a dedicated API key for the lab

On the **GCP VM** (or via the UI), create a separate key so lab data is tagged distinctly:
```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:5006/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@Zentra.local","password":"admin123"}' \
  | python3 -c "import sys,json; print(json.load(sys.stdin)['token'])")

curl -s -X POST http://127.0.0.1:5006/api/agents/register \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"datacenter_name":"dc-local-lab","description":"Windows local lab"}' \
  | python3 -m json.tool
```

Copy the `isok_...` key.

### 3. Start the lab

On Windows, in PowerShell:
```powershell
cd $env:USERPROFILE\Projects\Zentra\docker\local-lab

.\start-lab.ps1 `
    -ApiEndpoint "http://<GCP_VM_EXTERNAL_IP>:5010" `
    -AgentApiKey "isok_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" `
    -DcName "dc-local-lab"
```

The script will:
1. Verify Docker Desktop is running
2. Load `zentra-agent-latest.tar.gz` into Docker
3. Build the 3 service containers from `docker-compose.yml`
4. Inject the API endpoint + key into the agent container
5. Bring everything up with `docker compose up -d`

### 4. Verify in the Zentra UI

Open `http://<GCP_VM_EXTERNAL_IP>:5010` and check:

| Where | What you should see |
|-------|---------------------|
| **Fleet → Agents** | A new agent `dc-local-lab` with status `active` |
| **Infrastructure → Servers** | 3 hosts: `lab-linux-01`, `lab-nginx-01`, `lab-httpd-01` |
| **Middleware** | Sub-menus for **Nginx** and **Apache** appear within ~2 min |

### Lab management

```powershell
# View logs
docker logs -f zentra-agent
docker logs -f lab-nginx-01

# Restart just the agent
docker restart zentra-agent

# Tear down the lab
cd $env:USERPROFILE\Projects\Zentra\docker\local-lab
docker compose down -v
```

### Troubleshooting the lab

| Symptom | Fix |
|---------|-----|
| Agent logs show `connection refused` | Check the GCP firewall allows port 5010 from your Windows public IP (`curl ifconfig.me` to find it) |
| `docker load` fails with "no such file" | Confirm `zentra-agent-latest.tar.gz` is in the `local-lab/` folder next to `start-lab.ps1` |
| Lab containers exit immediately | `docker compose logs lab-linux-01` — usually a setup-script syntax issue; rebuild with `docker compose build --no-cache` |
| Agent registered but no targets show up | Wait 2 min for the first poll; check `docker logs zentra-agent` for SSH/SNMP errors |

---

## Re-running / Upgrading

```bash
cd /store/zentra
sudo git pull origin main
sudo systemctl restart zentra-api
```

The setup script is idempotent — re-running is safe.

---

## Key File Locations

| Path | Purpose |
|------|---------|
| `/store/zentra/bin/zentra-server` | API binary |
| `/store/zentra/frontend/dist/` | React SPA |
| `/store/zentra/.env` | Application secrets |
| `/store/zentra/pgdata/` | PostgreSQL data dir |
| `/store/zentra/logs/api.log` | API stdout |
| `/store/zentra/logs/api-error.log` | API stderr |
| `/etc/systemd/system/zentra-api.service` | Systemd unit |
| `/etc/nginx/sites-available/zentra` | Nginx server block |
| `/etc/postgresql/16/main/postgresql.conf` | PG main config |
| `/var/log/zentra-setup.log` | Setup script log |

---

## Service Management

```bash
# Status
systemctl status zentra-api postgresql@16-main nginx

# Restart
sudo systemctl restart zentra-api
sudo systemctl reload nginx

# Logs
tail -f /store/zentra/logs/api.log
journalctl -u zentra-api -f
```

---

## Troubleshooting

### `zentra-api` exits with code 127 (libssl.so.1.1 not found)

```bash
wget "http://security.debian.org/debian-security/pool/updates/main/o/openssl/libssl1.1_1.1.1w-0+deb11u5_amd64.deb" -O /tmp/libssl1.1.deb
sudo dpkg -i /tmp/libssl1.1.deb
sudo systemctl restart zentra-api
```

### Migration `27` fails: `permission denied for parameter app.current_org_id`

```bash
sudo -u postgres psql -p 5434 -d zentra -c "ALTER ROLE zentra_app SUPERUSER;"
sudo systemctl restart zentra-api
```

### `POST /api/agents/register` returns `500 internal error`

Check `/store/zentra/logs/api.log` for the SQL error. If it says `column "key_prefix" / "rate_limit_rpm" / "enabled" of relation "otlp_api_keys" does not exist`, apply migration 80:

```bash
sudo -u postgres psql -p 5434 -d zentra \
  -f /store/zentra/migrations/20250101000080_otlp_api_keys_columns.sql
```

### `404 Not Found` on `/api/agents/register`

The deployed binary is older than the agents route. Update on the build server, run `deploy/publish-release.sh`, then on the GCP VM:
```bash
cd /store/zentra && sudo git pull origin main && sudo systemctl restart zentra-api
```

### Browser shows `ERR_CONNECTION_TIMED_OUT` on port 5010

GCP firewall — verify rule `allow-zentra-5010` exists and the instance has the `http-server` network tag:
```bash
curl -sf -H "Metadata-Flavor: Google" \
  http://metadata.google.internal/computeMetadata/v1/instance/tags
```

### Middleware sub-menu only shows "Dashboard"

The sub-menu is built dynamically from `/api/middleware/dashboard`. Add a target (Step 9) and wait ~2 min for the next poll — Nginx, Apache, etc. will appear.

### Nginx config test fails — wrong `root` path

```bash
sudo sed -i 's|/opt/zentra|/store/zentra|g' /etc/nginx/sites-available/zentra
sudo /usr/sbin/nginx -t && sudo systemctl reload nginx
```

---

## Known Platform Notes

| Issue | Cause | Permanent fix |
|-------|-------|---------------|
| `libssl.so.1.1: not found` | Binary built on Debian 11 (OpenSSL 1.1), Debian 12 ships 3.x | `setup-debian12.sh` installs `libssl1.1` from bullseye |
| `postgresql.service` is a no-op | Debian 12 uses `postgresql@16-main.service` | Script uses correct unit name |
| `app.current_org_id` permission denied on migration 27 | RLS migration uses `SET` requiring elevated role | `ALTER ROLE zentra_app SUPERUSER;` after first run |
| `otlp_api_keys` missing columns on agent register | Original migration 65 was minimal; agent code expects extra columns | Migration `20250101000080_otlp_api_keys_columns.sql` adds them |
| `chown -R zentra` breaks pgdata ownership | Recursive chown overwrites postgres ownership | Script restores `chown -R postgres:postgres /store/zentra/pgdata` |
| nginx config has `/opt/zentra` paths | Bundled `nginx.conf` is the Docker path | Script auto-replaces via `sed` |
