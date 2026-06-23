# Zentra — GCP Deployment (Binary-Only from GitHub)

> No Docker. No build tools on the server.
> Pre-built binaries are pushed to private GitHub repos from the build server,
> then pulled onto GCP and on-prem hosts.

## Architecture

```
Build Server (10.88.11.162)
  │  cargo build → zentra-server, zentra-agent
  │  npm build   → frontend/dist/
  │
  ├──► rameshsraj/zentra-release  (private GitHub repo)
  │       bin/zentra-server, frontend/dist/, migrations/, deploy/
  │
  └──► rameshsraj/zentra-agent    (private GitHub repo)
          bin/zentra-agent, deploy/

GCP Server                        On-Premises Host
  git clone zentra-release          git clone zentra-agent
  → /store/zentra                   → /store/zentra-agent
  sudo bash deploy/setup-debian12.sh  sudo bash deploy/setup-agent.sh
```

---

## Part 1 — Build & Publish (on Build Server)

Run on the **Linux build server** (`10.88.11.162`):

```bash
cd /store/Zentra

# Build everything
bash docker/build-image.sh

# Publish binaries to GitHub repos
export GITHUB_TOKEN="ghp_xxxxxxxxxxxx"
bash deploy/publish-release.sh
```

This pushes artifacts to:
- `rameshsraj/zentra-release` — server binary + frontend + migrations + setup script
- `rameshsraj/zentra-agent` — agent binary + setup script

**First-time repo setup**: Create both repos as **private** on GitHub before the first publish.

---

## Part 2 — Deploy Zentra App on GCP

### 2.1 Create GCE VM

```bash
gcloud compute instances create zentra-server \
  --zone=us-central1-a \
  --machine-type=e2-standard-2 \
  --boot-disk-size=50GB \
  --boot-disk-type=pd-balanced \
  --image-family=debian-12 \
  --image-project=debian-cloud \
  --tags=zentra-server
```

### 2.2 Open Firewall

```bash
gcloud compute firewall-rules create zentra-web \
  --allow=tcp:5010 \
  --source-ranges="YOUR_OFFICE_IP/32" \
  --target-tags=zentra-server
```

### 2.3 SSH In, Clone, and Run Setup

```bash
gcloud compute ssh zentra-server --zone=us-central1-a
```

On the VM:

```bash
# Install git (if not present)
sudo apt-get update && sudo apt-get install -y git

# Clone the release repo (requires GitHub PAT for private repo)
sudo git clone https://<GITHUB_TOKEN>@github.com/rameshsraj/zentra-release.git /store/zentra

# Run setup — installs PostgreSQL, TimescaleDB, Nginx, configures everything
cd /store/zentra
sudo bash deploy/setup-debian12.sh
```

**That's it.** The setup script:
1. Installs PostgreSQL 16 + TimescaleDB + Nginx (no build tools)
2. Configures the database (port 5434, `zentra` DB, `zentra_app` user, extensions)
3. Generates secrets (JWT, master key, PG password) automatically
4. Configures Nginx reverse proxy (`:5010` → API `:5006`)
5. Creates systemd service for `zentra-api`
6. Starts everything and runs health checks

Migrations are **embedded in the binary** — they run automatically on first startup.

### 2.4 Verify

```bash
curl http://localhost:5010/api/health
```

Open `http://<GCE_EXTERNAL_IP>:5010` — Login: `admin@Zentra.local` / `changeme`

---

## Part 3 — Deploy Agent On-Premises

### 3.1 Clone and Run Setup

On your datacenter Linux host:

```bash
sudo git clone https://<GITHUB_TOKEN>@github.com/rameshsraj/zentra-agent.git /store/zentra-agent
cd /store/zentra-agent
sudo bash deploy/setup-agent.sh
```

The script prompts for:
- **API endpoint** — e.g. `http://34.68.222.94:5010`
- **Datacenter name** — e.g. `dc-london-01`
- **Agent API key** — starts with `isok_` (generate in Zentra UI)

It then:
1. Installs SSH client + SNMP tools
2. Creates service user and `.env`
3. Generates SSH key pair (prints the public key to distribute)
4. Creates systemd service and starts it
5. Tests connectivity to the Zentra API

### 3.2 Distribute SSH Keys

The setup script prints the SSH public key. Copy it to each monitored host:

```bash
ssh-copy-id -i /store/zentra-agent/.ssh/id_rsa.pub root@<TARGET_IP>
```

### 3.3 Allow Agent Through GCP Firewall

```bash
gcloud compute firewall-rules create zentra-agent-inbound \
  --allow=tcp:5010 \
  --source-ranges="AGENT_PUBLIC_IP/32" \
  --target-tags=zentra-server
```

---

## Part 4 — Updates

### Update Zentra App

```bash
# On the build server — build and publish
bash docker/build-image.sh
export GITHUB_TOKEN="ghp_xxxxxxxxxxxx"
bash deploy/publish-release.sh

# On the GCP VM — pull and restart
cd /store/zentra
sudo git pull
sudo systemctl restart zentra-api
```

### Update Agent

```bash
# On the build server — publish-release.sh covers this

# On the on-prem host — pull and restart
cd /store/zentra-agent
sudo git pull
sudo systemctl restart zentra-agent
```

### Backup PostgreSQL

```bash
sudo -u postgres pg_dump -p 5434 zentra | gzip > zentra-backup-$(date +%Y%m%d).sql.gz
```

---

## Firewall Summary

### GCP Inbound

| Port | Source | Purpose |
|---|---|---|
| tcp/5010 | Office IP | Web UI |
| tcp/5010 | Agent public IP | Agent → API |

### On-Premises Outbound (from Agent)

| Port | Destination | Purpose |
|---|---|---|
| tcp/5010 | GCE external IP | Agent → API |
| tcp/22 | Local servers | SSH collection |
| udp/161 | Local devices | SNMP collection |

---

## File Layout

### /store/zentra (GCP Server)

```
/store/zentra/
├── bin/zentra-server          # API binary
├── frontend/dist/             # React SPA
├── migrations/                # SQL (reference only — embedded in binary)
├── deploy/
│   ├── setup-debian12.sh      # Setup script
│   └── nginx.conf             # Nginx config
├── logs/                      # Created by setup
├── .env                       # Created by setup (secrets)
└── VERSION
```

### /store/zentra-agent (On-Prem Host)

```
/store/zentra-agent/
├── bin/zentra-agent            # Agent binary
├── deploy/
│   ├── setup-agent.sh          # Setup script
│   └── agent-entrypoint.sh     # Docker entrypoint (optional)
├── logs/                       # Created by setup
├── buffer/                     # Offline metric buffer
├── .ssh/                       # Generated SSH keys
├── .env                        # Created by setup
└── VERSION
```

---

## Quick Checklist

- [ ] Build server: `build-image.sh` + `publish-release.sh` completed
- [ ] GitHub: `rameshsraj/zentra-release` repo exists (private)
- [ ] GitHub: `rameshsraj/zentra-agent` repo exists (private)
- [ ] GCE VM created, firewall open on 5010
- [ ] `/store/zentra` cloned, `setup-debian12.sh` ran successfully
- [ ] `curl localhost:5010/api/health` returns OK
- [ ] Default admin password changed
- [ ] On-prem host: `/store/zentra-agent` cloned, `setup-agent.sh` ran
- [ ] SSH keys distributed to target hosts
- [ ] Agent logs show "API reachable"
- [ ] Targets added in Zentra UI, metrics flowing
