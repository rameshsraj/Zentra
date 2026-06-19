# Zentra Agent - Production Deployment Guide

This guide covers deploying the `zentra-agent` on monitored servers in production environments.

> **IMPORTANT:** The agent should run directly on the host OS of the server being monitored, NOT in containers. Container-based deployment is only for demo/simulation purposes.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│  Zentra Central Server                                       │
│  - zentra-api (backend) + frontend                           │
│  - PostgreSQL + TimescaleDB                                  │
│  - API endpoint: https://zentra.example.com                  │
└─────────────────────────────────────────────────────────────┘
                            ▲
                            │ HTTPS (API endpoint)
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
┌───────▼────────┐  ┌──────▼────────┐  ┌───────▼────────┐
│ Production     │  │ Production     │  │ Production     │
│ Server A       │  │ Server B       │  │ Server C       │
│                 │  │                 │  │                 │
│ zentra-agent   │  │ zentra-agent   │  │ zentra-agent   │
│ (host OS)      │  │ (host OS)      │  │ (host OS)      │
│                 │  │                 │  │                 │
│ Collects:      │  │ Collects:      │  │ Collects:      │
│ - CPU, Memory  │  │ - CPU, Memory  │  │ - CPU, Memory  │
│ - Disk, Net    │  │ - Disk, Net    │  │ - Disk, Net    │
│ - SSH plugins  │  │ - SSH plugins  │  │ - SSH plugins  │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

---

## Prerequisites

### On the monitored server:
- Linux (Debian/Ubuntu/RHEL/Rocky/CentOS) or Windows
- Network connectivity to Zentra server (HTTPS port 5006 or custom)
- Root or sudo access for installation

### On the Zentra server:
- zentra-api running and accessible
- Agent API key generated (from Zentra UI → Settings → API Keys)

---

## Installation Steps

### Step 1: Generate Agent API Key

1. Log in to Zentra UI
2. Navigate to **Settings → API Keys**
3. Create a new API key with type `Agent`
4. Copy the generated key (e.g., `za_xxxx...`)

### Step 2: Download Agent Binary

#### Linux (x86_64)

```bash
# Download the latest agent binary
wget https://zentra.example.com/downloads/zentra-agent-linux-amd64 -O zentra-agent

# Or use curl
curl -L https://zentra.example.com/downloads/zentra-agent-linux-amd64 -o zentra-agent
```

#### Linux (ARM64)

```bash
wget https://zentra.example.com/downloads/zentra-agent-linux-arm64 -O zentra-agent
```

#### Windows

Download from: `https://zentra.example.com/downloads/zentra-agent-windows-amd64.exe`

### Step 3: Install Agent

#### Linux (Systemd Service)

```bash
# Move binary to system location
sudo mv zentra-agent /usr/local/bin/zentra-agent
sudo chmod +x /usr/local/bin/zentra-agent

# Create agent user
sudo useradd -r -s /bin/false zentra-agent

# Create directories
sudo mkdir -p /opt/zentra-agent/buffer /opt/zentra-agent/logs
sudo chown -R zentra-agent:zentra-agent /opt/zentra-agent

# Create environment file
sudo tee /etc/zentra-agent/env > /dev/null <<EOF
AGENT_MODE=remote
API_ENDPOINT=https://zentra.example.com
AGENT_API_KEY=your-agent-api-key-here
DC_NAME=$(hostname)
BUFFER_DIR=/opt/zentra-agent/buffer
ZENTRA_LOG_LEVEL=info
EOF

# Secure the environment file
sudo chmod 600 /etc/zentra-agent/env

# Create systemd service
sudo tee /etc/systemd/system/zentra-agent.service > /dev/null <<EOF
[Unit]
Description=Zentra Monitoring Agent
After=network.target

[Service]
Type=simple
User=zentra-agent
Group=zentra-agent
EnvironmentFile=/etc/zentra-agent/env
ExecStart=/usr/local/bin/zentra-agent
Restart=always
RestartSec=10

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/zentra-agent

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd and enable service
sudo systemctl daemon-reload
sudo systemctl enable zentra-agent
sudo systemctl start zentra-agent

# Check status
sudo systemctl status zentra-agent
```

#### Linux (Manual/Testing)

```bash
# Set environment variables
export AGENT_MODE=remote
export API_ENDPOINT=https://zentra.example.com
export AGENT_API_KEY=your-agent-api-key-here
export DC_NAME=$(hostname)
export BUFFER_DIR=/tmp/zentra-buffer
export ZENTRA_LOG_LEVEL=debug

# Run agent
./zentra-agent
```

#### Windows (Service)

```powershell
# Download and extract agent binary
# Create service directory
mkdir C:\zentra-agent

# Create environment file
@"
AGENT_MODE=remote
API_ENDPOINT=https://zentra.example.com
AGENT_API_KEY=your-agent-api-key-here
DC_NAME=$env:COMPUTERNAME
BUFFER_DIR=C:\zentra-agent\buffer
ZENTRA_LOG_LEVEL=info
"@ | Out-File -FilePath C:\zentra-agent\.env -Encoding ASCII

# Install using NSSM (Non-Sucking Service Manager)
# Download NSSM from https://nssm.cc/download
nssm install ZentraAgent C:\zentra-agent\zentra-agent.exe
nssm set ZentraAgent AppEnvironmentFile C:\zentra-agent\.env
nssm start ZentraAgent
```

### Step 4: Verify Agent Connection

1. In Zentra UI, navigate to **Targets**
2. You should see a new agent target with:
   - Name: `agent-<hostname>` or your configured `DC_NAME`
   - Status: `Online` (green)
   - Last heartbeat: Recent timestamp

3. Check agent logs:

```bash
# Linux (systemd)
sudo journalctl -u zentra-agent -f

# Or direct log file
sudo tail -f /opt/zentra-agent/logs/agent.log
```

---

## Configuration Options

| Environment Variable | Required | Description | Example |
|---------------------|----------|-------------|---------|
| `AGENT_MODE` | Yes | Agent mode: `remote` (recommended) or `local` | `remote` |
| `API_ENDPOINT` | Yes (remote) | Zentra server API URL | `https://zentra.example.com` |
| `AGENT_API_KEY` | Yes (remote) | Agent API key from Zentra UI | `za_xxxx...` |
| `DC_NAME` | Yes | Unique identifier for this agent/datacenter | `us-east-1-server-1` |
| `BUFFER_DIR` | No | Directory for offline buffering | `/opt/zentra-agent/buffer` |
| `ZENTRA_LOG_LEVEL` | No | Log level: `debug`, `info`, `warn`, `error` | `info` |
| `BUFFER_MAX_SIZE_MB` | No | Max offline buffer size in MB | `1000` |

---

## Local Mode (Alternative)

For environments where the agent cannot reach the Zentra server over HTTPS, use **LOCAL mode**:

```bash
# Environment file for local mode
AGENT_MODE=local
DATABASE_URL=postgresql://postgres:password@zentra-db:5432/zentra
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
```

**Note:** Local mode requires direct database access and is less secure than remote mode.

---

## Troubleshooting

### Agent not connecting

1. **Check network connectivity:**
   ```bash
   curl -v https://zentra.example.com/api/health
   ```

2. **Verify API key is valid in Zentra UI**

3. **Check agent logs for errors:**
   ```bash
   sudo journalctl -u zentra-agent -n 50
   ```

### Agent collecting no metrics

1. **Verify targets are assigned to this agent** in Zentra UI

2. **Check agent has permissions to run collection plugins:**
   ```bash
   # Test SSH connectivity
   sudo -u zentra-agent ssh -o StrictHostKeyChecking=no localhost echo "test"
   ```

3. **Enable debug logging** and check for plugin errors

### High CPU/Memory usage

1. **Reduce collection interval** in Zentra UI
2. **Disable unnecessary plugins** for this agent
3. **Check for stuck collection loops** in logs

---

## Security Best Practices

1. **Use HTTPS** for `API_ENDPOINT` in production
2. **Rotate agent API keys** periodically
3. **Run agent as non-root user** (default)
4. **Restrict network access** to Zentra server only
5. **Monitor agent logs** for suspicious activity
6. **Keep agent binary updated** to latest version

---

## Uninstalling Agent

#### Linux

```bash
# Stop and disable service
sudo systemctl stop zentra-agent
sudo systemctl disable zentra-agent

# Remove service file
sudo rm /etc/systemd/system/zentra-agent.service
sudo systemctl daemon-reload

# Remove binary and directories
sudo rm /usr/local/bin/zentra-agent
sudo rm -rf /opt/zentra-agent
sudo rm -rf /etc/zentra-agent

# Remove user (optional)
sudo userdel zentra-agent
```

#### Windows

```powershell
nssm stop ZentraAgent
nssm remove ZentraAgent
Remove-Item -Recurse -Force C:\zentra-agent
```

---

## Monitoring the Agent Itself

The agent reports its own health metrics to Zentra:
- CPU usage
- Memory usage
- Network I/O
- Disk usage
- Collection latency

These appear in Zentra UI under the agent's target.

---

## Scaling Considerations

- **Single agent per server** is recommended
- **Multiple agents per server** can be used for different datacenters/partitions
- **Load balancing:** Agents automatically balance targets via leader election (local mode)
- **Offline buffering:** Agents buffer data locally if Zentra server is down (up to configured max size)

---

## Next Steps

1. **Create targets** in Zentra UI for the agent to monitor
2. **Configure collection plugins** (SSH, SNMP, etc.)
3. **Set up alerting rules** based on collected metrics
4. **Create dashboards** for visualization
