# Zentra — Local Lab

Run 3 monitored Linux containers + 1 Zentra agent on your Windows machine.
The agent collects metrics from the containers and pushes them to your GCP Zentra server.

```
┌─────────────────────────────────────────────────────────┐
│ Windows Machine (Docker Desktop)                        │
│                                                         │
│  lab-linux-01  (172.30.0.10) — Rocky Linux + SSH + SNMP │
│  lab-nginx-01  (172.30.0.11) — Alpine + Nginx + SSH     │
│  lab-httpd-01  (172.30.0.12) — Alpine + Apache + SSH    │
│       ▲  ▲  ▲                                           │
│       └──┼──┘ SSH/SNMP collection                       │
│          │                                              │
│  zentra-agent  (172.30.0.2)                             │
│          │                                              │
└──────────│──────────────────────────────────────────────┘
           │ HTTP (port 5010)
           ▼
   GCP Zentra Server
```

---

## Prerequisites

- **Docker Desktop** installed and running on Windows
- **GCP Zentra server** deployed and accessible (port 5010 open)
- Files in this folder (all included):
  - `docker-compose.yml`
  - `setup-linux.sh`, `setup-nginx.sh`, `setup-httpd.sh`
  - `start-lab.ps1`
  - `zentra-agent-latest.tar.gz`

---

## Step 1 — Create an Agent API Key

The agent needs an API key (starts with `isok_`) to authenticate with the Zentra server.
You only need to do this **once per datacenter**.

### Option A: Zentra Web UI (recommended)

1. Open your Zentra server in a browser:
   ```
   http://<GCP_EXTERNAL_IP>:5010
   ```

2. Login with the default credentials:
   ```
   Email:    admin@Zentra.local
   Password: changeme
   ```

3. Navigate to **Fleet → Install Agent** (or click the **+** button on the Fleet page)

4. Fill in:
   - **Datacenter Name**: `dc-local-lab` (or any name you prefer)
   - **Region Label**: optional (e.g. `Local`)

5. Click **Register**

6. **Copy the API key** — it looks like:
   ```
   isok_a7b3f92e1d4c6x8m0k2p5n7q
   ```
   > ⚠️ **This key is shown only once.** Copy it immediately and store it securely.

7. Confirm you've copied it, then click **Next**

### Option B: Using curl (command line)

**Step 1 — Login and get a JWT token:**

```bash
curl -s http://<GCP_IP>:5010/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"admin@Zentra.local","password":"changeme"}'
```

Response:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIs...",
  "user": { ... }
}
```

**Step 2 — Register the agent:**

```bash
curl -s http://<GCP_IP>:5010/api/agents/register \
  -H "Authorization: Bearer <TOKEN_FROM_STEP_1>" \
  -H "Content-Type: application/json" \
  -d '{"datacenter_name":"dc-local-lab"}'
```

Response:
```json
{
  "agent_id": "01964f3a-...",
  "api_key": "isok_a7b3f92e1d4c6x8m0k2p5n7q",
  "warning": "Store this key securely. It will not be shown again."
}
```

Copy the `api_key` value — that's your `AGENT_API_KEY`.

### Option C: Using PowerShell (Windows)

```powershell
# Login
$resp = Invoke-RestMethod -Uri "http://<GCP_IP>:5010/api/auth/login" `
  -Method POST -ContentType "application/json" `
  -Body '{"email":"admin@Zentra.local","password":"changeme"}'
$token = $resp.token

# Register agent
$agent = Invoke-RestMethod -Uri "http://<GCP_IP>:5010/api/agents/register" `
  -Method POST -ContentType "application/json" `
  -Headers @{ Authorization = "Bearer $token" } `
  -Body '{"datacenter_name":"dc-local-lab"}'

Write-Host "API Key: $($agent.api_key)"
```

---

## Step 2 — Start the Lab

Open PowerShell, navigate to this folder, and run:

```powershell
cd docker\local-lab
.\start-lab.ps1 -ApiEndpoint "http://<GCP_IP>:5010" -AgentApiKey "isok_xxxxx"
```

The script will:
1. Check Docker is running
2. Auto-load the agent image from `zentra-agent-latest.tar.gz`
3. Start all 4 containers
4. Verify everything is running

If you omit the parameters, the script will prompt you interactively.

---

## Verify

### Check container status
```powershell
docker ps
```

### View agent logs
```powershell
docker logs -f zentra-agent
```

You should see:
```
[agent] Zentra API is reachable
[agent] Starting zentra-agent in remote mode...
```

### Check in Zentra UI
1. Open `http://<GCP_IP>:5010`
2. Go to **Fleet** — the agent should appear as `dc-local-lab`
3. Go to **Targets** — 3 hosts should be discovered
4. Check **Dashboards** for incoming metrics

---

## Commands Reference

| Action | Command |
|---|---|
| Start lab | `docker compose up -d` |
| Stop lab | `docker compose down` |
| Restart all | `docker compose restart` |
| Agent logs | `docker logs -f zentra-agent` |
| All logs | `docker compose logs -f` |
| Shell into Linux | `docker exec -it lab-linux-01 bash` |
| Shell into Nginx | `docker exec -it lab-nginx-01 sh` |
| Shell into Apache | `docker exec -it lab-httpd-01 sh` |
| Shell into Agent | `docker exec -it zentra-agent bash` |

---

## Containers

| Container | Base Image | IP | Services | Purpose |
|---|---|---|---|---|
| lab-linux-01 | rockylinux:8 | 172.30.0.10 | SSH (22), SNMP (161) | Plain Linux server |
| lab-nginx-01 | nginx:1.25-alpine | 172.30.0.11 | SSH (22), HTTP (80), Status (8080) | Nginx web server |
| lab-httpd-01 | httpd:2.4-alpine | 172.30.0.12 | SSH (22), HTTP (80), /server-status | Apache HTTP server |
| zentra-agent | zentra-agent:latest | 172.30.0.2 | — | Collects & pushes to GCP |

All containers use password `zentra123` for SSH root access.

---

## Troubleshooting

**Agent can't reach GCP server:**
- Ensure GCP firewall allows your public IP on port 5010
- Test: `curl http://<GCP_IP>:5010/api/health`

**Agent shows "invalid API key":**
- Ensure the key starts with `isok_`
- Generate a new key via the UI or API (see Step 1)

**Containers keep restarting:**
- Check logs: `docker compose logs <container-name>`
- Ensure Docker Desktop has enough memory (2 GB minimum)

**SNMP not responding:**
- Shell into the Linux container and check: `snmpwalk -v2c -c public localhost`
