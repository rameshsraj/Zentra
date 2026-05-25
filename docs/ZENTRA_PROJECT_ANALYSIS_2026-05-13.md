# ZENTRA — Complete Project Analysis

> **Generated:** 13 May 2026  
> **Version:** 1.0.0  
> **Language:** Rust (Backend) · TypeScript/React (Frontend)  
> **License:** Proprietary

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Backend — Rust Crate Inventory](#3-backend--rust-crate-inventory)
4. [Plugin System — 8 Monitoring Plugins](#4-plugin-system--8-monitoring-plugins)
5. [Frontend — React Dashboard](#5-frontend--react-dashboard)
6. [Database Monitoring — Implemented Features](#6-database-monitoring--implemented-features)
7. [Network Monitoring — Implemented Features](#7-network-monitoring--implemented-features)
8. [Storage Monitoring — Implemented Features](#8-storage-monitoring--implemented-features)
9. [Server & Infrastructure Monitoring](#9-server--infrastructure-monitoring)
10. [Security & Threat Detection](#10-security--threat-detection)
11. [Middleware & Service Monitoring](#11-middleware--service-monitoring)
12. [Cloud Monitoring (AWS / Azure / GCP)](#12-cloud-monitoring-aws--azure--gcp)
13. [AI, ML & Predictive Analytics](#13-ai-ml--predictive-analytics)
14. [Alerting & Notification Engine](#14-alerting--notification-engine)
15. [Log Engine & Analytics](#15-log-engine--analytics)
16. [Dashboard & Visualization System](#16-dashboard--visualization-system)
17. [Authentication, RBAC & Multi-Tenancy](#17-authentication-rbac--multi-tenancy)
18. [License Management](#18-license-management)
19. [Database Schema — Full Inventory](#19-database-schema--full-inventory)
20. [Deployment & Infrastructure](#20-deployment--infrastructure)
21. [API Surface — Complete Endpoint Map](#21-api-surface--complete-endpoint-map)
22. [Implementation Statistics](#22-implementation-statistics)

---

## 1. Executive Summary

Zentra is a **fully implemented enterprise infrastructure monitoring and observability platform** built entirely in Rust with a React/TypeScript frontend. It provides:

- **Multi-domain monitoring** — Servers, Databases, Storage, Network, Cloud, Middleware, Security
- **8 purpose-built monitoring plugins** — Network, Database, Storage, Server, Security, Cloud, AI, License
- **Advanced analytics** — Anomaly detection (Z-Score, EWMA, Seasonal, K-Means), capacity forecasting, trend alerting
- **Enterprise features** — Multi-tenancy, RBAC, MFA, audit logging, row-level security, license compliance
- **Prometheus compatibility** — Remote write, PromQL queries, scrape targets, recording rules
- **Real-time streaming** — Event bus (pub/sub), WebSocket live logs, alert escalation chains
- **22+ UI pages** with 25 chart types, drill-down navigation, floating AI chatbot, and dark mode

The platform monitors **Oracle, PostgreSQL, MySQL, MongoDB, Redis** databases, **NetApp, Pure Storage, Ceph, MinIO, Veeam, ZFS** storage systems, **AWS/Azure/GCP** cloud providers, and **Nginx, Apache, HAProxy, Kafka, Elasticsearch, Redis, Docker, Kubernetes** middleware — all from a single unified interface.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         FRONTEND (React 18)                         │
│  Vite 5 · Tailwind CSS · Recharts · React Query · Zustand · Lucide │
│  22 Pages · 25 Chart Types · Dark Mode · Command Palette · MCP Chat│
├──────────────────────────────┬──────────────────────────────────────┤
│          REST API            │          WebSocket                    │
│      (Axum + Tower)          │     (Live Logs / Events)             │
├──────────────────────────────┴──────────────────────────────────────┤
│                       zentra-api (Main Server)                      │
│  18 Route Modules · 89 Handlers · JWT Auth · Rate Limiting · CORS  │
│  OpenAPI Docs (utoipa) · Prometheus Metrics · API Versioning (v1)   │
├─────────────┬──────────────┬──────────────┬─────────────────────────┤
│ zentra-query│zentra-stream │zentra-drilldown│  zentra-ingestion     │
│ PromQL      │ Event Bus    │ Hierarchy Nav  │  Metric/Log Pipeline  │
│ Aggregation │ Alert Engine │ 10 Levels      │  SNMP Trap Server     │
│ Global Search│Notifications│ Breadcrumbs    │                       │
├─────────────┴──────────────┴──────────────┴─────────────────────────┤
│                        zentra-db (Data Layer)                       │
│  SQLx · PgPool · Repository Pattern · TimescaleDB · Migrations     │
├─────────────────────────────────────────────────────────────────────┤
│                     PostgreSQL 16 + TimescaleDB                     │
│  60+ Tables · Hypertables · Continuous Aggregates · Compression     │
│  Row-Level Security · Full-Text Search (tsvector + trigram)         │
├─────────────────────────────────────────────────────────────────────┤
│                      zentra-agent (Collector)                       │
│  Plugin Orchestrator · Leader Election · Target Discovery           │
│  Exponential Backoff · Graceful Shutdown · Multi-Target Polling     │
├──────────┬──────────┬──────────┬──────────┬──────────┬──────────────┤
│ network  │ database │ storage  │ server   │ security │ cloud / ai / │
│ plugin   │ plugin   │ plugin   │ plugin   │ plugin   │ license      │
└──────────┴──────────┴──────────┴──────────┴──────────┴──────────────┘
│                     zentra-ml / zentra-predict                      │
│  Z-Score · EWMA · Seasonal · K-Means · Linear Regression           │
│  Baselines · Forecasts · Trend Alerts · Recording Rules             │
├─────────────────────────────────────────────────────────────────────┤
│                        zentra-netflow                               │
│  NetFlow v5/v9 · IPFIX · sFlow · UDP Listeners · Flow Aggregation  │
├─────────────────────────────────────────────────────────────────────┤
│                       zentra-log-engine                             │
│  Multi-Format Parser (Plain/JSON/Syslog/PostgreSQL CSV/Custom)      │
│  Log Correlation · Root Cause Analysis · Severity Inference         │
├─────────────────────────────────────────────────────────────────────┤
│                        zentra-scrape                                │
│  Prometheus Text Format Parser · HTTP Scraper · Target Discovery    │
│  Recording Rules · Health Tracking · Multi-Provider SD              │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Backend — Rust Crate Inventory

| # | Crate | Purpose | Key Highlights |
|---|-------|---------|----------------|
| 1 | **zentra-core** | Foundation & shared types | Models (Target, Metric, Alert, Log, Event), Plugin trait, AES-GCM crypto, config, error types |
| 2 | **zentra-api** | REST/WebSocket server | 18 route modules, 89 handlers, Axum framework, JWT auth, rate limiting, CORS, Prometheus exporter |
| 3 | **zentra-db** | Database access layer | SQLx PgPool, repository pattern, dual pool (meta 5s / TSDB 60s), advisory locks |
| 4 | **zentra-agent** | Distributed collection agent | Plugin orchestrator, leader election, target discovery, graceful shutdown, exponential backoff |
| 5 | **zentra-ingestion** | Data ingestion pipeline | Metric & log batch consumers, SNMP trap server, event publishing |
| 6 | **zentra-stream** | Event-driven alert processing | Event bus (broadcast, 4096 capacity), alert engine, escalation chains, notification dispatch |
| 7 | **zentra-query** | Query engine | PromQL parser & evaluator, metric aggregation, log search, global cross-domain search |
| 8 | **zentra-drilldown** | Multi-level navigation | 10-level hierarchy (Global→Metric), breadcrumbs, context-aware queries |
| 9 | **zentra-log-engine** | Log processing & analytics | 5 format parsers, regex caching (ReDoS protection), severity inference, log correlation |
| 10 | **zentra-ml** | Machine learning algorithms | Z-Score, EWMA, Seasonal anomaly detection, in-memory state, stale metric eviction |
| 11 | **zentra-predict** | Predictive analytics | Baselines (7-day rolling), linear regression forecasting, K-Means clustering, recording rules, trend alerts |
| 12 | **zentra-netflow** | Network flow processing | NetFlow v5/v9, IPFIX, sFlow UDP receivers, in-memory flow aggregation, TopN extraction |
| 13 | **zentra-scrape** | Prometheus-compatible scraper | HTTP scraper, text format parser, target discovery (Static, FileSD, Consul, Kubernetes, Custom) |

**Workspace:** 13 crates + 8 plugins = **21 Rust packages** in the workspace.

---

## 4. Plugin System — 8 Monitoring Plugins

All plugins implement the core `Plugin` trait:

```rust
trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    async fn initialize(&mut self, target: &Target) -> Result<()>;
    async fn test_connection(&self, target: &Target) -> Result<bool>;
    async fn collect_metrics(&self, target: &Target) -> Result<Vec<Metric>>;
    async fn health_check(&self, target: &Target) -> Result<bool>;
    fn polling_interval(&self) -> Duration;
}
```

### 4.1 Network Plugin

| Feature | Implementation |
|---------|---------------|
| Live packet capture | libpcap with BPF filter support |
| Interface metrics | bytes/packets received/transmitted, errors, drops (via sysinfo) |
| Traffic analysis | Anomaly detection, threshold-based alerts |
| Host resolution | DNS reverse lookups, geographic IP location |
| Service classification | Port-to-service mapping database |
| Process-to-connection | Map network sockets to processes |
| Threat engine | IDS/IPS-style threat detection |
| SNMP traps | Trap receiver with log conversion |
| Routing protocols | BGP, OSPF monitoring |
| VPN tunnels | Tunnel detection and status |
| DNS/DHCP | Traffic analysis for DNS and DHCP |
| Bandwidth | Utilization aggregation |
| Supported subtypes | interface, switch, router, firewall, bgp, ospf, vpn, dns, dhcp, netflow, sflow, ipfix |

### 4.2 Database Plugin

| Feature | Implementation |
|---------|---------------|
| **Oracle** — RAC monitoring | gv$instance, gv$sysstat, interconnect latency, per-instance metrics |
| **Oracle** — Data Guard | Apply lag, transport lag, redo sequence, role tracking |
| **Oracle** — RMAN | Backup job status, duration, compression ratio, failure count |
| **Oracle** — Tablespaces | Usage%, autoextend settings, max size, free space |
| **Oracle** — Wait events | Top-10 waits by total_waits and time_waited |
| **Oracle** — Long queries | v$sql filtering by elapsed time with SQL text capture |
| **Oracle** — Listeners | TNS listener health via Oracle Net API |
| **Oracle** — PDB/CDB | Per-pluggable database resource usage (12c+) |
| **Oracle** — Connection pool | Native OCI FFI via `oracle` crate (feature-gated), SSH+sqlplus fallback |
| **PostgreSQL** | Active connections, database size, xact_commit/rollback, tup_fetched |
| **MySQL/MariaDB** | Connection monitoring, query performance |
| **MongoDB** | Collection metrics |
| **Redis** | Key count, memory, connections |

### 4.3 Storage Plugin

| Feature | Implementation |
|---------|---------------|
| **NetApp ONTAP** | Aggregate capacity/performance, volume usage, LUN status, SnapMirror replication lag, cache hit ratio |
| **Pure Storage** | Array capacity/performance, volume replicas, snapshots, read/write latency, bandwidth |
| **Ceph** | Cluster health, PG status, OSD disk usage, pool capacity, replication status |
| **MinIO / S3** | Bucket usage, object count, API rates (GET/PUT/DELETE), bandwidth |
| **Veeam VBR** | Backup job status/duration, data processed/deduped/compressed, repository capacity |
| **ZFS** | Pool capacity, compression ratio, ARC cache hit rate, snapshot count/size |
| **Dell EMC** | Unity, PowerStore, VNX |
| **Local disks** | sysinfo-based disk metrics |
| **NFS/SAN** | File system and block storage monitoring |

### 4.4 Server Plugin

| Feature | Implementation |
|---------|---------------|
| **Linux** | CPU%, memory, disk per-mount, processes, file descriptors, sockets, load avg, swap (SSH agentless) |
| **Windows** | WMI performance counters via WinRM |
| **S.M.A.R.T.** | Disk health metrics via SSH |
| **Redfish (BMC)** | Hardware inventory, power state, thermal status (iDRAC/iLO) |
| **IPMI** | Hardware monitoring |
| **SNMP** | Device polling |
| **vSphere / ESXi / vCenter** | VM CPU/memory allocation, guest metrics |
| **Kubernetes** | Node/pod resource usage, restart counts |
| **Containers** | Docker container monitoring |

### 4.5 Security Plugin

| Feature | Implementation |
|---------|---------------|
| IDS engine | Suricata/Snort-style alert processing, EVE JSON parsing, alert thresholding (100/SID/5min) |
| Firewall log parser | iptables, firewalld, pf (OpenBSD) normalization |
| DNS analyzer | Query/response analysis, anomaly detection, DNS tunneling detection |
| HTTP analyzer | Method/status/header analysis |
| SSL/TLS analyzer | Certificate chain validation, JA3 fingerprinting |
| Community ID | RFC 7539 flow correlation |
| GeoIP enrichment | IP geolocation for threat mapping |
| File analysis | Malware/suspicious file detection |

### 4.6 Cloud Plugin

| Provider | Services Monitored | Metrics |
|----------|-------------------|---------|
| **AWS** | EC2, RDS, ELB/ALB, Lambda, ECS, EKS | CPU%, NetworkIn/Out, DiskOps, StatusCheck, IOPS, Latency, Invocations, Errors, Throttles |
| **Azure** | VMs, SQL Database, AKS, App Service, Storage, Load Balancer | CPU%, DTU%, Connections, Deadlocks, Worker%, Response time, Memory |
| **GCP** | GCE, Cloud SQL, GKE, Cloud Run, Load Balancer | CPU%, Memory%, Disk%, Connections, Request count/latency, Instance count |

Authentication: AWS SigV4, Azure OAuth2 client credentials, GCP JWT service account. Graceful fallback to simulated data on connection failure.

### 4.7 AI Plugin

| Feature | Implementation |
|---------|---------------|
| Prompt injection detection | Regex patterns for instruction overrides, role-playing attacks, jailbreak markers |
| PII scanner | SSN, credit card, phone number, email detection |
| Toxicity scorer | Violence, hate speech, self-harm, illegal activity keyword classification |
| Hallucination detector | Word-set overlap between LLM response and context, Z-score measurement |

### 4.8 License Plugin

| Feature | Implementation |
|---------|---------------|
| Expiry checker | Daily scan, 90-day warning / 30-day critical thresholds, auto-expire |
| Seat utilization | Total vs. assigned seats, under/over-licensed detection, wasted cost calculation |
| Compliance report | Per-license utilization%, org-level compliance score (0-100%) |
| Audit logging | Immutable trail for all license changes |

---

## 5. Frontend — React Dashboard

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | React 18.3.1 + TypeScript 5.5.4 |
| Build | Vite 5.4 (port 5010, vendor splitting) |
| Routing | React Router DOM v6 |
| State | React Query v5 (30s stale/refetch), Zustand 4.5 |
| Charts | Recharts 2.12 (25 visualization types) |
| Styling | Tailwind CSS 3.4 with dark mode + 15 CSS custom properties |
| Icons | Lucide React 0.424 |

### Pages Implemented (22 Main Views)

| Category | Page | Key Features |
|----------|------|-------------|
| **Command Center** | Dashboard | System status, events, alerts, sparklines, traffic charts, heatmaps, gauges, timeline |
| **Infrastructure** | InfrastructureDashboard | KPI strip (servers/storage/network), utilization gauges, drill-down |
| | Servers | CPU/memory/disk sparklines, modal drilldowns, SSH key mgmt, process tracking |
| | StorageArrays | SAN/NAS/DAS arrays, capacity planning, IOPS, vendor/model details |
| | Network | Multi-tab: connections, hosts, devices, blacklist, protocol breakdown, PCAP export/import, BPF filters |
| **Databases** | DatabaseDashboard | Multi-DB overview (Oracle, PostgreSQL, MySQL, MongoDB, SQL Server), type filtering |
| | OracleMonitoring | **17 tabs:** connections, tablespaces, wait classes, cache/SGA, top queries, sessions, PDB/CDB, Data Guard, RMAN, listeners, RAC, alert log, TxEventQ, SQL workspace, scrape status |
| | OracleMetricEditor | Custom metric creation and testing for Oracle |
| **Middleware** | MiddlewareMonitoring | 12 service types: Nginx, Apache, HAProxy, Tomcat, PHP-FPM, RabbitMQ, Kafka, Elasticsearch, Redis, Memcached, Docker, Kubernetes |
| | MiddlewareServicePage | Per-service detail with status/history tabs |
| | NginxMonitoring | Nginx-specific deep metrics |
| | ApacheMonitoring | Apache scoreboard and performance |
| **Security** | SecurityDashboard | IDS/IPS stats, severity distribution, top attack sources, connection/DNS stats |
| | ThreatHunting | Unified search across IDS alerts, connections, DNS logs |
| | FirewallMonitoring | Firewall rule and event monitoring |
| **Logs** | LogExplorer | 4 tabs: LogQL queries, stream mgmt, alert config, analytics |
| | LogSources | 6 source types: file, syslog, Docker, CloudWatch |
| | LogAlerts | Log-based alert rules with matcher expressions |
| | LogAnalytics | Volume by source (pie), error rate trends (area), top errors table |
| **Alerting** | Alerts | Status filters, inhibition rules, grouped view, trend charts |
| | AlertRules | 4 tabs: rules, groups, templates (10 built-in), library |
| | NotificationChannels | 8 channel types, policies, mute timings, escalation chains, on-call schedules |
| **AI/ML** | AIInsights | ML models (IsolationForest, RandomForest, KMeans, GradientBoosting), anomaly detection, forecasting, cost optimization |
| | PredictiveAlerts | 3 tabs: anomalies, forecasts, trend alerts with severity filtering |
| | RCA | Root Cause Analysis with error patterns, solution recommendations, confidence scoring |
| **Admin** | UserManagement | CRUD, bulk lock/unlock, password reset, role/group assignment |
| | UserGroupManagement | Group hierarchy and member management |
| | RoleManagement | Role definitions with permission matrices |
| | UserProfile | Profile editing, preferences, API token management |
| **Automation** | Runbooks | 70+ automated remediation runbooks: disk cleanup, CPU throttling, DB optimization, network failover |
| **Auth** | LoginPage | Email/password with MFA (2FA), ForgotPassword, ResetPassword |

### Chart Components (25 Types)

| Chart | Use Case |
|-------|----------|
| Sparkline | Mini time-series (20-point inline) |
| MetricAreaChart | Multi-metric area charts |
| StackedAreaChart | Percentage breakdowns |
| BarChart | Standard bar chart |
| BarGauge | Horizontal gauge bars |
| RadialBar | Circular radial bar |
| DonutChart | Donut/pie with center label |
| PieChart | Simple pie chart |
| GaugeChart | Circular gauge (0-100%) with needle |
| XYChart | Generic scatter/line |
| TrafficChart | Inbound/outbound dual-axis |
| HeatmapGrid | Cell-based heatmap (health matrix) |
| Histogram | Distribution histogram |
| CandlestickChart | OHLC (forecasting) |
| StateTimeline | Status over time (healthy→warning→critical) |
| TimelineList | Vertical event timeline |
| StatusGrid | Grid of status tiles |
| ProgressBar | Horizontal progress (disk/memory %) |
| StatCard | KPI card with value + trend |
| AlertBadge | Severity badge |
| TextPanel | Markdown/text display |
| LogPanel | Scrollable log viewer |
| DataTable | Table with pagination |
| TracesWaterfall | Distributed trace waterfall (OpenTelemetry) |
| GeoMap | Geographic IP heatmap (country stats) |

### Reusable Components (20+)

- **Layout.tsx** — Main app shell with collapsible nested sidebar
- **PageHeader.tsx** — Header with refresh, search, add target buttons
- **AddTargetModal.tsx** — Generic target creation (servers, databases, storage, network)
- **AddDatabaseModal.tsx** — Database-specific form
- **AddNetworkDeviceModal.tsx** — Network device creation
- **DrilldownModal.tsx** — Hierarchical drill-down navigator
- **RequireAuth.tsx** — Protected route with role/permission checks
- **PermissionGate.tsx** — Conditional rendering based on RBAC
- **FloatingChatbot.tsx** — AI chat sidebar (MCP integration)
- **CommandPalette.tsx** — Omnibox-style navigation (role-aware commands)
- **PanelInspector.tsx** — Data inspection drawer
- **SqlWorkspaceTab.tsx** — SQL editor for database connections
- **LiveModeButton.tsx** — Toggle for live/demo mode
- **DemoModeBadge.tsx** — Demo mode indicator
- **EmptyState.tsx** — Placeholder for empty views
- **LoadingSkeleton.tsx** — Skeleton loading states
- **ErrorBoundary.tsx** — Error catching wrapper

### API Integration (17 Modules)

| Module | Coverage |
|--------|----------|
| auth.ts | Login, logout, MFA setup/verify, sessions, password reset |
| users.ts | CRUD, bulk actions, lock/unlock, roles, groups |
| targets.ts | Create/delete, SSH setup, health checks, stats |
| metrics.ts | Prometheus-compatible (labels, metadata, series), forecasts, anomalies, K-Means |
| logs.ts | LogQL queries, label introspection, streams, volume, SSE tailing, correlation |
| alerts.ts | CRUD, inhibitions, mute timings, grouping, backtesting |
| dashboards.ts | Templates, export/import, annotations, playlists, bookmarks, query history |
| network.ts | Packet capture start/stop, connections, traffic breakdown, PCAP export/import, blacklist |
| oracle.ts | Connections, custom metrics, alert log, scrape status, TxEventQ, scheduler |
| middleware.ts | Generic endpoints for 12 middleware services |
| infra.ts | Templates, health checks, SLA reports, dependencies, deployments, runbooks |
| security.ts | IDS/IPS rules/alerts, connections, DNS, HTTP, SSL logs, threat lists |
| ai.ts | MCP chat, MCP tools, RCA analysis, trace search (OpenTelemetry) |
| enterprise.ts | Multi-tenancy, licensing, billing |
| health.ts | System status, version, uptime |

---

## 6. Database Monitoring — Implemented Features

### Oracle (Primary Focus — Deepest Coverage)

| Area | Feature | Implementation Detail |
|------|---------|----------------------|
| **Connections** | Native OCI pool | Feature-gated `oracle` Rust crate with FFI, SSH+sqlplus fallback |
| | Pool management | Min/max/increment, max lifetime, query timeout, global pool registry |
| | Auth modes | Password, Wallet, External, Vault |
| | Roles | DEFAULT, SYSDBA, SYSOPER, SYSASM, SYSDG, SYSKM |
| **RAC** | Cluster monitoring | gv$instance, gv$sysstat, interconnect latency, per-instance metrics |
| **Data Guard** | Standby replication | Apply lag, transport lag, redo sequence, role tracking |
| **RMAN** | Backup monitoring | Job status, duration, compression ratio, failure count, last full backup time |
| **Tablespaces** | Space management | Usage%, autoextend settings, max size, free space |
| **Wait Events** | Performance analysis | Top-10 waits by total_waits and time_waited from v$system_event |
| **Top SQL** | Query performance | v$sql filtering by elapsed time with SQL text capture |
| **Sessions** | Session monitoring | Running sessions with status, blocking chains |
| **PDB/CDB** | Multi-tenant DB | Per-pluggable database resource usage (12c+ architecture) |
| **Listeners** | Network health | TNS listener health via Oracle Net API |
| **Alert Log** | Event monitoring | Oracle alert log parsing and display |
| **TxEventQ** | Event streaming | Transaction Event Queues monitoring |
| **Custom Metrics** | Extensibility | User-defined SQL metrics with context, labels, types, and scheduled scraping |
| **Scrape Status** | Health tracking | Per-connection scrape results: health, sample count, duration, errors |
| **SQL Workspace** | Ad-hoc queries | Interactive SQL editor connected to Oracle targets |
| **UI** | 17-tab Oracle page | Connections, tablespaces, wait classes, cache/SGA, top queries, sessions, PDB/CDB, Data Guard, RMAN, listeners, RAC, alert log, TxEventQ, SQL workspace, scrape status |

### PostgreSQL

| Feature | Implementation |
|---------|---------------|
| Connection monitoring | Active connections count |
| Database size tracking | Size per database |
| Transaction rates | xact_commit, xact_rollback counts |
| Tuple metrics | tup_fetched for query workload |
| Background collectors | Automated PostgreSQL metric collection in API server |
| CSV log parsing | PostgreSQL CSV log format parser in log engine |

### MySQL / MariaDB

| Feature | Implementation |
|---------|---------------|
| Connection monitoring | Active connections |
| Query performance | Query stats |
| Plugin support | database-plugin MySQL module |

### MongoDB

| Feature | Implementation |
|---------|---------------|
| Collection metrics | Collection-level monitoring |
| Connection status | Connection pool monitoring |

### Redis

| Feature | Implementation |
|---------|---------------|
| Key count | Database key statistics |
| Memory usage | Memory consumption tracking |
| Connection count | Active client connections |

### Database Schema Tables

- `oracle_connections` — Connection pool configs with encrypted credentials
- `oracle_metric_definitions` — Custom SQL metric definitions (5 seeded defaults: sessions, resource, wait_class, tablespace, asm_diskgroup)
- `oracle_metric_cache` — Latest scraped values per connection/metric pair
- `scrape_targets` — Prometheus scrape targets for any database
- `scrape_results` — Scrape health tracking

---

## 7. Network Monitoring — Implemented Features

### Packet Capture & Analysis

| Feature | Implementation |
|---------|---------------|
| Live capture | libpcap-based with BPF filter expressions |
| Protocol parsing | etherparse for L2-L4 parsing |
| Connection tracking | 5-tuple flow tracking (src/dst IP:port + protocol) |
| Host discovery | Automatic host enumeration from traffic |
| Service classification | Port-to-service mapping database |
| Process mapping | Network socket to process association |
| PCAP export/import | Packet capture file import and export |
| Country stats | Geographic IP statistics |

### Flow Processing (zentra-netflow)

| Feature | Implementation |
|---------|---------------|
| NetFlow v5 | Full PDU parsing (src/dst IP, ports, protocol, packets, bytes) |
| NetFlow v9 | Template-based format parsing with multi-template support |
| IPFIX | RFC 5101/5102, variable-length fields |
| sFlow v5 | Flow sample & counter sample extraction |
| UDP listeners | Port 2055 (NetFlow), 4739 (IPFIX), 6343 (sFlow) |
| Flow aggregation | In-memory aggregation by 5-tuple, TopN extraction, periodic DB flush |
| Flow exporters | Auto-registration with packet/flow counters |

### Network Device Monitoring

| Device Type | Capabilities |
|-------------|-------------|
| Switches | Interface stats, VLAN, port status |
| Routers | Routing tables, BGP peers, OSPF neighbors |
| Firewalls | Rule hit counts, blocked traffic, event logs |
| VPN tunnels | Tunnel status, throughput |
| DNS servers | Query analytics, anomaly detection |
| DHCP servers | Lease tracking, scope utilization |

### Database Schema

- `network_flows` — TimescaleDB hypertable (1-day chunks), 90-day retention, compression after 1 day
- `netflow_1m` — 1-minute continuous aggregate
- `netflow_1h` — 1-hour continuous aggregate
- `netflow_topn_talkers_hourly` — Top talkers aggregate
- `flow_exporters` — Auto-registered exporters with last_seen, packets_rx, flows_rx
- `network_favorites` — Saved IPs/hosts
- `network_blacklist` — Blocked IPs with source tracking (manual/threat_engine/ids_rule)

### UI Features

- Multi-tab interface: connections, hosts, devices, blacklist management
- Protocol breakdown charts
- Country statistics with GeoMap visualization
- PCAP file export and import
- BPF filter editor
- Packet capture control (start/stop)
- Traffic charts (inbound/outbound dual-axis)
- Bandwidth utilization tracking

---

## 8. Storage Monitoring — Implemented Features

### Enterprise SAN Systems

| System | Metrics Collected |
|--------|------------------|
| **NetApp ONTAP** | Aggregate capacity/performance (IOPS, throughput), volume usage, qtree quotas, LUN status, snapshot utilization, SnapMirror replication lag, cache hit ratio |
| **Pure Storage** | Array capacity/performance, volume replicas/snapshots, read/write latency, bandwidth utilization |
| **Dell EMC Unity** | Array monitoring |
| **Dell EMC PowerStore** | Storage performance |
| **Dell EMC VNX** | Legacy array support |

### Distributed Storage

| System | Metrics Collected |
|--------|------------------|
| **Ceph** | Cluster health, PG status, OSD disk usage, pool capacity, replication/erasure code status |
| **IBM GPFS/Spectrum Scale** | File system metrics |

### Object Storage

| System | Metrics Collected |
|--------|------------------|
| **MinIO** | Bucket usage, object count, API request rates (GET/PUT/DELETE), bandwidth |
| **S3-compatible** | Standard S3 metrics |

### Backup Systems

| System | Metrics Collected |
|--------|------------------|
| **Veeam VBR** | Backup job status/duration, data processed/deduplicated/compressed, repository capacity/free space |

### File Systems

| System | Metrics Collected |
|--------|------------------|
| **ZFS** | Pool capacity, compression ratio, ARC cache hit rate, snapshot count/size |
| **FreeNAS / TrueNAS** | Pool and dataset monitoring |
| **Local disks** | sysinfo-based disk metrics (total, used, used%) |
| **NFS** | Network file system monitoring |

### UI Features

- Storage array dashboard with SAN/NAS/DAS filtering
- Capacity planning charts
- Utilization tracking with progress bars
- IOPS monitoring
- Vendor/model detail views
- Per-mount disk breakdown

---

## 9. Server & Infrastructure Monitoring

### Operating Systems

| OS | Collection Method | Metrics |
|----|------------------|---------|
| **Linux** | SSH (agentless) | CPU%, memory (total/used/swap/available), disk per-mount (total/used/%), process count, top processes, file descriptors, network sockets, load averages, swap activity |
| **Windows** | WinRM/WMI | Performance counters (CPU, memory, disk, network) |
| **FreeBSD** | SSH | System metrics |

### Virtualization & Containers

| Platform | Metrics |
|----------|---------|
| **vSphere** | VM CPU/memory allocation, guest metrics |
| **ESXi** | Host-level metrics |
| **vCenter** | Cluster-wide management |
| **Kubernetes** | Node/pod resource usage, restart counts |
| **Docker** | Container monitoring |

### Hardware / BMC

| Interface | Capabilities |
|-----------|-------------|
| **Redfish** | Hardware inventory, power state, thermal status (iDRAC/iLO) |
| **IPMI** | Hardware monitoring |
| **SNMP** | Device polling |
| **S.M.A.R.T.** | Disk health metrics via SSH |

### UI Features

- Server fleet view with CPU/memory/disk sparklines
- Modal-based server drilldowns
- SSH key management interface
- Process and user tracking
- Infrastructure dashboard with KPI strip
- Resource utilization gauges

---

## 10. Security & Threat Detection

### IDS/IPS Engine

| Feature | Implementation |
|---------|---------------|
| Rule engine | Suricata/Snort-style rule processing |
| Event parsing | EVE JSON event ingestion |
| Alert thresholding | Max 100 alerts per SID per 5-minute window |
| Severity classification | Critical / High / Medium / Low |
| Status tracking | New → Acknowledged → Closed / False Positive |
| Community ID | RFC 7539 flow correlation |

### Protocol Analyzers

| Protocol | Analysis |
|----------|---------|
| **DNS** | Query/response analysis, anomaly detection, DNS tunneling detection |
| **HTTP** | Method/status/header analysis |
| **SSL/TLS** | Certificate chain validation, JA3/JA3s fingerprinting, cipher analysis |
| **SSH** | Auth attempts, client/server identification, HASSH fingerprinting |

### Firewall Integration

| Firewall | Features |
|----------|---------|
| **iptables** | Log parsing and normalization |
| **firewalld** | Event extraction |
| **pf (OpenBSD)** | Packet filter log analysis |
| All | Action tracking (accept/drop/reject), rule name, interface |

### GeoIP & Threat Intelligence

- IP geolocation enrichment for threat mapping
- Network blacklist with source tracking (manual, threat_engine, ids_rule)
- Top attack source identification

### Database Schema

- `ids_rules` — IDS rules with SID, category, severity, rule text
- `ids_alerts` — IDS alerts with src/dst IP, protocol, payload, community_id
- `connection_logs` — Connection tracking with geo enrichment
- `dns_logs` — DNS query/response logs
- `http_logs` — HTTP request/response logs
- `ssl_logs` — TLS handshake logs with JA3/JA3s, cert chain
- `ssh_logs` — SSH authentication logs with HASSH

### UI Features

- Security dashboard with IDS/IPS statistics
- Alert severity distribution charts
- Top attack source visualization
- Threat hunting with unified search across IDS alerts, connections, DNS
- Firewall rule and event monitoring page
- GeoMap for geographic threat visualization

---

## 11. Middleware & Service Monitoring

### Supported Middleware (12 Types)

| Service | Monitoring Capabilities |
|---------|------------------------|
| **Nginx** | Dedicated monitoring page, connection stats, request rates |
| **Apache** | Scoreboard metrics, performance monitoring |
| **HAProxy** | Backend/frontend stats, server health |
| **Tomcat** | JVM metrics, thread pools, request processing |
| **PHP-FPM** | Worker pool stats, request queuing |
| **RabbitMQ** | Queue depth, message rates, connection count |
| **Kafka** | Topic metrics, consumer lag, broker stats |
| **Elasticsearch** | Cluster health, index stats, query performance |
| **Redis** | Memory, connections, command stats |
| **Memcached** | Cache hit/miss rates, memory usage |
| **Docker** | Container status, resource usage |
| **Kubernetes** | Pod/node health, resource utilization |

### Background Collectors

The API server runs dedicated background metric collectors for middleware services, polling at configurable intervals and feeding metrics into the ingestion pipeline.

### UI Features

- MiddlewareMonitoring dashboard with all 12 service types
- Per-service detail pages with status and history tabs
- Dedicated Nginx and Apache deep monitoring pages

---

## 12. Cloud Monitoring (AWS / Azure / GCP)

### AWS

| Service | Metrics |
|---------|---------|
| EC2 | CPU%, NetworkIn/Out, DiskReadOps, DiskWriteOps, StatusCheck |
| RDS | CPU%, Connections, FreeStorageSpace, IOPS (read/write), Latency, FreeableMemory, ReplicaLag |
| ELB/ALB | RequestCount, TargetResponseTime, HTTPCode_5XX/4XX, HealthyHostCount |
| Lambda | Invocations, Errors, Duration(ms), Throttles, ConcurrentExecutions |
| ECS/EKS | CPU%, Memory% |

**Auth:** SigV4-signed HTTP requests (no SDK dependency)

### Azure

| Service | Metrics |
|---------|---------|
| VMs | CPU%, Network In/Out, Disk Read/Write (bytes/ops), Available Memory |
| SQL Database | CPU%, DTU%, Storage%, Connection Success/Failed, Deadlocks, Worker% |
| AKS | Node CPU/Memory%, Pod/Node Count, Node Ready Status |
| App Service | CPU time, Requests, HTTP 4xx/5xx, Response time, Memory |
| Storage | Storage account metrics |
| Load Balancer | Backend health, throughput |

**Auth:** OAuth2 client credentials flow

### GCP

| Service | Metrics |
|---------|---------|
| GCE | CPU%, Network bytes (in/out), Disk ops/bytes, Memory |
| Cloud SQL | CPU%, Memory%, Disk%, Connections, Read/Write ops, Replication lag |
| GKE | Node CPU cores, Memory bytes, Pod volumes |
| Cloud Run | Request count/latency, CPU%, Memory%, Instance count |
| Load Balancer | Request distribution, latency |

**Auth:** JWT service account authentication

---

## 13. AI, ML & Predictive Analytics

### Anomaly Detection (zentra-ml)

| Algorithm | Description |
|-----------|------------|
| **Z-Score** | Rolling standard deviation & mean with configurable sigma threshold |
| **EWMA** | Exponential Weighted Moving Average with adaptive thresholding (alpha=0.1) |
| **Seasonal** | Daily/weekly seasonality pattern learning and deviation detection |

Implementation: In-memory state per metric (RwLock + HashMap), periodic eviction of stale metrics (>1 hour, every 10 minutes), persists anomalies to database, publishes AnomalyDetected events.

### Predictive Analytics (zentra-predict)

| Feature | Description |
|---------|------------|
| **Baselines** | 7-day rolling window with per-metric mean & standard deviation |
| **Forecasting** | Linear regression trend extrapolation, predicts hours_to_threshold |
| **K-Means Clustering** | Unsupervised anomaly detection, groups similar metrics, flags outliers |
| **Recording Rules** | Pre-compute aggregations (avg, sum, p50, p95, p99, rate, stddev) |
| **Trend Alerts** | Alert types: capacity_exhaustion, anomaly_spike, degradation_trend, resource_contention |

### AI Plugin Features

- Prompt injection detection (regex-based)
- PII scanning (SSN, credit card, phone, email)
- Toxicity scoring (violence, hate speech, self-harm, illegal activity)
- Hallucination detection (overlap ratio + Z-score)

### UI Features

- AIInsights page with ML model overview (IsolationForest, RandomForest, KMeans, GradientBoosting)
- Performance trend charts
- Cost optimization tracking ($5k+ savings)
- PredictiveAlerts page with 3 tabs: anomalies, forecasts, trend alerts
- RCA (Root Cause Analysis) with error pattern detection, Stack Overflow/docs solution recommendations, confidence scoring
- Floating AI chatbot with MCP tool calling integration

### Database Schema

- `metric_baselines` — Rolling statistics (mean, stddev, min, max, sample count)
- `predictions` — Forecasts with predicted_value, rate_per_hour, hours_to_threshold, confidence
- `anomaly_events` — Detected anomalies with observed_value, expected values, z_score, severity
- `trend_alerts` — Predictive alerts with current/predicted values, threshold, hours_to_breach, recommendations (JSONB)
- `recording_rules` — Pre-aggregation rules (source→output metric, aggregation function, window, group_by)

---

## 14. Alerting & Notification Engine

### Alert Engine (zentra-stream)

| Feature | Implementation |
|---------|---------------|
| State machine | Pending → Triggered → Acknowledged → Resolved / Suppressed |
| Sliding windows | Per (rule_id, resource_id) metric aggregation |
| Aggregations | avg, max, min, sum, count, p95, p99 |
| Operators | GreaterThan, LessThan, Equal |
| Hysteresis | Recovery threshold ≠ trigger threshold (prevents flapping) |
| Delayed triggering | `for_duration_seconds` before transition to Triggered |
| Repeat notifications | `repeat_interval_seconds` for periodic re-notification |
| Predictive integration | Baseline, anomaly, forecast, and trend alert engines feed into alert evaluation |

### Alert Dependencies

- Parent-child rule relationships
- Suppress child alerts when parent is not triggered
- Multi-level hierarchy support
- Prevents alert storms in cascading failures

### Escalation Engine

- Multi-level escalation chains
- Per-level: wait_minutes + notifier list
- Auto-escalate on unacknowledged alerts
- Repeat until acknowledged or max level reached

### Notification Channels (8 Types)

| Channel | Integration |
|---------|------------|
| **Email** | SMTP |
| **Slack** | Webhook |
| **Microsoft Teams** | Adaptive Card |
| **PagerDuty** | Incident creation |
| **Telegram** | Bot API |
| **Discord** | Webhook |
| **Syslog** | Remote syslog |
| **Custom Webhook** | HTTP POST with HMAC-SHA256 signature, retry with backoff |

### Alert Management Features

- Notification policies with channel routing
- Mute timings (maintenance windows)
- Inhibition rules (suppress alerts based on conditions)
- Alert grouping
- Backtesting (test rules against historical data)
- 10 built-in alert rule templates (CPU, memory, disk, service, error rate, etc.)
- Alert silences
- On-call schedules

### Database Schema

- `alert_rules` — Rule definitions with condition (JSONB), window, severity
- `alerts` — Active/historical alerts with status tracking
- `alert_routing` — Rule-to-channel mapping with config (JSONB)
- `alert_state_log` — Append-only state transitions (triggered→acknowledged, etc.)

---

## 15. Log Engine & Analytics

### Multi-Format Parsing (zentra-log-engine)

| Format | Features |
|--------|---------|
| **Plain Text** | Raw line parsing |
| **JSON** | Structured field extraction |
| **Syslog** | RFC3164 format parsing |
| **PostgreSQL CSV** | Database-specific fields (severity, database, user) |
| **Custom Regex** | User-defined patterns with ReDoS protection, regex caching |

### Log Processing

| Feature | Implementation |
|---------|---------------|
| Severity inference | Automatic detection from raw log content |
| Full-text search | PostgreSQL tsvector + trigram indexes |
| Log correlation | Link related log entries, root cause finding |
| Log sources | File tailing, Journald, Syslog socket, PostgreSQL log, API polling |
| Loki compatibility | `/loki/api/v1/push` endpoint for Loki-compatible ingestion |
| LogQL support | Log query language for complex searches |
| SSE tailing | Server-Sent Events for live log streaming |

### Database Schema

- `logs` — TimescaleDB hypertable (1-day chunks), 30-day retention, compression after 6 hours
- `message_tsv` — tsvector generated column for full-text search
- `idx_logs_message_fts` — GIN index on tsvector
- `idx_logs_message_trgm` — Trigram index for LIKE queries

### UI Features

- LogExplorer with 4 tabs: LogQL query, stream management, alert config, analytics
- LogSources page: 6 source types (file, syslog, Docker, CloudWatch)
- LogAlerts: Log-based alert rules with matcher expressions
- LogAnalytics: Volume by source (pie chart), error rate trends (area chart), top errors table
- Live log viewer with scrolling panel

---

## 16. Dashboard & Visualization System

### Dashboard Features

| Feature | Implementation |
|---------|---------------|
| Dashboard CRUD | Create, read, update, delete dashboards |
| Versioning | Full version history with version_number and diff |
| Folders | Hierarchical folder organization (self-referencing parent_id) |
| Panels | 9 panel types: timeseries, gauge, stat, table, barchart, piechart, heatmap, logs, text |
| Library panels | Reusable panels shared across dashboards (usage tracking) |
| Template variables | 6 variable types: query, custom, interval, textbox, constant, datasource |
| Snapshots | Dashboard state capture |
| Annotations | 6 types: manual, deployment, alert, incident, maintenance, change |
| Playlists | Auto-rotating dashboard playlists |
| Bookmarks | Saved dashboard shortcuts |
| Query history | Per-user query tracking with starring |
| Export/Import | Dashboard portability |
| Data transforms | ETL pipeline with step configuration (JSONB) |

### Prometheus Compatibility

| Feature | Implementation |
|---------|---------------|
| Remote write | `/v1/write` endpoint for Prometheus remote write protocol |
| PromQL | Parser & evaluator (binary operators, aggregations, range functions) |
| Instant queries | `/v1/query` for point-in-time queries |
| Range queries | Bucketed aggregation with auto-interval calculation |
| Label discovery | `/v1/labels` and label_values endpoints |
| Metric metadata | Type and help text discovery |
| Scrape targets | HTTP metric scraping with Prometheus text format parser |
| Recording rules | Pre-computed aggregations to reduce query load |
| Target discovery | Static, FileSD, Consul, Kubernetes, Custom HTTP |

### Auto-Bucketing Strategy

| Time Range | Bucket Size |
|-----------|-------------|
| ≤ 1 hour | 10 seconds |
| ≤ 6 hours | 1 minute |
| ≤ 1 day | 5 minutes |
| ≤ 7 days | 30 minutes |
| ≤ 30 days | 2 hours |
| > 30 days | 12 hours |

---

## 17. Authentication, RBAC & Multi-Tenancy

### Authentication

| Feature | Implementation |
|---------|---------------|
| JWT tokens | Bearer token authentication |
| Refresh tokens | Automatic token refresh with 401 retry |
| MFA/2FA | Multi-factor authentication setup and verification |
| Password hashing | bcrypt |
| Brute-force protection | Login attempt tracking, account locking (locked_until) |
| Password reset | Token-based reset flow |
| Session management | Token hash + refresh token hash, IP/user-agent tracking, expiry |
| API tokens | Service account keys with key_hash, key_prefix display, role assignment, expiry |

### RBAC (Role-Based Access Control)

| Component | Implementation |
|-----------|---------------|
| Roles | System roles + custom roles per org |
| Permissions | Resource × Action matrix (dashboard/alert/target/user/org/servers/network/storage × read/write/delete/admin/acknowledge) |
| Role-permission mapping | Many-to-many (role_permissions table) |
| User-role assignment | Per-org assignment with audit (assigned_by, assigned_at) |
| User groups | Group hierarchy with member management |
| UI enforcement | RequireAuth component (route-level), PermissionGate component (element-level), role-aware CommandPalette |

### Multi-Tenancy

| Feature | Implementation |
|---------|---------------|
| Org isolation | org_id column on all tables |
| OrgId extractor | Axum middleware extracts org from JWT |
| Row-Level Security | PostgreSQL RLS policies (org_id = current_setting) on targets, metrics, logs, alerts, dashboards |
| Org settings | Per-org limits (max_targets, max_dashboards, max_alert_rules, max_users, retention_days, feature flags) |
| Demo mode | Per-tenant demo mode toggle |

### Audit Logging

- `audit_log` — TimescaleDB hypertable, append-only, 365-day retention
- Tracks: action (dashboard.create, alert.delete, etc.), resource_type/id, user_id, IP address, user agent, status (success/denied/error)

---

## 18. License Management

### Core Features

| Feature | Implementation |
|---------|---------------|
| License types | per_seat, per_device, site, enterprise, concurrent, subscription |
| Categories | software, hardware, cloud, saas, perpetual |
| Expiry tracking | Daily scan, 90-day warning / 30-day critical thresholds, auto-expire |
| Seat management | Checkout/checkin tracking, assignment to users/devices/services |
| Compliance | Utilization%, over/under-licensed detection, wasted cost calculation, org-level compliance score |
| Audit trail | Immutable log: created, updated, deleted, seat_assigned, renewed, expired |
| Cost tracking | cost_per_seat, total_cost, currency, wasted cost |

### Database Views

- `license_compliance_summary` — Joined view with total/assigned/free seats, compliance_status (over_licensed/fully_utilized/under_licensed/ok)
- `licenses_expiring_soon` — Active licenses expiring within 90 days

---

## 19. Database Schema — Full Inventory

### TimescaleDB Hypertables

| Table | Chunk Interval | Retention | Compression |
|-------|---------------|-----------|-------------|
| `metrics` | 1 day | 90 days | After 2 hours (segment: org_id, target_id, metric_name) |
| `logs` | 1 day | 30 days | After 6 hours (segment: org_id, severity, source) |
| `network_flows` | 1 day | 90 days | After 1 day (segment: org_id, protocol, direction) |
| `events` | 1 day | 7 days | After 1 day (segment: event_type) |
| `threats` | 1 day | 90 days | After 1 day (segment: org_id, category, severity) |
| `audit_log` | — | 365 days | — |

### Continuous Aggregates

| Aggregate | Source | Bucket |
|-----------|--------|--------|
| `metrics_1m` | metrics | 1 minute |
| `metrics_5m` | metrics | 5 minutes |
| `metrics_1h` | metrics | 1 hour |
| `metrics_1d` | metrics | 1 day |
| `netflow_1m` | network_flows | 1 minute |
| `netflow_1h` | network_flows | 1 hour |
| `netflow_topn_talkers_hourly` | network_flows | Hourly top talkers |

### Complete Table List (60+)

**Core:**
`targets`, `metrics`, `logs`, `events`, `threats`

**Alerts:**
`alert_rules`, `alerts`, `alert_routing`, `alert_state_log`

**Predictions & ML:**
`metric_baselines`, `predictions`, `anomaly_events`, `trend_alerts`, `recording_rules`

**Dashboards:**
`dashboards`, `dashboard_panels`, `dashboard_variables`, `dashboard_folders`, `dashboard_versions`, `library_panels`, `library_panel_usage`, `data_transforms`, `annotations`

**Scraping:**
`scrape_targets`, `scrape_results`, `scrape_recording_rules`

**Licenses:**
`licenses`, `license_seats`, `license_assignments`, `license_audit_log`

**Auth & RBAC:**
`users`, `user_sessions`, `login_attempts`, `user_groups`, `user_group_members`, `roles`, `permissions`, `role_permissions`, `user_roles`, `api_keys`, `org_settings`

**Network Security:**
`ids_rules`, `ids_alerts`, `connection_logs`, `dns_logs`, `http_logs`, `ssl_logs`, `ssh_logs`

**Network Flows:**
`network_flows`, `flow_exporters`, `network_favorites`, `network_blacklist`

**Oracle:**
`oracle_connections`, `oracle_metric_definitions`, `oracle_metric_cache`

**Views:**
`license_compliance_summary`, `licenses_expiring_soon`

### Key Indexes

| Table | Index Strategy |
|-------|---------------|
| targets | org_id, (org_id, type), (org_id, active), status |
| metrics | (org_id, metric_name, timestamp DESC), (target_id, metric_name, timestamp DESC), (resource_id, metric_name, timestamp DESC), plugin_id, resource_type |
| logs | (org_id, timestamp DESC), (target_id, timestamp DESC), (severity, timestamp DESC), (source, timestamp DESC), message_fts (GIN), message_trgm (trigram) |
| alerts | (org_id, status), (org_id, severity), triggered_at DESC, target_id, rule_id |
| network_flows | (exporter_ip, timestamp DESC), src_ip, dst_ip, protocol |
| ids_alerts | timestamp DESC, src_ip, dest_ip, severity, status, community_id |

### Row-Level Security

RLS policies enforced on: `targets`, `metrics`, `logs`, `alerts`, `alert_rules`, `dashboards`  
Policy: `org_id = current_setting('app.current_org_id')::uuid`

---

## 20. Deployment & Infrastructure

### Production Architecture

```
Production Server (10.88.11.162)
├── /store/Zentra/
│   ├── bin/zentra-server (release binary, 3 backup rotation)
│   ├── frontend/dist/ (React SPA)
│   ├── migrations/ (SQL files)
│   ├── logs/ (api.log, frontend.log, nginx_*.log)
│   └── .env (configuration)
│
├── systemd services
│   ├── zentra-api.service → port 5006
│   └── zentra-frontend.service → port 5010
│
├── Nginx reverse proxy (port 5010)
│   ├── / → React SPA (try_files)
│   ├── /api/* → backend proxy (127.0.0.1:5006)
│   ├── /api/auth/login → rate limited (5 req/min/IP)
│   └── /metrics → basic auth + IP whitelist
│
└── PostgreSQL 16 + TimescaleDB (port 5434)

Oracle Instance (10.88.218.59)
├── TNS Listener (port 1521)
├── RMAN backup monitoring
└── Data Guard / RAC
```

### Kubernetes Deployment

| Resource | Configuration |
|----------|---------------|
| Namespace | zentra |
| API Deployment | 2 replicas, 512Mi-2Gi memory, 500m-2000m CPU |
| Frontend Deployment | 2 replicas, nginx-alpine |
| Liveness probe | GET /api/health (10s delay, 30s period) |
| Readiness probe | GET /api/health (5s delay, 10s period) |
| Secrets | database-url, jwt-secret, oracle-dsn/user/password |
| ConfigMaps | RUST_LOG, ports, Oracle/PostgreSQL/MySQL metric TOML files |
| Docker (API) | rust:1.75-slim builder → debian:bookworm-slim runtime |
| Docker (Frontend) | node:20-alpine builder → nginx:alpine |

### Nginx Security Headers

- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: SAMEORIGIN`
- `X-XSS-Protection: 1; mode=block`
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Content-Security-Policy: default-src 'self'`
- `Permissions-Policy: camera=(), microphone=(), geolocation=()`
- Sensitive file blocking (.env, .git, .toml, .rs, .sql, .sh)

### Deployment Scripts

| Script | Purpose |
|--------|---------|
| `deploy.sh` | Full deployment (backend + frontend) via SCP/SSH |
| `setup_server.sh` | First-time server setup (user, dirs, systemd units) |
| `run-migrations.sh` | Execute database migrations (7 SQL files in sequence) |
| `restart-services.sh` | Kill/restart processes with health check verification |
| `full-test.sh` | Comprehensive endpoint test suite (50+ endpoints) |
| `smoke-test.sh` | Quick health validation |
| `backup-db.sh` | PostgreSQL backup to timestamped file |
| `cleanup-postgres-db.sh` | Old record cleanup + vacuum analyze |
| `check-docker.sh` | Docker environment probe (SSH to Oracle host) |
| `start-oracle.sh` | Oracle container/process startup |
| `test-oracle.sh` | Oracle TNS/SQL*Plus connectivity test |
| `pentest.ps1` | Windows penetration testing |

### Port Configuration

| Service | Port |
|---------|------|
| REST API | 5006 |
| Frontend (Vite dev) | 5010 |
| Agent | 5001 |
| Prometheus metrics | 5002 |
| PostgreSQL | 5434 |
| Oracle TNS | 1521 |
| NetFlow | 2055 |
| IPFIX | 4739 |
| sFlow | 6343 |

---

## 21. API Surface — Complete Endpoint Map

### Core System
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/health` | System health check |
| GET | `/api/system/status` | System status |
| GET | `/api/system/db` | Database status |

### Authentication
| Method | Endpoint | Handler |
|--------|----------|---------|
| POST | `/api/auth/login` | Email/password login |
| POST | `/api/auth/logout` | Logout |
| POST | `/api/auth/mfa/setup` | MFA setup |
| POST | `/api/auth/mfa/verify` | MFA verification |
| GET | `/api/auth/sessions` | List sessions |
| POST | `/api/auth/change-password` | Change password |
| POST | `/api/auth/forgot-password` | Forgot password |
| POST | `/api/auth/reset-password` | Reset password |

### Targets
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET/POST | `/api/targets` | List/Create targets |
| GET/PUT/DELETE | `/api/targets/:id` | CRUD target |
| POST | `/api/targets/:id/test` | Test connection |
| GET | `/api/targets/stats` | Target statistics |
| GET/POST | `/api/scrape/targets` | Scrape target CRUD |
| GET | `/api/service-discovery/*` | Service discovery |
| GET | `/api/collectors` | Collector status |

### Metrics (Prometheus-Compatible)
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/metrics` | Query metrics |
| GET | `/api/metrics/aggregated` | Aggregated metrics |
| POST | `/api/v1/write` | Prometheus remote write |
| GET/POST | `/api/v1/query` | PromQL instant query |
| GET/POST | `/api/v1/query_range` | PromQL range query |
| GET | `/api/v1/labels` | Label discovery |
| GET | `/api/v1/label/:name/values` | Label values |
| GET | `/api/v1/metadata` | Metric metadata |
| GET | `/api/v1/series` | Series discovery |

### Logs
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/logs` | Search logs |
| POST | `/api/logs/query` | LogQL query |
| POST | `/api/loki/api/v1/push` | Loki-compatible push |
| GET/POST | `/api/log-alerts` | Log alert rules |
| GET/POST | `/api/discovery-rules` | Log discovery rules |
| GET | `/api/logs/tail` | SSE live log tailing |

### Alerts
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/alerts` | List alerts |
| PUT | `/api/alerts/:id/acknowledge` | Acknowledge alert |
| PUT | `/api/alerts/:id/resolve` | Resolve alert |
| GET/POST | `/api/alert-rules` | Alert rule CRUD |
| GET/POST | `/api/notifications/*` | Notification channels & policies |
| GET/POST | `/api/alerts/silences` | Alert silences |
| GET/POST | `/api/alerts/inhibitions` | Inhibition rules |

### Dashboards
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET/POST | `/api/dashboards` | Dashboard CRUD |
| GET | `/api/dashboards/:id/versions` | Version history |
| POST | `/api/dashboards/export` | Export dashboard |
| POST | `/api/dashboards/import` | Import dashboard |
| GET/POST | `/api/library-panels` | Reusable panels |
| GET/POST | `/api/snapshots` | Dashboard snapshots |
| GET/POST | `/api/query-history` | Query history |
| GET/POST | `/api/annotations` | Annotations |

### Admin
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET/POST | `/api/users` | User CRUD |
| GET/POST | `/api/user-groups` | Group management |
| GET/POST | `/api/roles` | Role management |
| GET/POST | `/api/api-tokens` | API token management |
| GET/PUT | `/api/settings` | Org settings |
| GET | `/api/admin/licenses` | License management |
| GET | `/api/audit-log` | Audit log |

### Network
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET/POST | `/api/network/capture` | Packet capture control |
| GET | `/api/network/connections` | Connection tracking |
| GET | `/api/network/hosts` | Discovered hosts |
| GET | `/api/network/protocols` | Protocol breakdown |
| GET | `/api/network/bandwidth` | Bandwidth stats |
| GET | `/api/network/deep/*` | Deep analysis (snapshot, services, ICMP, threats) |
| POST | `/api/network/pcap/export` | PCAP export |
| POST | `/api/network/pcap/import` | PCAP import |

### Oracle
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET/POST | `/api/oracle/connections` | Oracle connection CRUD |
| GET | `/api/oracle/metrics` | Oracle metric scraping |
| GET | `/api/oracle/alert-log` | Oracle alert log |

### Databases
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/databases` | Multi-DB status |
| POST | `/api/databases/query` | Execute DB query |

### Security
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/security/ids/*` | IDS rules & alerts |
| GET | `/api/security/connections` | Connection logs |
| GET | `/api/security/dns` | DNS logs |
| GET | `/api/security/http` | HTTP logs |
| GET | `/api/security/ssl` | SSL/TLS logs |
| GET | `/api/security/threats` | Threat lists |

### Middleware
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/middleware/:service` | Service-specific metrics |
| GET | `/api/middleware/:service/status` | Service health |

### AI & MCP
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/ai/insights` | AI insights |
| POST | `/api/ai/chat` | AI chat (MCP/LLM/API modes) |
| GET | `/api/ai/chat/history` | Chat history |
| GET | `/api/mcp/tools` | List MCP tools |
| POST | `/api/mcp/call` | Execute MCP tool |

### Infrastructure
| Method | Endpoint | Handler |
|--------|----------|---------|
| GET | `/api/infra/*` | Templates, health checks, SLA, dependencies |
| GET/POST | `/api/runbooks` | Automation runbooks |

---

## 22. Implementation Statistics

### Codebase

| Metric | Count |
|--------|-------|
| Rust crates | 13 |
| Monitoring plugins | 8 |
| Total Rust packages | 21 |
| API route modules | 18 |
| API handler files | 89 |
| Frontend pages | 22 |
| Frontend components | 20+ |
| Chart types | 25 |
| API client modules | 17 |
| Frontend routes | 25+ |
| Database migrations | 24 |
| Database tables | 60+ |
| Database views | 2 |
| TimescaleDB hypertables | 6 |
| Continuous aggregates | 7 |
| Deployment scripts | 15+ |

### Monitoring Coverage

| Domain | Subtypes/Systems |
|--------|-----------------|
| Databases | Oracle (17-tab deep), PostgreSQL, MySQL, MariaDB, MongoDB, Redis |
| Storage | NetApp, Pure Storage, Dell EMC (3), Ceph, MinIO, Veeam, ZFS, local disk |
| Servers | Linux, Windows, FreeBSD, vSphere/ESXi/vCenter, K8s, Docker, SNMP, IPMI, Redfish |
| Network | Switch, Router, Firewall, BGP, OSPF, VPN, DNS, DHCP, NetFlow v5/v9, IPFIX, sFlow |
| Cloud | AWS (EC2, RDS, ELB, Lambda, ECS, EKS), Azure (VM, SQL, AKS, AppService, Storage, LB), GCP (GCE, SQL, GKE, Run, LB) |
| Middleware | Nginx, Apache, HAProxy, Tomcat, PHP-FPM, RabbitMQ, Kafka, Elasticsearch, Redis, Memcached, Docker, Kubernetes |
| Security | IDS/IPS, DNS analysis, HTTP analysis, SSL/TLS (JA3), SSH (HASSH), Firewall (iptables/firewalld/pf), GeoIP |
| AI/ML | Z-Score, EWMA, Seasonal, K-Means, Linear Regression, Baselines, Forecasts, Trend Alerts |

### Enterprise Features

| Feature | Status |
|---------|--------|
| Multi-tenancy | ✅ Implemented (org_id isolation) |
| Row-Level Security | ✅ Implemented (PostgreSQL RLS policies) |
| RBAC | ✅ Implemented (roles, permissions, groups) |
| JWT + MFA | ✅ Implemented |
| Audit logging | ✅ Implemented (365-day retention) |
| API tokens | ✅ Implemented |
| License compliance | ✅ Implemented |
| Dashboard versioning | ✅ Implemented |
| PromQL compatibility | ✅ Implemented |
| Prometheus remote write | ✅ Implemented |
| Alert escalation chains | ✅ Implemented |
| 8-channel notifications | ✅ Implemented |
| Alert dependencies | ✅ Implemented |
| Predictive alerting | ✅ Implemented |
| Anomaly detection (4 algorithms) | ✅ Implemented |
| Capacity forecasting | ✅ Implemented |
| 70+ runbooks | ✅ Implemented |
| Dark mode | ✅ Implemented |
| Command palette | ✅ Implemented |
| AI chatbot (MCP) | ✅ Implemented |
| Drill-down navigation (10 levels) | ✅ Implemented |
| SQL workspace | ✅ Implemented |
| OpenAPI documentation | ✅ Implemented |
| Kubernetes deployment | ✅ Implemented |
| PCAP export/import | ✅ Implemented |
| NetFlow/sFlow/IPFIX | ✅ Implemented |
| Live log streaming (SSE) | ✅ Implemented |
| LogQL queries | ✅ Implemented |
| Full-text search (tsvector + trigram) | ✅ Implemented |
| TimescaleDB continuous aggregates | ✅ Implemented |
| Data compression policies | ✅ Implemented |
| Rate limiting | ✅ Implemented |
| CORS | ✅ Implemented |
| Security headers (CSP, HSTS) | ✅ Implemented |

---

*This document was auto-generated from a complete analysis of the Zentra codebase on 13 May 2026.*
