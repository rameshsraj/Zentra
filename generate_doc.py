#!/usr/bin/env python3
"""Generate Zentra SaaS Architecture & Git Workflow Word Document."""

from docx import Document
from docx.shared import Inches, Pt, Cm, RGBColor
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.oxml.ns import qn
import datetime

doc = Document()

# ── Styles ──
style = doc.styles['Normal']
style.font.name = 'Calibri'
style.font.size = Pt(11)
style.paragraph_format.space_after = Pt(6)

for level in range(1, 4):
    hs = doc.styles[f'Heading {level}']
    hs.font.color.rgb = RGBColor(0x1A, 0x56, 0xDB)

# ── Helper Functions ──
def add_code_block(text):
    p = doc.add_paragraph()
    p.paragraph_format.space_before = Pt(6)
    p.paragraph_format.space_after = Pt(6)
    p.paragraph_format.left_indent = Cm(1)
    run = p.add_run(text)
    run.font.name = 'Consolas'
    run.font.size = Pt(9.5)
    run.font.color.rgb = RGBColor(0x1E, 0x1E, 0x1E)
    # Light grey background via shading
    shading = run._element.get_or_add_tcPr = None  # skip complex shading
    return p

def add_table(headers, rows):
    table = doc.add_table(rows=1 + len(rows), cols=len(headers))
    table.style = 'Light Grid Accent 1'
    table.alignment = WD_TABLE_ALIGNMENT.LEFT
    for i, h in enumerate(headers):
        cell = table.rows[0].cells[i]
        cell.text = h
        for p in cell.paragraphs:
            for r in p.runs:
                r.bold = True
                r.font.size = Pt(10)
    for ri, row in enumerate(rows):
        for ci, val in enumerate(row):
            cell = table.rows[ri + 1].cells[ci]
            cell.text = str(val)
            for p in cell.paragraphs:
                for r in p.runs:
                    r.font.size = Pt(10)
    return table

def add_note(text):
    p = doc.add_paragraph()
    run = p.add_run("⚠ " + text)
    run.font.color.rgb = RGBColor(0xCC, 0x66, 0x00)
    run.font.size = Pt(10)
    run.italic = True

# ═══════════════════════════════════════════════════════════════════════
# TITLE PAGE
# ═══════════════════════════════════════════════════════════════════════
doc.add_paragraph()
doc.add_paragraph()
title = doc.add_paragraph()
title.alignment = WD_ALIGN_PARAGRAPH.CENTER
run = title.add_run("ZENTRA")
run.font.size = Pt(36)
run.font.color.rgb = RGBColor(0x1A, 0x56, 0xDB)
run.bold = True

subtitle = doc.add_paragraph()
subtitle.alignment = WD_ALIGN_PARAGRAPH.CENTER
run = subtitle.add_run("SaaS Architecture & Git Workflow Guide")
run.font.size = Pt(18)
run.font.color.rgb = RGBColor(0x44, 0x44, 0x44)

doc.add_paragraph()
meta = doc.add_paragraph()
meta.alignment = WD_ALIGN_PARAGRAPH.CENTER
meta.add_run(f"Version 2.0 Draft\n{datetime.date.today().strftime('%B %d, %Y')}\nConfidential").font.size = Pt(11)

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# TABLE OF CONTENTS (manual)
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("Table of Contents", level=1)
toc_items = [
    "1. Git Branch Strategy",
    "   1.1 Creating the SaaS Branch",
    "   1.2 GitHub Desktop Workflow",
    "   1.3 Switching Between Branches",
    "   1.4 Branch Protection",
    "   1.5 Hotfix Workflow",
    "   1.6 Quick Reference — All Git Commands",
    "2. Current Architecture (v1.0 — Single Container)",
    "3. SaaS Architecture (v2.0 — Agent + Cloud)",
    "   3.1 Architecture Overview",
    "   3.2 Data Flow Patterns",
    "   3.3 Pattern 1: Agent Polls Targets",
    "   3.4 Pattern 2: Inbound Listeners",
    "   3.5 Pattern 3: Local Security Analysis",
    "   3.6 Agent as Remote Executor",
    "4. Target Management — How the Agent Knows What to Monitor",
    "   4.1 Manual Target Registration",
    "   4.2 Auto-Discovery (Crawl)",
    "   4.3 Service Discovery Providers",
    "   4.4 Plugin Routing",
    "5. Deployment Strategy",
    "   5.1 v1.0 — Single Container Deployment",
    "   5.2 v2.0 — SaaS Dual-Image Deployment",
    "6. Code Changes Required for SaaS",
    "   6.1 Agent Transport Swap",
    "   6.2 Functions That Must Move to Agent",
    "   6.3 Effort Summary",
]
for item in toc_items:
    p = doc.add_paragraph(item)
    p.paragraph_format.space_after = Pt(2)
    if item.startswith("   "):
        p.paragraph_format.left_indent = Cm(1.5)

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# SECTION 1: GIT BRANCH STRATEGY
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("1. Git Branch Strategy", level=1)

doc.add_paragraph(
    "Zentra maintains two independent branches. The main branch contains the stable v1.0 "
    "single-container release. The zentra-saas branch is a full copy of the codebase where "
    "all SaaS-related changes are developed independently. These branches never merge."
)

# 1.1
doc.add_heading("1.1 Creating the SaaS Branch", level=2)

doc.add_paragraph("Terminal Commands:")
add_code_block(
    "# Ensure main is clean and up to date\n"
    "git checkout main\n"
    "git pull origin main\n"
    "\n"
    "# Tag the current release\n"
    "git tag -a v1.0.0 -m \"Single-container release — pre-SaaS\"\n"
    "git push origin v1.0.0\n"
    "\n"
    "# Create the SaaS branch (full copy of current code)\n"
    "git checkout -b zentra-saas\n"
    "\n"
    "# Push it to GitHub\n"
    "git push -u origin zentra-saas"
)

doc.add_paragraph("GitHub Desktop Steps:")
steps = [
    "Open GitHub Desktop → ensure Current Branch shows main",
    "Click Current Branch dropdown → New Branch",
    "Name: zentra-saas",
    "Confirm 'Create branch based on: main'",
    "Click Create Branch",
    "Click Publish branch (blue button at top)",
]
for i, s in enumerate(steps, 1):
    doc.add_paragraph(f"{i}. {s}", style='List Number')

doc.add_paragraph("Result:")
add_code_block(
    "main          ← v1.0.0 — frozen, never touched\n"
    "zentra-saas   ← full copy, all SaaS work happens here"
)

# 1.2
doc.add_heading("1.2 GitHub Desktop — Daily Workflow", level=2)

add_table(
    ["Step", "Action"],
    [
        ["1", "GitHub Desktop → Current Branch → zentra-saas"],
        ["2", "Make changes in VS Code / Devin"],
        ["3", "GitHub Desktop shows changed files on the left"],
        ["4", "Type commit message → Commit to zentra-saas"],
        ["5", "Click Push origin at the top"],
    ]
)

doc.add_paragraph()
add_note(
    "VS Code, Devin, and GitHub Desktop all see the same folder. "
    "Switching branches in GitHub Desktop immediately changes the files "
    "that VS Code and Devin see. There is no separate folder per branch."
)

# 1.3
doc.add_heading("1.3 Switching Between Branches", level=2)

doc.add_paragraph("To temporarily work on main (e.g., fix a production bug):")
add_code_block(
    "# GitHub Desktop: Current Branch → main\n"
    "# Make your fix, commit, push\n"
    "# GitHub Desktop: Current Branch → zentra-saas  (switch back)"
)

add_note(
    "Always commit or stash uncommitted changes before switching branches. "
    "GitHub Desktop will warn you if you have unsaved changes."
)

# 1.4
doc.add_heading("1.4 Branch Protection (Optional)", level=2)

doc.add_paragraph("To prevent accidental changes to main:")
steps2 = [
    "Go to GitHub.com → your repo → Settings → Branches",
    "Click Add rule",
    "Branch name pattern: main",
    "Check Lock branch",
    "Save changes",
]
for i, s in enumerate(steps2, 1):
    doc.add_paragraph(f"{i}. {s}", style='List Number')

doc.add_paragraph("This makes main read-only. Nobody can push to it.")

# 1.5
doc.add_heading("1.5 Hotfix Workflow", level=2)

doc.add_paragraph(
    "If you need to fix a critical bug in the v1.0 production release:"
)
add_code_block(
    "# Branch off main\n"
    "git checkout main\n"
    "git checkout -b hotfix/fix-description\n"
    "\n"
    "# Fix the bug, commit\n"
    "git add -A && git commit -m \"Fix: description\"\n"
    "git push origin hotfix/fix-description\n"
    "\n"
    "# Merge to main via Pull Request on GitHub\n"
    "# Then optionally bring the fix into zentra-saas:\n"
    "git checkout zentra-saas\n"
    "git cherry-pick <commit-hash>"
)

# 1.6
doc.add_heading("1.6 Quick Reference — All Git Commands", level=2)

add_table(
    ["Task", "Terminal Command", "GitHub Desktop"],
    [
        ["Create SaaS branch", "git checkout -b zentra-saas", "Current Branch → New Branch → zentra-saas"],
        ["Push branch to GitHub", "git push -u origin zentra-saas", "Click Publish branch"],
        ["Switch to SaaS branch", "git checkout zentra-saas", "Current Branch → zentra-saas"],
        ["Switch to main", "git checkout main", "Current Branch → main"],
        ["Commit changes", "git add -A && git commit -m \"msg\"", "Type message → Commit to zentra-saas"],
        ["Push changes", "git push", "Click Push origin"],
        ["Tag a release", "git tag -a v1.0.0 -m \"msg\" && git push origin v1.0.0", "N/A (use terminal)"],
        ["Check current branch", "git branch --show-current", "Bottom-left of VS Code shows branch"],
        ["Pull latest", "git pull", "Click Fetch origin → Pull origin"],
        ["Stash changes", "git stash", "N/A (use terminal)"],
    ]
)

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# SECTION 2: CURRENT ARCHITECTURE
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("2. Current Architecture (v1.0 — Single Container)", level=1)

doc.add_paragraph(
    "Zentra v1.0 runs as a single Docker container deployed inside the customer's data center. "
    "All components run within one container managed by Supervisor:"
)

add_table(
    ["Component", "Role"],
    [
        ["PostgreSQL 16 + TimescaleDB", "Time-series metrics storage"],
        ["zentra-api (Rust/Axum)", "REST + gRPC API backend"],
        ["zentra-agent", "Metric collection orchestrator"],
        ["React Frontend", "Dashboard UI"],
        ["Nginx", "Reverse proxy, serves frontend"],
        ["Supervisor", "Process manager for all services"],
    ]
)

doc.add_paragraph()
doc.add_paragraph(
    "Everything runs on the same network as the monitored infrastructure. "
    "The API can SSH directly to servers, connect to databases, and receive "
    "SNMP traps and NetFlow because it is co-located in the data center."
)

doc.add_paragraph()
doc.add_paragraph("Docker Image: ghcr.io/rameshsraj/zentra:1.0.0")
doc.add_paragraph("Base OS: Rocky Linux 8.10")

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# SECTION 3: SAAS ARCHITECTURE
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("3. SaaS Architecture (v2.0 — Agent + Cloud)", level=1)

# 3.1
doc.add_heading("3.1 Architecture Overview", level=2)

doc.add_paragraph(
    "In the SaaS model, Zentra splits into two deployable units:"
)

add_table(
    ["Component", "Runs On", "Image", "Contains"],
    [
        ["zentra-cloud", "GCP (you manage)", "ghcr.io/rameshsraj/zentra-cloud:2.0.0",
         "API + DB + Frontend + Nginx"],
        ["zentra-agent", "Customer DC", "ghcr.io/rameshsraj/zentra-agent:2.0.0",
         "Agent binary + SSH/SNMP tools only (~20MB)"],
    ]
)

doc.add_paragraph()
doc.add_paragraph("High-Level Architecture Diagram:")
doc.add_paragraph()

# ASCII architecture diagram
add_code_block(
    "┌─────────────────────────────────────────────────────────┐\n"
    "│                    GCP (Zentra SaaS)                    │\n"
    "│                                                         │\n"
    "│   ┌──────────┐    ┌──────────┐    ┌──────────────────┐ │\n"
    "│   │  React   │───▶│ zentra-  │───▶│  TimescaleDB     │ │\n"
    "│   │ Frontend │    │   api    │    │  (PostgreSQL 16) │ │\n"
    "│   │ (Nginx)  │    │ (Axum)  │    │                  │ │\n"
    "│   └──────────┘    └────▲─────┘    └──────────────────┘ │\n"
    "│                        │                                │\n"
    "│                   HTTPS│ingest                          │\n"
    "│                   (443)│                                │\n"
    "└────────────────────────┼────────────────────────────────┘\n"
    "                         │\n"
    "          ═══════════ INTERNET ════════════\n"
    "                         │\n"
    "                    Outbound only\n"
    "                    (no VPN needed)\n"
    "                         │\n"
    "┌────────────────────────┼────────────────────────────────┐\n"
    "│            Customer Data Center                         │\n"
    "│                        │                                │\n"
    "│              ┌─────────▼──────────┐                     │\n"
    "│              │   zentra-agent     │                     │\n"
    "│              │                    │                     │\n"
    "│              │  ┌──────────────┐  │                     │\n"
    "│              │  │ ServerPlugin │──┼──▶ Linux/Win (SSH)  │\n"
    "│              │  ├──────────────┤  │                     │\n"
    "│              │  │   DBPlugin   │──┼──▶ Oracle/PG (SQL)  │\n"
    "│              │  ├──────────────┤  │                     │\n"
    "│              │  │NetworkPlugin │──┼──▶ Switches (SNMP)  │\n"
    "│              │  ├──────────────┤  │                     │\n"
    "│              │  │StoragePlugin │──┼──▶ NetApp/Pure(API) │\n"
    "│              │  ├──────────────┤  │                     │\n"
    "│              │  │  NetFlow RX  │◀─┼── Routers (UDP)     │\n"
    "│              │  ├──────────────┤  │                     │\n"
    "│              │  │ SNMP Trap RX │◀─┼── Any device (UDP)  │\n"
    "│              │  ├──────────────┤  │                     │\n"
    "│              │  │  IDS/Threat  │──┼──▶ Suricata logs    │\n"
    "│              │  └──────────────┘  │                     │\n"
    "│              └────────────────────┘                     │\n"
    "│                                                         │\n"
    "│   Monitored:  10 Linux │ 10 Windows │ 4 Oracle DB      │\n"
    "│                4 Storage Arrays │ 100 Network Devices   │\n"
    "└─────────────────────────────────────────────────────────┘"
)

# 3.2
doc.add_heading("3.2 Data Flow Patterns", level=2)

doc.add_paragraph(
    "There are three distinct data flow patterns. Understanding these is critical "
    "for the SaaS architecture because each has different network requirements."
)

# 3.3
doc.add_heading("3.3 Pattern 1: Agent Polls Targets (Push to GCP)", level=2)

doc.add_paragraph(
    "The agent reaches out to targets on the local network and collects data. "
    "This data flows through the MetricBatch channel and is pushed to GCP over HTTPS."
)

add_table(
    ["Component", "Protocol", "Direction"],
    [
        ["Linux Servers", "SSH (agentless)", "Agent → Target"],
        ["Windows Servers", "WMI / WinRM", "Agent → Target"],
        ["SNMP Polling", "SNMP v2c/v3 GET/WALK", "Agent → Target"],
        ["IPMI / BMC", "ipmitool CLI", "Agent → Target"],
        ["Oracle / Postgres / MySQL", "SQL connections", "Agent → Target"],
        ["Storage Arrays (NetApp, Pure, Dell)", "REST API / SSH", "Agent → Target"],
        ["vSphere / ESXi", "vSphere API", "Agent → Target"],
        ["Kubernetes", "K8s API", "Agent → Target"],
        ["Prometheus Scrape", "HTTP GET /metrics", "Agent → Target"],
        ["eBPF Profiling", "SSH + perf record", "Agent → Target"],
        ["SSL Cert Checks", "TLS handshake", "Agent → Target"],
    ]
)

doc.add_paragraph()
doc.add_paragraph("SaaS Impact: Straightforward — just change the transport from DB insert to HTTPS POST.")

# 3.4
doc.add_heading("3.4 Pattern 2: Inbound Listeners (Devices Push TO Agent)", level=2)

doc.add_paragraph(
    "These components listen on UDP/TCP ports inside the DC. Network devices "
    "send data TO them. These cannot run on GCP — they must run inside the agent."
)

add_table(
    ["Component", "Port", "Protocol", "Sends To Agent"],
    [
        ["NetFlow v5 Receiver", "UDP 2055", "NetFlow v5", "Routers, switches"],
        ["NetFlow v9 Receiver", "UDP 2056", "NetFlow v9", "Routers, switches"],
        ["IPFIX Receiver", "UDP 4739", "IPFIX", "Routers, firewalls"],
        ["sFlow Receiver", "UDP 6343", "sFlow", "Switches"],
        ["SNMP Trap Receiver", "UDP 162", "SNMP Traps", "Any managed device"],
        ["Packet Capture", "Raw socket", "pcap", "Network tap / mirror port"],
        ["Hubble / Cilium gRPC", "gRPC", "Cilium Hubble", "K8s Cilium nodes"],
    ]
)

doc.add_paragraph()
doc.add_paragraph(
    "SaaS Impact: These listeners must be moved into the agent binary. "
    "Currently they write directly to Postgres — they need to buffer locally "
    "and push aggregated flow records via HTTPS."
)

# 3.5
doc.add_heading("3.5 Pattern 3: Local Security Analysis", level=2)

doc.add_paragraph(
    "These components analyze raw traffic and logs in real-time. The raw data is "
    "too high-volume to ship to GCP. Analysis happens at the agent; only results "
    "(alerts, threat events, aggregated metrics) are pushed."
)

add_table(
    ["Component", "Data Source"],
    [
        ["IDS Engine", "Suricata EVE JSON logs"],
        ["Threat Detection", "Packet capture state (port scans, DDoS, exfiltration)"],
        ["DNS Anomaly Detection", "Captured DNS traffic"],
        ["Firewall Log Parsing", "iptables / firewalld / nftables log lines"],
    ]
)

doc.add_paragraph()
doc.add_paragraph("SaaS Impact: Minimal — these already produce Metric and ThreatEvent structs that flow through the pipeline.")

# 3.6
doc.add_heading("3.6 Agent as Remote Executor", level=2)

doc.add_paragraph(
    "In v1.0, the API server handles SSH verification, SSH key setup, server crawling, "
    "and connection testing directly — because it runs on the same network. "
    "In SaaS, the API is on GCP and cannot reach internal IPs. These functions "
    "must be delegated to the agent."
)

add_table(
    ["Function", "v1.0 (runs on API)", "v2.0 SaaS (must run on Agent)"],
    [
        ["verify_ssh(host)", "API SSHes to host directly", "API sends command → Agent SSHes → returns result"],
        ["setup_ssh(host, user, pass)", "API pushes SSH key", "API sends command → Agent pushes key → returns result"],
        ["crawl_server(target_id)", "API runs discovery script via SSH", "API sends command → Agent crawls → returns CrawlResult"],
        ["test_connection(host, port)", "API does TcpStream::connect", "API sends command → Agent connects locally → returns result"],
        ["test_db_connection(host, creds)", "API does sqlx::connect", "API sends command → Agent connects → returns result"],
    ]
)

doc.add_paragraph()
doc.add_paragraph("Communication Pattern:")
add_code_block(
    "GCP API                              Agent (DC)\n"
    "────────                             ──────────\n"
    "                                     \n"
    "Stores targets in DB                 Connects outbound to GCP API\n"
    "Serves UI                            Polls: GET /api/agent/commands\n"
    "Queues commands for agent             Executes locally (SSH, TCP, SNMP)\n"
    "                                     Posts results: POST /api/agent/results"
)

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# SECTION 4: TARGET MANAGEMENT
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("4. Target Management — How the Agent Knows What to Monitor", level=1)

# 4.1
doc.add_heading("4.1 Manual Target Registration (via UI)", level=2)

doc.add_paragraph(
    "An admin opens the Zentra UI (on GCP) and clicks Add Target. "
    "They fill in the details and the target is saved to the database."
)

doc.add_paragraph("Example — Adding a Linux Server:")
add_code_block(
    'POST /api/targets\n'
    '{\n'
    '  "name": "prod-web-01",\n'
    '  "target_subtype": "linux",\n'
    '  "host": "10.0.1.10",\n'
    '  "port": 22,\n'
    '  "username": "zentra",\n'
    '  "environment": "production",\n'
    '  "tags": { "dc": "us-east-1", "rack": "A3" }\n'
    '}'
)

doc.add_paragraph("Supported Target Types and Subtypes:")
add_table(
    ["Type", "Subtypes"],
    [
        ["Server", "linux, windows, container, kubernetes, esxi, vsphere, vcenter, snmp, ipmi, bmc"],
        ["Database", "oracle, postgresql, mysql, mariadb, mongodb, redis"],
        ["Storage", "localdisk, nfs, san, hp3par, s3, minio, ceph, netapp, purestorage, veeam, san_fabric"],
        ["Network", "interface, switch, router, firewall, bgp, ospf, vpn, dns, dhcp, netflow, sflow, ipfix"],
        ["Middleware", "nginx, apache, elasticsearch, haproxy, tomcat, phpfpm, rabbitmq, kafka, memcached"],
    ]
)

doc.add_paragraph()
doc.add_paragraph(
    "The subtype automatically determines the parent type. For example, "
    "TargetSubtype::Oracle maps to TargetType::Database."
)

# 4.2
doc.add_heading("4.2 Auto-Discovery (Server Crawl)", level=2)

doc.add_paragraph(
    "When a server target is added, Zentra automatically crawls it via SSH "
    "to discover additional components running on that server. The crawl checks for:"
)

add_table(
    ["Section", "Component Type", "Subtype", "Detection Method"],
    [
        ["ORACLE", "database", "oracle", "sqlplus binary or port 1521 listening"],
        ["NGINX", "middleware", "nginx", "nginx binary or process listening"],
        ["APACHE", "middleware", "apache", "httpd/apache2 binary or process"],
        ["TOMCAT", "middleware", "tomcat", "Port 8080 listening"],
        ["NFS", "storage", "nfs", "showmount or port 2049"],
        ["SNMP", "network", "snmp", "Port 161 (UDP) listening"],
        ["FIREWALL", "network", "firewall", "firewalld / nftables / iptables active"],
    ]
)

doc.add_paragraph()
doc.add_paragraph(
    "The UI shows discovered components with checkboxes. The admin selects which "
    "to monitor, and each becomes a separate target with its own plugin and collection loop."
)

add_note(
    "In SaaS mode, the crawl must be executed by the agent (not the API), "
    "since the API on GCP cannot SSH to internal servers."
)

# 4.3
doc.add_heading("4.3 Service Discovery Providers", level=2)

doc.add_paragraph("The agent also supports automatic target discovery from external sources:")

add_table(
    ["Provider", "How It Works", "Refresh Interval"],
    [
        ["Static File", "Reads targets from a JSON/YAML config file", "Every 5 minutes"],
        ["DNS-SD", "Looks up SRV records (e.g., _postgres._tcp.dc.local)", "Every 60 seconds"],
        ["Docker", "Queries Docker API for containers with label Zentra.enable=true", "Every 30 seconds"],
        ["Kubernetes", "Queries K8s API for pods with annotation Zentra.io/scrape=true", "Every 30 seconds"],
    ]
)

# 4.4
doc.add_heading("4.4 Plugin Routing", level=2)

doc.add_paragraph(
    "The agent's create_plugin_for_target() function matches the target subtype "
    "to the correct plugin:"
)

add_code_block(
    "TargetSubtype::Linux | Windows     → ServerPlugin   (SSH, SNMP, IPMI, WMI)\n"
    "TargetSubtype::PostgreSQL | MySQL   → DatabasePlugin (SQL connections)\n"
    "TargetSubtype::LocalDisk | NetApp   → StoragePlugin  (REST/SSH)\n"
    "TargetSubtype::Switch | Router      → NetworkPlugin  (SNMP, pcap)"
)

doc.add_paragraph()
doc.add_paragraph(
    "The agent polls for new targets every 30 seconds. When a new target appears, "
    "it acquires a PostgreSQL advisory lock (leader election), creates the plugin, "
    "and starts a dedicated collection loop with backoff and retry."
)

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# SECTION 5: DEPLOYMENT
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("5. Deployment Strategy", level=1)

# 5.1
doc.add_heading("5.1 v1.0 — Single Container Deployment", level=2)

doc.add_paragraph("Build (on main branch):")
add_code_block(
    "git checkout main\n"
    "./docker/build-image.sh\n"
    "# Produces: docker/zentra-latest.tar.gz\n"
    "# Push to GHCR:\n"
    "docker push ghcr.io/rameshsraj/zentra:1.0.0"
)

doc.add_paragraph("Deploy (in customer DC):")
add_code_block(
    "docker run -d \\\n"
    "  --name zentra \\\n"
    "  -p 5010:5010 \\\n"
    "  -v zentra-data:/var/lib/postgresql/16/data \\\n"
    "  ghcr.io/rameshsraj/zentra:1.0.0"
)

# 5.2
doc.add_heading("5.2 v2.0 — SaaS Dual-Image Deployment", level=2)

doc.add_paragraph("Build (on zentra-saas branch):")
add_code_block(
    "git checkout zentra-saas\n"
    "\n"
    "# Build GCP image (API + DB + Frontend)\n"
    "docker build -f docker/Dockerfile.cloud -t zentra-cloud:2.0.0 .\n"
    "docker push ghcr.io/rameshsraj/zentra-cloud:2.0.0\n"
    "\n"
    "# Build Agent image (lightweight, agent only)\n"
    "docker build -f docker/Dockerfile.agent -t zentra-agent:2.0.0 .\n"
    "docker push ghcr.io/rameshsraj/zentra-agent:2.0.0"
)

doc.add_paragraph("Deploy — GCP Side (you manage):")
add_code_block(
    "docker run -d \\\n"
    "  --name zentra-cloud \\\n"
    "  -p 443:443 \\\n"
    "  -v zentra-data:/var/lib/postgresql/16/data \\\n"
    "  ghcr.io/rameshsraj/zentra-cloud:2.0.0"
)

doc.add_paragraph("Deploy — Customer DC (customer runs):")
add_code_block(
    "docker run -d \\\n"
    "  --name zentra-agent \\\n"
    "  -e ZENTRA_API_URL=https://your-zentra-gcp.com \\\n"
    "  -e ZENTRA_API_KEY=customer-api-key \\\n"
    "  -p 2055:2055/udp -p 2056:2056/udp \\\n"
    "  -p 4739:4739/udp -p 6343:6343/udp \\\n"
    "  -p 162:162/udp \\\n"
    "  ghcr.io/rameshsraj/zentra-agent:2.0.0"
)

doc.add_paragraph()
add_note(
    "The agent only requires outbound HTTPS (port 443) to your GCP IP. "
    "The UDP ports (2055, 2056, 4739, 6343, 162) are for receiving NetFlow, "
    "sFlow, IPFIX, and SNMP traps from devices within the same DC."
)

doc.add_paragraph("Multi-Customer Deployment:")
add_code_block(
    "┌─────────────────────────────────────────┐\n"
    "│      GCP (you manage — one instance)    │\n"
    "│      zentra-cloud:2.0.0                 │\n"
    "│      Multi-tenant DB (org_id scoping)   │\n"
    "└──────────────┬──────────┬───────────────┘\n"
    "               │          │\n"
    "          HTTPS│     HTTPS│\n"
    "               │          │\n"
    "    ┌──────────▼──┐  ┌───▼───────────┐\n"
    "    │ Customer A  │  │  Customer B   │\n"
    "    │ DC Agent    │  │  DC Agent     │\n"
    "    │ API_KEY=aaa │  │  API_KEY=bbb  │\n"
    "    └─────────────┘  └───────────────┘"
)

doc.add_page_break()

# ═══════════════════════════════════════════════════════════════════════
# SECTION 6: CODE CHANGES
# ═══════════════════════════════════════════════════════════════════════
doc.add_heading("6. Code Changes Required for SaaS", level=1)

# 6.1
doc.add_heading("6.1 Agent Transport Swap", level=2)

doc.add_paragraph(
    "The agent currently writes metrics directly to Postgres via IngestionService. "
    "For SaaS, replace this with an HTTP forwarder that POSTs to the GCP API."
)

doc.add_paragraph("Current (v1.0):")
add_code_block(
    "Plugins → collect_metrics() → MetricBatch → mpsc channel → IngestionService → INSERT INTO metrics")

doc.add_paragraph("SaaS (v2.0):")
add_code_block(
    "Plugins → collect_metrics() → MetricBatch → mpsc channel → HttpForwarder → HTTPS POST /api/ingest")

doc.add_paragraph("Specific coupling points to change:")
add_table(
    ["#", "Coupling Point", "File", "Change Required"],
    [
        ["1", "IngestionService writes to Postgres", "crates/zentra-ingestion/src/lib.rs",
         "Replace insert_metrics(&pool) with HTTP POST to /api/ingest"],
        ["2", "list_targets() reads from DB", "crates/zentra-agent/src/main.rs",
         "Replace with GET /api/targets HTTP call"],
        ["3", "leader_election uses PG advisory locks", "crates/zentra-agent/src/leader_election.rs",
         "Replace with API-based lock or local file lock"],
        ["4", "EbpfProfiler writes to DB", "crates/zentra-agent/src/collectors/ebpf_profiler.rs",
         "POST profile data to API"],
        ["5", "FlowServer writes to Postgres", "crates/zentra-netflow/src/flow_server.rs",
         "Buffer locally, push aggregated flows via HTTP"],
        ["6", "SnmpTrapServer writes to Postgres", "crates/zentra-ingestion/src/snmp_trap_server.rs",
         "Buffer locally, push via HTTP"],
    ]
)

# 6.2
doc.add_heading("6.2 Functions That Must Move to Agent", level=2)

add_table(
    ["Function", "Current Location", "Why It Must Move"],
    [
        ["verify_ssh()", "zentra-api/handlers/targets.rs", "API on GCP cannot SSH to internal IPs"],
        ["setup_ssh()", "zentra-api/handlers/targets.rs", "API on GCP cannot push SSH keys to internal servers"],
        ["crawl_server()", "zentra-api/handlers/targets.rs", "API on GCP cannot run SSH discovery scripts"],
        ["test_connection()", "zentra-api/handlers/targets.rs", "API on GCP cannot TCP connect to internal IPs"],
        ["test_db_connection()", "zentra-api/handlers/targets.rs", "API on GCP cannot sqlx::connect to internal DBs"],
    ]
)

# 6.3
doc.add_heading("6.3 Effort Summary", level=2)

add_table(
    ["Work Item", "Effort", "What Changes"],
    [
        ["Agent HTTP transport", "~300 lines new code", "Replace IngestionService with HttpForwarder"],
        ["Agent command channel", "~200 lines new code", "Agent polls /api/agent/commands, executes, returns results"],
        ["Move crawl/SSH to agent", "~150 lines moved", "Same logic, different execution location"],
        ["API-side ingest endpoint", "~100 lines new code", "REST endpoint that receives MetricBatch JSON"],
        ["API key auth per org", "~100 lines new code", "Bearer token validation middleware"],
        ["Dockerfile.cloud", "~40 lines", "API + DB + Frontend image"],
        ["Dockerfile.agent", "~30 lines", "Agent-only image"],
    ]
)

doc.add_paragraph()
doc.add_paragraph(
    "What does NOT change: all plugins (server, database, network, storage, security), "
    "the orchestrator's collection loop, threat detection algorithms, packet analysis, "
    "SNMP parsing, the React frontend, all 100+ API handlers, dashboards, ML/predict, "
    "and the query engine."
)

# ── Save ──
output_path = "/System/Volumes/Data/Projects/Zentra/docs/Zentra_SaaS_Architecture_and_Git_Workflow.docx"
doc.save(output_path)
print(f"Document saved to: {output_path}")
