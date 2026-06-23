# Zentra — GCP Cloud + On-Premises Agent Deployment Guide

> **Scope**: Deploy the Zentra monitoring app on a GCP Compute Engine VM.
> Run zentra-agent in your local datacenter to collect metrics via SSH/SNMP
> and push them to the cloud-hosted API.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                  GCP  (e.g. us-central1)                 │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │        GCE VM  (e2-standard-2, 50 GB disk)        │  │
│  │                                                    │  │
│  │   zentra:latest container                          │  │
│  │     Nginx (:5010) → API (:5006)                    │  │
│  │     PostgreSQL + TimescaleDB (:5434)               │  │
│  │     React SPA frontend                             │  │
│  └────────────────────────────────────────────────────┘  │
│         ▲  port 5010 exposed via GCP firewall            │
└─────────┼────────────────────────────────────────────────┘
          │
     Internet / VPN
          │
┌─────────┼────────────────────────────────────────────────┐
│         ▼          On-Premises Datacenter                │
│                                                          │
│  ┌────────────────────────────────────────────────────┐  │
│  │  zentra-agent  (Docker or bare-metal binary)       │  │
│  │  Pushes metrics to https://<GCP-IP>:5010           │  │
│  └───────┬────────────────────────────────────────────┘  │
│          │  SSH (22) / SNMP (161)                        │
│  ┌───────┴────────────────────────────────────────────┐  │
│  │  servers  switches  firewalls  storage  databases  │  │
│  └────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

---

## Part 1 — Zentra App on GCP

### 1.1  Create a GCE VM

```bash
gcloud compute instances create zentra-server \
  --zone=us-central1-a \
  --machine-type=e2-standard-2 \
  --boot-disk-size=50GB \
  --boot-disk-type=pd-balanced \
  --image-family=rocky-linux-8 \
  --image-project=rocky-linux-cloud \
  --tags=zentra-server
```

Open port 5010 to your IP only:

```bash
gcloud compute firewall-rules create zentra-web \
  --allow=tcp:5010 \
  --source-ranges="YOUR_OFFICE_PUBLIC_IP/32" \
  --target-tags=zentra-server
```

### 1.2  Install Docker on the VM

```bash
gcloud compute ssh zentra-server --zone=us-central1-a
```

Inside the VM:

```bash
sudo dnf install -y dnf-utils
sudo dnf config-manager \
  --add-repo https://download.docker.com/linux/centos/docker-ce.repo
sudo dnf install -y docker-ce docker-ce-cli containerd.io \
  docker-compose-plugin
sudo systemctl enable --now docker
sudo usermod -aG docker "$USER"
exit          # re-login to pick up docker group
```

### 1.3  Upload & Load the Image

From your local workstation:

```bash
gcloud compute scp \
  docker/zentra-latest.tar.gz \
  zentra-server:~ \
  --zone=us-central1-a
```

On the VM:

```bash
docker load -i ~/zentra-latest.tar.gz
```

### 1.4  Create the Environment File

```bash
sudo mkdir -p /opt/zentra

JWT=$(openssl rand -hex 32)
PGPW=$(openssl rand -hex 16)
MKEY=$(openssl rand -hex 32)

sudo tee /opt/zentra/.env >/dev/null <<EOF
POSTGRES_USER=postgres
POSTGRES_PASSWORD=${PGPW}
POSTGRES_DB=zentra
POSTGRES_PORT=5434

ZENTRA_JWT_SECRET=${JWT}
ZENTRA_MASTER_KEY=${MKEY}
ZENTRA_ALLOW_INSECURE_JWT=0
ZENTRA_API_PORT=5006
ZENTRA_ALLOW_ORIGINS=*
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
RUST_LOG=info
EOF

sudo chmod 600 /opt/zentra/.env
```

### 1.5  Start the Container

```bash
docker run -d \
  --name zentra \
  -p 5010:5010 \
  --restart unless-stopped \
  --env-file /opt/zentra/.env \
  -v zentra-data:/var/lib/postgresql/data \
  -v /opt/zentra/logs:/opt/zentra/logs \
  zentra:latest
```

### 1.6  Verify

```bash
# Wait ~30 s for init + migrations
curl -s http://localhost:5010/api/health
docker logs zentra --tail 20
```

Open `http://<GCE_EXTERNAL_IP>:5010` → login `admin@Zentra.local` / `changeme`.

**Change the password immediately.**

### 1.7  (Optional) HTTPS

Put a reverse proxy in front of Zentra. Example with Caddy:

```
zentra.yourdomain.com {
    reverse_proxy localhost:5010
}
```

Then allow tcp:443 in the GCP firewall and point DNS.

---

## Part 2 — Agent On-Premises

### Option A — Docker Container (Recommended)

#### 2A.1  Load the Image

Copy `zentra-agent-latest.tar.gz` to a Linux host in your datacenter:

```bash
scp docker/zentra-agent-latest.tar.gz user@dc-host:~/
ssh user@dc-host "docker load -i ~/zentra-agent-latest.tar.gz"
```

#### 2A.2  Start the Agent

```bash
GCP_URL="http://<GCE_EXTERNAL_IP>:5010"

docker run -d \
  --name zentra-agent \
  --restart unless-stopped \
  --network host \
  -e API_ENDPOINT="${GCP_URL}" \
  -e AGENT_MODE="remote" \
  -e DC_NAME="dc-onprem-01" \
  -e RUST_LOG="info" \
  -e SIM_NODES="srv-01:10.0.1.10 srv-02:10.0.1.11 sw-core:10.0.1.1" \
  -e SSH_PASSWORD="your_ssh_password" \
  zentra-agent:latest
```

| Variable | Purpose | Example |
|---|---|---|
| `API_ENDPOINT` | Zentra API URL on GCP | `https://zentra.example.com` |
| `DC_NAME` | Friendly name shown in UI | `dc-london-01` |
| `SIM_NODES` | `hostname:ip` pairs (space-separated) | `srv-01:10.0.1.10 sw-01:10.0.1.1` |
| `SSH_PASSWORD` | Used once for initial key distribution | — |
| `AGENT_MODE` | Always `remote` for on-prem | `remote` |

`--network host` lets the agent reach all local IPs without port mapping.

#### 2A.3  Verify

```bash
docker logs -f zentra-agent
# Expected:
#   [agent] Zentra API is reachable (after 3s)
#   [agent] Starting zentra-agent in remote mode...
```

### Option B — Bare-Metal Binary

#### 2B.1  Copy the Binary

```bash
scp docker/simulation/bin/zentra-agent user@dc-host:/usr/local/bin/
ssh user@dc-host "chmod +x /usr/local/bin/zentra-agent"
```

#### 2B.2  Create a systemd Unit

```ini
# /etc/systemd/system/zentra-agent.service
[Unit]
Description=Zentra Agent
After=network-online.target

[Service]
Type=simple
ExecStart=/usr/local/bin/zentra-agent
Restart=always
RestartSec=10
Environment=AGENT_MODE=remote
Environment=API_ENDPOINT=http://<GCE_EXTERNAL_IP>:5010
Environment=DC_NAME=dc-onprem-01
Environment=RUST_LOG=info
Environment=BUFFER_DIR=/opt/zentra-agent/buffer
WorkingDirectory=/opt/zentra-agent

[Install]
WantedBy=multi-user.target
```

```bash
sudo mkdir -p /opt/zentra-agent/{buffer,logs}
sudo systemctl daemon-reload
sudo systemctl enable --now zentra-agent
sudo journalctl -u zentra-agent -f
```

#### 2B.3  SSH Keys (Bare Metal Only)

The Docker entrypoint does this automatically. For bare metal, distribute keys manually:

```bash
ssh-keygen -t rsa -b 4096 -f /root/.ssh/id_rsa -N ""

for HOST in 10.0.1.10 10.0.1.11 10.0.1.1 10.0.1.254; do
  ssh-copy-id -i /root/.ssh/id_rsa.pub "root@${HOST}"
done
```

---

## Part 3 — Firewall Rules

### GCP Inbound

| Rule | Port | Source | Purpose |
|---|---|---|---|
| `zentra-web` | tcp/5010 | Office public IP | Web UI |
| `zentra-agent-api` | tcp/5010 | Agent host public IP | Agent → API |

```bash
gcloud compute firewall-rules create zentra-agent-inbound \
  --allow=tcp:5010 \
  --source-ranges="AGENT_PUBLIC_IP/32" \
  --target-tags=zentra-server
```

### On-Premises Outbound (from Agent Host)

| Direction | Protocol | Port | Destination |
|---|---|---|---|
| Outbound | TCP | 5010 or 443 | GCE external IP |
| Local | TCP | 22 | Monitored servers |
| Local | UDP | 161 | Monitored SNMP devices |

---

## Part 4 — Adding Targets in Zentra UI

1. Open `http://<GCE_IP>:5010` → **Targets** → **Add Target**
2. Enter the IP reachable from the agent (e.g. `10.0.1.10`)
3. Select protocol: SSH for servers, SNMP for network gear
4. Select the agent name matching `DC_NAME`
5. Metrics begin flowing automatically

---

## Part 5 — Maintenance

### Update Zentra App

```bash
gcloud compute scp docker/zentra-latest.tar.gz zentra-server:~ \
  --zone=us-central1-a
gcloud compute ssh zentra-server --zone=us-central1-a

docker stop zentra && docker rm zentra
docker rmi zentra:latest
docker load -i ~/zentra-latest.tar.gz
docker run -d --name zentra -p 5010:5010 --restart unless-stopped \
  --env-file /opt/zentra/.env \
  -v zentra-data:/var/lib/postgresql/data \
  -v /opt/zentra/logs:/opt/zentra/logs zentra:latest
```

Data is preserved in the `zentra-data` volume.

### Update Agent

```bash
# Docker
docker stop zentra-agent && docker rm zentra-agent
docker rmi zentra-agent:latest
docker load -i zentra-agent-latest.tar.gz
# re-run the docker run command from 2A.2

# Bare metal
sudo systemctl stop zentra-agent
sudo cp zentra-agent /usr/local/bin/
sudo systemctl start zentra-agent
```

### Backup

```bash
docker run --rm \
  -v zentra-data:/data \
  -v "$(pwd)":/backup \
  alpine tar czf "/backup/zentra-backup-$(date +%Y%m%d).tar.gz" \
  -C /data .
```

---

## Quick-Start Checklist

- [ ] GCE VM created with Docker
- [ ] `zentra-latest.tar.gz` uploaded and loaded
- [ ] `.env` created with generated secrets
- [ ] Container running, `curl localhost:5010/api/health` returns OK
- [ ] GCP firewall allows 5010 from office IP + agent IP
- [ ] Agent image/binary copied to on-prem host
- [ ] Agent running with correct `API_ENDPOINT`
- [ ] Agent logs show "Zentra API is reachable"
- [ ] Targets added in UI, metrics flowing
- [ ] Default admin password changed
