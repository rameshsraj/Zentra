import {
  Document, Packer, Paragraph, TextRun, HeadingLevel, Table, TableRow, TableCell,
  WidthType, AlignmentType, PageBreak, Header, Footer,
  ShadingType, TableLayoutType, convertInchesToTwip
} from "docx";
import { writeFileSync } from "fs";

// ── Styling ────────────────────────────────────────────────────────────
const F = "Calibri", CF = "Consolas", BL = "1F4E79", DK = "2D2D2D", WH = "FFFFFF", ALT = "EDF2F7";

function h1(t) { return new Paragraph({ heading: HeadingLevel.HEADING_1, spacing: { before: 300, after: 120 }, children: [new TextRun({ text: t, font: F, bold: true, color: BL })] }); }
function h2(t) { return new Paragraph({ heading: HeadingLevel.HEADING_2, spacing: { before: 240, after: 100 }, children: [new TextRun({ text: t, font: F, bold: true, color: BL })] }); }
function h3(t) { return new Paragraph({ heading: HeadingLevel.HEADING_3, spacing: { before: 200, after: 80 }, children: [new TextRun({ text: t, font: F, bold: true, color: BL })] }); }
function p(t, o = {}) { return new Paragraph({ spacing: { after: 80 }, ...o.p, children: [new TextRun({ text: t, font: F, size: 22, color: DK, ...o })] }); }
function b(t, d, l = 0) { return new Paragraph({ bullet: { level: l }, spacing: { after: 40 }, children: d ? [new TextRun({ text: t + " — ", font: F, size: 22, bold: true, color: DK }), new TextRun({ text: d, font: F, size: 22, color: DK })] : [new TextRun({ text: t, font: F, size: 22, color: DK })] }); }
function cb(t) { const lines = t.split("\n"); return [new Paragraph({ spacing: { before: 80 }, children: [] }), ...lines.map(l => new Paragraph({ spacing: { after: 0 }, shading: { type: ShadingType.SOLID, color: "F5F5F5", fill: "F5F5F5" }, indent: { left: convertInchesToTwip(0.3) }, children: [new TextRun({ text: l || " ", font: CF, size: 18, color: "333333" })] })), new Paragraph({ spacing: { after: 80 }, children: [] })]; }
function sp() { return new Paragraph({ spacing: { after: 60 }, children: [] }); }
function pb() { return new Paragraph({ children: [new PageBreak()] }); }
function tbl(hd, rw) {
  return new Table({ width: { size: 100, type: WidthType.PERCENTAGE }, layout: TableLayoutType.AUTOFIT, rows: [
    new TableRow({ tableHeader: true, children: hd.map(h => new TableCell({ shading: { type: ShadingType.SOLID, color: BL, fill: BL }, width: { size: Math.floor(100/hd.length), type: WidthType.PERCENTAGE }, children: [new Paragraph({ spacing: { after: 40 }, children: [new TextRun({ text: h, font: F, size: 20, bold: true, color: WH })] })] })) }),
    ...rw.map((r, i) => new TableRow({ children: r.map(c => new TableCell({ shading: i%2===1 ? { type: ShadingType.SOLID, color: ALT, fill: ALT } : undefined, children: [new Paragraph({ spacing: { after: 40 }, children: [new TextRun({ text: String(c), font: F, size: 20, color: DK })] })] })) }))
  ] });
}

function makeDoc(title, subtitle, date, children) {
  return new Document({
    creator: "Zentra Team", title, description: subtitle,
    sections: [{ properties: { page: { margin: { top: convertInchesToTwip(0.8), bottom: convertInchesToTwip(0.8), left: convertInchesToTwip(0.9), right: convertInchesToTwip(0.9) } } },
      headers: { default: new Header({ children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun({ text: `${title}  •  v1.0.0`, font: F, size: 16, color: "AAAAAA", italics: true })] })] }) },
      footers: { default: new Footer({ children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "Internal Use Only — Zentra Team", font: F, size: 16, color: "AAAAAA" })] })] }) },
      children }]
  });
}

function titlePage(main, sub, date) {
  return [
    new Paragraph({ spacing: { before: 3000 }, children: [] }),
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 200 }, children: [new TextRun({ text: "ZENTRA", font: F, size: 72, bold: true, color: BL })] }),
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 100 }, children: [new TextRun({ text: main, font: F, size: 44, color: BL })] }),
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 60 }, children: [new TextRun({ text: `Version 1.0.0  •  ${date}`, font: F, size: 24, color: "666666" })] }),
    new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 60 }, children: [new TextRun({ text: sub, font: F, size: 22, color: "999999", italics: true })] }),
    pb()
  ];
}

// ═══════════════════════════════════════════════════════════════════════
// DOCUMENT 1: Main Operations Runbook (with Lab section)
// ═══════════════════════════════════════════════════════════════════════
function buildMainRunbook() {
  const c = [];
  c.push(...titlePage("Operations Runbook", "Internal Use Only — Learning & Operations", "May 2026"));

  // TOC
  c.push(h1("Table of Contents"));
  ["1.  Project Overview","2.  Architecture","3.  Technology Stack","4.  Crate & Plugin Structure","5.  Environment Configuration",
   "6.  Lab Components — IP & Port Reference","7.  Docker Deployment","8.  Docker Image Hardening","9.  License & Trial Period",
   "10. Authentication & RBAC","11. API Reference","12. Frontend Modules","13. OpenTelemetry Configuration",
   "14. Database Schema","15. Monitoring Targets","16. Alerting System","17. AI & Analytics","18. Security Module",
   "19. Network Monitoring","20. Middleware Monitoring","21. Connection Pooling","22. Backup & Recovery","23. Troubleshooting"
  ].forEach(t => c.push(p(t, { p: { spacing: { after: 60 } } })));
  c.push(pb());

  // 1
  c.push(h1("1. Project Overview"));
  c.push(p("Zentra is an all-in-one infrastructure monitoring platform built with a Rust backend and React frontend. It runs as a single Docker container that includes:"));
  c.push(b("Backend API", "Rust/Axum on port 5006 (internal)"));
  c.push(b("Frontend SPA", "React/Vite served by Nginx on port 5010"));
  c.push(b("Database", "PostgreSQL 16 + TimescaleDB (internal, port 5434)"));
  c.push(b("Process Manager", "Supervisord managing all three services"));
  c.push(sp()); c.push(p("The platform monitors servers, databases, middleware, network devices, storage arrays, and security events. It includes OpenTelemetry ingest, AI-powered analytics, and a built-in alerting engine."));
  c.push(pb());

  // 2
  c.push(h1("2. Architecture"));
  c.push(...cb(`┌─────────────────────────────────────────────────────┐
│              Docker Container (Rocky Linux 8)        │
│  ┌─────────┐    ┌──────────────┐   ┌────────────┐   │
│  │  Nginx   │───>│ zentra-server│──>│ PostgreSQL │   │
│  │  :5010   │    │   :5006      │   │   :5434    │   │
│  └─────────┘    └──────────────┘   └────────────┘   │
│                  ┌──────────────┐                    │
│                  │  Supervisord  │                   │
│                  └──────────────┘                    │
└─────────────────────────────────────────────────────┘`));
  c.push(pb());

  // 3
  c.push(h1("3. Technology Stack"));
  c.push(tbl(["Layer","Technology","Version"],[["Backend","Rust + Axum","1.95+ / Axum 0.7"],["Frontend","React + TypeScript + Vite","React 18"],["UI","TailwindCSS + Lucide Icons","v3"],["State","TanStack React Query","v5"],["Database","PostgreSQL + TimescaleDB","PG 16"],["Process Mgr","Supervisord","4.2"],["Proxy","Nginx","Latest"],["Container","Docker (Rocky Linux 8","—"],["Telemetry","OpenTelemetry SDK","0.27"],["Auth","JWT (HS256) + MFA (TOTP)","—"],["SSH","OpenSSH + sshpass","—"]]));
  c.push(pb());

  // 4
  c.push(h1("4. Crate & Plugin Structure"));
  c.push(h2("4.1 Backend Crates"));
  c.push(tbl(["Crate","Purpose"],[["zentra-api","Main Axum HTTP server, routes, handlers, middleware"],["zentra-core","Shared models, crypto, telemetry init, SSH pool"],["zentra-db","Database pool management, repository layer"],["zentra-stream","Event bus, notification routing, alert evaluation"],["zentra-agent","Standalone agent for remote metric collection"],["zentra-ingestion","OTLP payload parsing and batch ingest"],["zentra-query","Query services for metrics, logs, traces"],["zentra-scrape","Prometheus-compatible scrape targets"],["zentra-ml","ML model integration for anomaly detection"],["zentra-predict","Predictive alerting and capacity forecasting"],["zentra-log-engine","Log parsing and structured processing"],["zentra-netflow","NetFlow/sFlow/IPFIX collector"],["zentra-drilldown","Deep-dive analytics"]]));
  c.push(sp());
  c.push(h2("4.2 Plugins"));
  c.push(tbl(["Plugin","Purpose"],[["server-plugin","Linux/Windows metrics via SSH"],["network-plugin","SNMP polling, device discovery"],["database-plugin","PostgreSQL, Oracle, MySQL monitoring"],["storage-plugin","Dell, NetApp, Pure Storage arrays"],["security-plugin","IDS/IPS, firewall logs, threat detection"],["cloud-plugin","AWS, Azure, GCP monitoring"],["ai-plugin","AI/ML insights, anomaly detection"],["license-plugin","Third-party license tracking"]]));
  c.push(pb());

  // 5
  c.push(h1("5. Environment Configuration"));
  c.push(tbl(["Variable","Default","Description"],[["POSTGRES_USER","postgres","PostgreSQL superuser"],["POSTGRES_PASSWORD","Auto-generated","Database password"],["POSTGRES_PORT","5434","PostgreSQL port"],["ZENTRA_API_PORT","5006","Backend API port"],["ZENTRA_JWT_SECRET","Auto-generated","JWT signing secret (min 32 chars)"],["ZENTRA_ORG_ID","00000000-...0001","Default organization UUID"],["ZENTRA_ALLOW_ORIGINS","*","CORS allowed origins"],["ZENTRA_MASTER_KEY","Auto-generated","AES-256 key (64 hex chars)"],["RUST_LOG","info","Log verbosity"]]));
  c.push(sp());
  c.push(h2("5.1 OpenTelemetry Variables"));
  c.push(tbl(["Variable","Default","Description"],[["OTEL_EXPORTER_OTLP_ENDPOINT","—","OTLP collector endpoint"],["ZENTRA_OTEL_SELF_ENABLED","true","Enable self-monitoring"],["ZENTRA_OTEL_SAMPLE_RATIO","1.0","Trace sampling ratio"],["ZENTRA_OTLP_SAMPLE_RATIO","1.0","Receiver-side sampling"]]));
  c.push(pb());

  // 6 — LAB COMPONENTS
  c.push(h1("6. Lab Components — IP & Port Reference"));
  c.push(p("The Zentra project includes a full Datacenter Simulation Lab (docker/simulation/) that spins up 12 Docker containers simulating real infrastructure.", { bold: true }));
  c.push(sp());

  c.push(h2("6.1 Zentra Main Container"));
  c.push(tbl(["Component","IP Address","Port(s)","Protocol","Description"],[
    ["Zentra Platform (Frontend)","Host network","5010","HTTP","Main UI entry point (Nginx)"],
    ["API (internal)","127.0.0.1","5006","HTTP","Backend API — proxied by Nginx"],
    ["PostgreSQL (internal)","127.0.0.1","5434","TCP","Database — not exposed"],
    ["OTLP gRPC (internal)","0.0.0.0","4317","gRPC","OTel gRPC ingest"],
    ["OTLP HTTP (internal)","0.0.0.0","4318","HTTP","OTel HTTP ingest via /v1/*"],
  ]));
  c.push(sp());

  c.push(h2("6.2 Simulation Lab — Linux Host (10.88.11.x)"));
  c.push(tbl(["#","Component","Container","IP Address","Port(s)","Image","Role"],[
    ["1","Linux Server 01","srv-linux-01","10.88.11.201","22, 161","rockylinux:8","Web Server"],
    ["2","Linux Server 02","srv-linux-02","10.88.11.202","22, 161","ubuntu:22.04","App Server"],
    ["3","Windows Server 01","srv-win-01","10.88.11.203","22, 161","polinux/snmpd:alpine","AD/DNS"],
    ["4","Windows Server 02","srv-win-02","10.88.11.204","22, 161","polinux/snmpd:alpine","File Server"],
    ["5","Core Switch","sw-core-01","10.88.11.205","22, 161","polinux/snmpd:alpine","Cisco 9300"],
    ["6","Access Switch","sw-access-01","10.88.11.206","22, 161","polinux/snmpd:alpine","Arista 7050X"],
    ["7","Firewall","fw-01","10.88.11.207","22, 161","polinux/snmpd:alpine","Palo Alto PA-3260"],
    ["8","Storage Array","stor-01","10.88.11.208","22, 161","polinux/snmpd:alpine","Dell PowerStore"],
    ["9","Nginx","mw-nginx-01","10.88.11.209","22, 80, 8080","nginx:1.25-alpine","Reverse Proxy"],
    ["10","Apache","mw-apache-01","10.88.11.210","22, 80, 8080","httpd:2.4-alpine","HTTP Server"],
    ["11","Tomcat","mw-tomcat-01","10.88.11.211","22, 8080, 9090","tomcat:9-jdk17","App Server"],
    ["12","Oracle DB","db-oracle-01","10.88.11.212","22, 1521","gvenzl/oracle-xe:21-slim","Oracle 21c XE"],
  ]));
  c.push(sp());

  c.push(h2("6.3 Simulation Lab — Windows Host (10.100.0.x bridge)"));
  c.push(tbl(["#","Component","Container","IP Address","Port(s)","Role"],[
    ["1","Linux Server 01","srv-linux-01","10.100.0.11","22, 161","Rocky Linux 8"],
    ["2","Linux Server 02","srv-linux-02","10.100.0.12","22, 161","Ubuntu 22.04"],
    ["3","Windows Server 01","srv-win-01","10.100.0.13","22, 161","Windows 2022"],
    ["4","Windows Server 02","srv-win-02","10.100.0.14","22, 161","Windows 2019"],
    ["5","Core Switch","sw-core-01","10.100.0.15","22, 161","Cisco 9300"],
    ["6","Access Switch","sw-access-01","10.100.0.16","22, 161","Arista 7050X"],
    ["7","Firewall","fw-01","10.100.0.17","22, 161","Palo Alto"],
    ["8","Storage Array","stor-01","10.100.0.18","22, 161","Dell PowerStore"],
    ["9","Nginx","mw-nginx-01","10.100.0.19","22, 80, 8080","Reverse Proxy"],
    ["10","Apache","mw-apache-01","10.100.0.20","22, 80, 8080","HTTP Server"],
    ["11","Tomcat","mw-tomcat-01","10.100.0.21","22, 8080, 9090","App Server"],
    ["12","Oracle DB","db-oracle-01","10.100.0.22","22, 1521","Oracle 21c XE"],
  ]));
  c.push(sp());

  c.push(h2("6.4 Lab Credentials"));
  c.push(tbl(["Service","Username","Password","Notes"],[
    ["All containers (SSH)","root","zentra123","Port 22"],
    ["SNMP (all devices)","—","Community: public","SNMP v2c, port 161"],
    ["Tomcat Manager","zentra","zentra123","http://<ip>:8080/manager"],
    ["Nginx Status","—","—","http://<ip>:8080/nginx_status"],
    ["Apache Status","—","—","http://<ip>:8080/server-status"],
    ["Oracle (SYS)","SYS","zentra123","SID: XE, Service: ZENTRADB"],
    ["Oracle (Monitor)","zentra_mon","zentra123","Read-only monitoring user"],
    ["Zentra Web UI","admin@zentra.local","changeme","Port 5010"],
  ]));
  c.push(sp());

  c.push(h2("6.5 Adding Lab Targets to Zentra"));
  c.push(tbl(["Target Type","Where in Zentra","What to Enter"],[
    ["Linux/Windows Servers","Infrastructure → Servers → Add Target","IP, port 22, root/zentra123"],
    ["Network Switches","Infrastructure → Network → Add Device","IP, SNMP community 'public'"],
    ["Firewall","Infrastructure → Network → Add Device","IP, SNMP community 'public'"],
    ["Storage Array","Infrastructure → Storage → Add Array","IP, SSH root/zentra123"],
    ["Nginx","Middleware → Nginx","IP, http://<ip>:8080/nginx_status"],
    ["Apache","Middleware → Apache","IP, http://<ip>:8080/server-status"],
    ["Tomcat","Middleware → Tomcat","IP:8080, zentra/zentra123"],
    ["Oracle","Database → Oracle","IP:1521, SID XE, zentra_mon/zentra123"],
  ]));
  c.push(pb());

  // 7
  c.push(h1("7. Docker Deployment"));
  c.push(h2("7.1 Quick Start"));
  c.push(...cb(`docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 \\
  --security-opt=no-new-privileges:true \\
  -v zentra-data:/var/lib/postgresql/data \\
  zentra:latest`));
  c.push(p("Access at http://localhost:5010 — Default login: admin@zentra.local / changeme", { bold: true }));
  c.push(sp());
  c.push(h2("7.2 Production (Hardened)"));
  c.push(...cb(`docker run -d --name zentra --restart unless-stopped \\
  --read-only \\
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \\
  --tmpfs /var/run:rw,noexec,nosuid,size=16m \\
  --tmpfs /var/log:rw,noexec,nosuid,size=128m \\
  --tmpfs /opt/zentra/logs:rw,noexec,nosuid,size=64m \\
  --tmpfs /root/.ssh:rw,noexec,nosuid,size=4m \\
  --security-opt=no-new-privileges:true \\
  --cap-drop ALL \\
  --cap-add CHOWN --cap-add SETUID --cap-add SETGID \\
  --cap-add DAC_OVERRIDE --cap-add FOWNER \\
  --cap-add NET_BIND_SERVICE --cap-add LINUX_IMMUTABLE \\
  --env-file /opt/zentra/.env \\
  -p 5010:5010 -v zentra-data:/var/lib/postgresql/data zentra:latest`));
  c.push(pb());

  // 8
  c.push(h1("8. Docker Image Hardening"));
  c.push(tbl(["Measure","Details"],[
    ["Immutable files","Config, license, binary, migrations locked with chattr +i"],
    ["No shells","bash, sh, csh, zsh removed 5s after startup"],
    ["No package managers","dnf, yum, rpm removed at runtime"],
    ["No editors","vi, vim, nano, ed, perl, lua, ruby removed"],
    ["No dangerous tools","wget, ftp, telnet, dd, gcc, gdb removed"],
    ["chattr removed","After locking files, chattr itself deleted"],
    ["SSH preserved","sshpass, ssh, scp kept for monitoring"],
    ["Tamper protection","SHA-256 integrity manifest on every startup"],
    ["Non-root API","zentra-server runs as user 'zentra'"],
  ]));
  c.push(pb());

  // 9
  c.push(h1("9. License & Trial Period"));
  c.push(b("First boot","Entrypoint generates signed license token (3-day trial)"));
  c.push(b("Format","install_epoch:expiry_epoch:hostname:sha256_hmac"));
  c.push(b("Verification","HMAC against binary hash, independent of env vars"));
  c.push(b("After expiry","Container starts in DEGRADED mode — Dashboard only"));
  c.push(b("Renewal","Rebuild image, reset volume, or contact Zentra team"));
  c.push(pb());

  // 10
  c.push(h1("10. Authentication & RBAC"));
  c.push(b("Login","POST /api/auth/login → JWT + refresh token"));
  c.push(b("Storage","localStorage (Zentra_token, Zentra_refresh_token)"));
  c.push(b("Auto-refresh","Proactive refresh 5 min before expiry"));
  c.push(b("MFA","TOTP via /api/auth/mfa/setup and /api/auth/mfa/verify"));
  c.push(sp());
  c.push(h2("10.1 Two-Layer RBAC"));
  c.push(b("Layer 1","Module Access (which sidebar modules a user sees)"));
  c.push(b("Layer 2","Sub-Module Menu Items (dotted convention, e.g., infrastructure.servers)"));
  c.push(pb());

  // 11
  c.push(h1("11. API Reference"));
  c.push(h2("11.1 Health & System"));
  c.push(tbl(["Endpoint","Method","Description"],[
    ["/api/health","GET","Health check"],
    ["/api/system/status","GET","System status"],
    ["/api/system/license-status","GET","License + trial_expired flag"],
    ["/api/system/pool-status","GET","Connection pool stats"],
  ]));
  c.push(sp());
  c.push(h2("11.2 OTLP Ingest"));
  c.push(tbl(["Endpoint","Method","Description"],[
    ["/v1/traces","POST","OTLP trace ingest (JSON/Protobuf)"],
    ["/v1/metrics","POST","OTLP metrics ingest"],
    ["/v1/logs","POST","OTLP logs ingest"],
    ["/v1/profiles","POST","OTLP profiles ingest"],
    [":4317","gRPC","gRPC OTLP ingest (all signals)"],
  ]));
  c.push(pb());

  // 12
  c.push(h1("12. Frontend Modules"));
  c.push(tbl(["Module","Route","Sub-pages"],[
    ["Command Center","/dashboard","Overview dashboard"],
    ["Infrastructure","/infrastructure","Servers, Storage, Network, eBPF"],
    ["Database","/databases","Oracle, Metric Editor"],
    ["Middleware","/middleware","Dynamic per detected service"],
    ["Security","/security/dashboard","Threat Hunting, Firewall"],
    ["Logs","/log-explorer","Sources, Alerts, Analytics"],
    ["Alerts & Events","/alerts","Rules, Channels"],
    ["AI & Analytics","/ai-insights","Predictive, RCA, LLM, Profiling"],
    ["OpenTelemetry","/otel-dashboard","Traces, Resources, Pipeline, RED, Alerts"],
    ["Admin","/admin/users","Groups, Roles, Profile"],
  ]));
  c.push(pb());

  // 13 — OTEL (Highlighted)
  c.push(h1("13. OpenTelemetry Configuration"));
  c.push(p("Zentra has full OpenTelemetry support for all four signals: Traces, Metrics, Logs, and Profiles.", { bold: true }));
  c.push(sp());
  c.push(h2("13.1 Self-Monitoring (Producer)"));
  c.push(b("TracerProvider","service.name, service.version, service.namespace=zentra, UUID v7 instance ID"));
  c.push(b("Sampler","ParentBased(TraceIdRatioBased) — configurable via ZENTRA_OTEL_SAMPLE_RATIO"));
  c.push(b("Exporter","OTLP gRPC to OTEL_EXPORTER_OTLP_ENDPOINT"));
  c.push(b("Log bridge","tracing events → OTLP LogRecords via opentelemetry-appender-tracing"));
  c.push(sp());
  c.push(h2("13.2 OTLP Ingest (Consumer)"));
  c.push(p("Authentication: x-otlp-api-key header required on all ingest endpoints"));
  c.push(sp());
  c.push(h2("13.3 Async Ingest Pipeline"));
  c.push(...cb(`Client → HTTP/gRPC → mpsc channel (10k) → Background worker
                                        ↓
                              Batch INSERT via UNNEST (1024/batch)
                                        ↓
                              PostgreSQL + TimescaleDB
                                        ↓
                              On failure (3 retries): DLQ table`));
  c.push(sp());
  c.push(h2("13.4 Collector Configuration"));
  c.push(...cb(`receivers:
  otlp:
    protocols:
      grpc: { endpoint: "0.0.0.0:4317" }
      http: { endpoint: "0.0.0.0:4318" }
processors:
  memory_limiter: { limit_mib: 512 }
  batch: { send_batch_size: 1024, timeout: 5s }
  tail_sampling:
    policies:
      - { name: errors, type: status_code, status_code: { status_codes: [ERROR] } }
      - { name: latency, type: latency, latency: { threshold_ms: 5000 } }
      - { name: probabilistic, type: probabilistic, probabilistic: { sampling_percentage: 10 } }
exporters:
  otlphttp: { endpoint: "http://zentra-api:5006" }
  prometheus: { endpoint: "0.0.0.0:8889" }`));
  c.push(sp());
  c.push(h2("13.5 Instrumentation Examples"));
  c.push(h3("Python"));
  c.push(...cb(`from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
exporter = OTLPSpanExporter(
    endpoint="http://zentra-host:4317",
    headers={"x-otlp-api-key": "YOUR_KEY"}
)`));
  c.push(h3("Node.js"));
  c.push(...cb(`const exporter = new OTLPTraceExporter({
  url: 'http://zentra-host:4317',
  headers: { 'x-otlp-api-key': 'YOUR_KEY' }
});`));
  c.push(h3("Java"));
  c.push(...cb(`otel.exporter.otlp.endpoint=http://zentra-host:4317
otel.exporter.otlp.headers=x-otlp-api-key=YOUR_KEY`));
  c.push(h3(".NET"));
  c.push(...cb(`builder.Services.AddOpenTelemetry()
    .WithTracing(t => t.AddOtlpExporter(o => {
        o.Endpoint = new Uri("http://zentra-host:4317");
        o.Headers = "x-otlp-api-key=YOUR_KEY";
    }));`));
  c.push(sp());
  c.push(h2("13.6 TimescaleDB Storage"));
  c.push(tbl(["Table","Hypertable","Retention","Compression"],[
    ["traces","Yes","90 days","Enabled"],
    ["otlp_metrics","Yes","90 days","Enabled"],
    ["otlp_logs","Yes","30 days","Enabled"],
  ]));
  c.push(sp());
  c.push(h2("13.7 OTel UI Pages"));
  c.push(tbl(["Page","Route","Description"],[
    ["Telemetry Health","/otel-dashboard","Signal health, throughput, errors"],
    ["Traces Explorer","/traces","Search and explore traces"],
    ["Span Metrics RED","/otel-span-metrics","Rate, Errors, Duration per service"],
    ["Signal Alerts","/otel-alert-rules","Alert rules for OTel signals"],
    ["OTLP Security","/otel-security","API key management and audit"],
  ]));
  c.push(pb());

  // 14-23 (condensed)
  c.push(h1("14. Database Schema"));
  c.push(tbl(["Table","Purpose"],[["users","User accounts"],["targets","Monitoring targets"],["alerts","Active/historical alerts"],["traces","OTLP trace spans (hypertable)"],["otlp_api_keys","OTLP ingest API keys"],["otlp_dead_letter","Failed ingest payloads"]]));
  c.push(pb());

  c.push(h1("15. Monitoring Targets"));
  c.push(tbl(["Type","Protocol","Metrics"],[["Linux Server","SSH","CPU, memory, disk, network"],["Network Device","SNMP v2c/v3","Interfaces, traffic, CPU"],["Oracle Database","OCI + SSH","Tablespaces, sessions, waits"],["Storage Array","SSH/API","Volumes, pools, IOPS"]]));
  c.push(pb());

  c.push(h1("16. Alerting System"));
  c.push(...cb(`Metric Collection → Rule Evaluation → Alert Creation → Notification Router
                                                         → Email, Slack, PagerDuty, Webhook, Teams`));
  c.push(pb());

  c.push(h1("17. AI & Analytics"));
  c.push(tbl(["Feature","Route"],[["AI Insights","/ai-insights"],["Predictive Alerts","/predictive-alerts"],["Root Cause Analysis","/rca"],["LLM Observability","/llm-observability"],["Profiling","/profiling"]]));
  c.push(pb());

  c.push(h1("18. Security Module"));
  c.push(tbl(["Feature","Route"],[["Dashboard","/security/dashboard"],["Threat Hunting","/security/hunt"],["Firewall","/security/firewall"]]));
  c.push(pb());

  c.push(h1("19. Network Monitoring"));
  c.push(p("Supports SNMP v2c/v3, NetFlow/sFlow/IPFIX, device auto-discovery, and topology mapping."));
  c.push(pb());

  c.push(h1("20. Middleware Monitoring"));
  c.push(tbl(["Service","Metrics"],[["Nginx","Connections, requests/sec, status codes"],["Apache","Workers, requests, scoreboard"],["Tomcat","Threads, sessions, JVM memory"],["Redis","Memory, keyspace, commands/sec"],["Kafka","Topics, partitions, consumer lag"]]));
  c.push(pb());

  c.push(h1("21. Connection Pooling"));
  c.push(tbl(["Pool","Type","Config"],[["Main PG","SQLx PgPool","200 max connections"],["Target DB","LRU DashMap","Max 64, 5-min TTL"],["SSH Mux","ControlMaster","ControlPersist=600s"],["HTTP","reqwest::Client","idle_per_host=32, timeout=90s"]]));
  c.push(pb());

  c.push(h1("22. Backup & Recovery"));
  c.push(...cb(`# Database backup
docker exec zentra su - postgres -c \\
  "pg_dump -p 5434 zentra | gzip > /var/lib/postgresql/data/backup.sql.gz"
docker cp zentra:/var/lib/postgresql/data/backup.sql.gz ./backup.sql.gz

# Volume backup
docker stop zentra
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \\
  tar czf /backup/zentra-data.tar.gz -C /data .
docker start zentra`));
  c.push(pb());

  c.push(h1("23. Troubleshooting"));
  c.push(tbl(["Problem","Cause","Solution"],[
    ["License expired warning","Trial exceeded","Rebuild image or reset volume"],
    ["Integrity verification failed","Files modified","Use official tarball"],
    ["API returns 402","Trial expired","Dashboard only; get license"],
    ["docker exec fails","Shells removed","Use ZENTRA_SKIP_LOCKDOWN=1"],
    ["OTLP returns 401","Invalid API key","Generate at /otel-security"],
    ["OTLP returns 429","Rate limit","Increase rate_limit_rpm"],
  ]));
  c.push(sp());

  // Footer
  c.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { before: 600 }, children: [new TextRun({ text: "— End of Runbook —", font: F, size: 20, color: "999999", italics: true })] }));

  return makeDoc("Zentra Operations Runbook", "Comprehensive operations runbook", "May 2026", c);
}

// ═══════════════════════════════════════════════════════════════════════
// DOCUMENT 2: Docker Configuration Runbook
// ═══════════════════════════════════════════════════════════════════════
function buildDockerRunbook() {
  const c = [];
  c.push(...titlePage("Docker Configuration Runbook", "Internal Use Only — Docker Image Setup, Deployment & Hardening", "May 2026"));

  // TOC
  c.push(h1("Table of Contents"));
  ["1.  Overview","2.  Prerequisites","3.  Building the Docker Image","4.  Environment Variables",
   "5.  Running the Container","6.  Container Architecture","7.  Nginx Configuration",
   "8.  Supervisord Configuration","9.  License & Trial Period","10. Docker Image Hardening",
   "11. Datacenter Simulation Lab","12. Volume Management","13. Backup & Recovery",
   "14. Health Checks","15. Upgrading","16. Troubleshooting"
  ].forEach(t => c.push(p(t, { p: { spacing: { after: 60 } } })));
  c.push(pb());

  // 1
  c.push(h1("1. Overview"));
  c.push(p("Zentra ships as a single all-in-one Docker image based on Rocky Linux 8."));
  c.push(tbl(["Component","Technology","Internal Port","Exposed"],[
    ["Frontend","React SPA via Nginx","5010","Yes (only port)"],
    ["Backend API","Rust / Axum","5006","No — proxied by Nginx"],
    ["Database","PostgreSQL 16 + TimescaleDB","5434","No — localhost only"],
    ["OTLP gRPC","Axum HTTP/2","4317","No"],
    ["OTLP HTTP","Axum handler","4318","No — proxied via /v1/*"],
    ["Process Manager","Supervisord","—","—"],
  ]));
  c.push(pb());

  // 2
  c.push(h1("2. Prerequisites"));
  c.push(tbl(["Requirement","Minimum"],[["Docker Engine","20.10+"],["RAM","2 GB"],["Disk","5 GB (image) + volume"],["CPU","2 cores"]]));
  c.push(sp());
  c.push(h2("Build Server Requirements"));
  c.push(tbl(["Requirement","Details"],[["Rust","1.95+"],["Node.js","18+"],["Cargo target","x86_64-unknown-linux-gnu"]]));
  c.push(pb());

  // 3
  c.push(h1("3. Building the Docker Image"));
  c.push(...cb(`cd /store/Zentra

# 1. Build frontend
cd frontend && npm ci && npx vite build && cd ..

# 2. Build backend
cargo build --release --bin zentra-server

# 3. Build Docker image
docker build -f docker/Dockerfile -t zentra:latest .

# 4. Export tarball
docker save zentra:latest | gzip > docker/zentra-latest.tar.gz`));
  c.push(sp());
  c.push(h2("3.1 What Gets Copied Into the Image"));
  c.push(tbl(["Source","Destination","Purpose"],[
    ["target/release/zentra-server","/usr/local/bin/zentra-server","Backend binary"],
    ["migrations/","/opt/zentra/migrations/","SQL migrations"],
    ["frontend/dist/","/opt/zentra/frontend/dist/","Frontend SPA"],
    ["docker/supervisord.conf","/etc/supervisord.conf","Process manager"],
    ["docker/nginx.conf","/etc/nginx/conf.d/zentra.conf","Reverse proxy"],
    ["docker/docker-entrypoint.sh","/usr/local/bin/docker-entrypoint.sh","Startup script"],
  ]));
  c.push(sp());
  c.push(h2("3.2 What Is NOT in the Image"));
  c.push(b("No source code", "crates/, frontend/src/, Cargo.toml, .git"));
  c.push(b("No credentials", "No API keys, no passwords"));
  c.push(b("No dev tools", "gcc, make, gdb, strace removed"));
  c.push(b("Verified at build", "Dockerfile checks for source leaks"));
  c.push(pb());

  // 4
  c.push(h1("4. Environment Variables"));
  c.push(h2("4.1 Required Variables"));
  c.push(tbl(["Variable","Default","Description"],[
    ["POSTGRES_USER","postgres","PostgreSQL superuser"],
    ["POSTGRES_PASSWORD","Auto-generated","Database password"],
    ["POSTGRES_DB","zentra","Database name"],
    ["POSTGRES_PORT","5434","PostgreSQL port (internal)"],
    ["ZENTRA_API_PORT","5006","Backend API port (internal)"],
    ["ZENTRA_JWT_SECRET","Auto-generated","JWT signing secret (min 32 chars)"],
    ["ZENTRA_ALLOW_INSECURE_JWT","1","Set to 0 in production"],
    ["ZENTRA_ORG_ID","00000000-...0001","Organization UUID"],
    ["ZENTRA_ALLOW_ORIGINS","*","CORS allowed origins"],
    ["ZENTRA_MASTER_KEY","Auto-generated","AES-256 key (64 hex chars)"],
    ["RUST_LOG","info","Log verbosity"],
  ]));
  c.push(sp());
  c.push(h2("4.2 Auto-Generated Values"));
  c.push(b("POSTGRES_PASSWORD","24-char random alphanumeric"));
  c.push(b("ZENTRA_JWT_SECRET","48-char random alphanumeric"));
  c.push(b("ZENTRA_MASTER_KEY","64-char hex (32 bytes)"));
  c.push(b("DATABASE_URL","Constructed using restricted zentra_app user"));
  c.push(sp());
  c.push(h2("4.3 OpenTelemetry Variables"));
  c.push(tbl(["Variable","Default","Description"],[
    ["OTEL_EXPORTER_OTLP_ENDPOINT","—","OTLP collector endpoint"],
    ["ZENTRA_OTEL_SELF_ENABLED","true","Enable self-monitoring"],
    ["ZENTRA_OTEL_SAMPLE_RATIO","1.0","Trace sampling ratio"],
  ]));
  c.push(sp());
  c.push(h2("4.4 Operational Variables"));
  c.push(tbl(["Variable","Default","Description"],[
    ["ZENTRA_BIND_ADDR","127.0.0.1","API bind address (do NOT change)"],
    ["ZENTRA_SKIP_LOCKDOWN","0","Set 1 to disable lockdown (dev only)"],
    ["ZENTRA_CLEAN_SEED_DATA","1","Auto-remove seed data on first boot"],
  ]));
  c.push(sp());
  c.push(h2("4.5 Example .env File"));
  c.push(...cb(`POSTGRES_USER=postgres
POSTGRES_PASSWORD=MySecureP@ssw0rd!
POSTGRES_DB=zentra
POSTGRES_PORT=5434
ZENTRA_API_PORT=5006
ZENTRA_JWT_SECRET=your-super-secret-jwt-key-at-least-32-chars-long!!
ZENTRA_ALLOW_INSECURE_JWT=0
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
ZENTRA_ALLOW_ORIGINS=https://zentra.yourcompany.com
RUST_LOG=info`));
  c.push(pb());

  // 5
  c.push(h1("5. Running the Container"));
  c.push(h2("5.1 Quick Start (Zero Config)"));
  c.push(...cb(`docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 \\
  --security-opt=no-new-privileges:true \\
  -v zentra-data:/var/lib/postgresql/data \\
  zentra:latest`));
  c.push(p("Access: http://localhost:5010 — Login: admin@zentra.local / changeme", { bold: true }));
  c.push(sp());
  c.push(h2("5.2 Production (Hardened)"));
  c.push(...cb(`docker run -d --name zentra --restart unless-stopped \\
  --read-only \\
  --tmpfs /tmp:rw,noexec,nosuid,size=64m \\
  --tmpfs /var/run:rw,noexec,nosuid,size=16m \\
  --tmpfs /var/log:rw,noexec,nosuid,size=128m \\
  --tmpfs /opt/zentra/logs:rw,noexec,nosuid,size=64m \\
  --tmpfs /root/.ssh:rw,noexec,nosuid,size=4m \\
  --security-opt=no-new-privileges:true \\
  --cap-drop ALL \\
  --cap-add CHOWN --cap-add SETUID --cap-add SETGID \\
  --cap-add DAC_OVERRIDE --cap-add FOWNER \\
  --cap-add NET_BIND_SERVICE --cap-add LINUX_IMMUTABLE \\
  --env-file /opt/zentra/.env \\
  -p 5010:5010 -v zentra-data:/var/lib/postgresql/data zentra:latest`));
  c.push(sp());
  c.push(h2("5.3 Air-Gapped Deployment"));
  c.push(...cb(`scp zentra-latest.tar.gz admin@air-gapped:/tmp/
# On air-gapped server:
docker load -i /tmp/zentra-latest.tar.gz
docker run -d -p 5010:5010 --name zentra \\
  -v zentra-data:/var/lib/postgresql/data zentra:latest`));
  c.push(sp());
  c.push(h2("5.4 Port Mapping Options"));
  c.push(tbl(["Scenario","Flag","Description"],[
    ["Standard","-p 5010:5010","Default port"],
    ["Custom port","-p 8080:5010","Map to 8080"],
    ["Host-only","-p 127.0.0.1:5010:5010","Localhost only"],
    ["Specific NIC","-p 10.0.0.5:5010:5010","Bind to NIC"],
  ]));
  c.push(pb());

  // 6
  c.push(h1("6. Container Architecture"));
  c.push(h2("6.1 Startup Sequence"));
  c.push(...cb(`docker run → docker-entrypoint.sh
  ├─ Phase 1: Integrity verification (SHA-256 manifest)
  ├─ Phase 2: License check / generation
  ├─ Phase 3: Environment setup (auto-gen secrets)
  ├─ Phase 4: PostgreSQL init (first boot only)
  │     ├─ initdb + configure port/listen/pg_hba
  │     ├─ Enable TimescaleDB
  │     └─ Create zentra_app user (least-privilege)
  ├─ Phase 5: Copy license to PG volume
  ├─ Phase 6: SSH key generation
  ├─ Phase 7: Self-lockdown (D09)
  │     ├─ chattr +i on all critical files
  │     ├─ Remove pkg managers, editors, tools
  │     └─ Schedule shell removal (5s)
  └─ Phase 8: exec supervisord
        ├─ Priority 10: PostgreSQL
        ├─ Priority 20: zentra-server (user: zentra)
        └─ Priority 30: Nginx`));
  c.push(sp());
  c.push(h2("6.2 File System Layout"));
  c.push(tbl(["Path","Contents","Owner","Perms"],[
    ["/usr/local/bin/zentra-server","Backend binary","root:zentra","550"],
    ["/usr/local/bin/docker-entrypoint.sh","Startup script","root:root","500"],
    ["/opt/zentra/migrations/","SQL migrations","root","400/file"],
    ["/opt/zentra/frontend/dist/","React SPA","root","read-only"],
    ["/opt/zentra/logs/","Runtime logs","zentra","writable"],
    ["/opt/zentra/.integrity","SHA-256 manifest","root","400"],
    ["/var/lib/postgresql/data/","VOLUME — PG data","postgres","700"],
    ["/etc/supervisord.conf","Supervisord config","root","600"],
    ["/etc/nginx/conf.d/zentra.conf","Nginx config","root","600"],
  ]));
  c.push(pb());

  // 7
  c.push(h1("7. Nginx Configuration"));
  c.push(p("Nginx is the sole external entry point on port 5010."));
  c.push(tbl(["Location","Action","Details"],[
    ["/","Serve SPA","try_files $uri /index.html"],
    ["/api/*","Reverse proxy","→ 127.0.0.1:5006"],
    ["/v1/*","OTLP proxy","→ 127.0.0.1:5006 (max 10 MB)"],
    ["/api/auth/login","Rate-limited","5 req/min per IP, burst 3"],
    ["/api/system/*","Internal only","allow 127.0.0.1; deny all"],
    ["/metrics","Internal only","Private networks only"],
    [".*.(env|git|sql|sh)$","Blocked","Returns 404"],
  ]));
  c.push(sp());
  c.push(h2("7.1 Security Headers"));
  c.push(b("X-Content-Type-Options","nosniff"));
  c.push(b("X-Frame-Options","SAMEORIGIN"));
  c.push(b("X-XSS-Protection","1; mode=block"));
  c.push(b("CSP","default-src 'self'; script-src 'self'; ..."));
  c.push(b("server_tokens","off (hide version)"));
  c.push(pb());

  // 8
  c.push(h1("8. Supervisord Configuration"));
  c.push(tbl(["Service","Command","User","Priority","Auto-restart"],[
    ["postgresql","postgres -D /var/lib/postgresql/data","postgres","10","Yes"],
    ["zentra-server","/usr/local/bin/zentra-server","zentra","20","Yes (10 retries)"],
    ["nginx","nginx -g 'daemon off;'","root","30","Yes"],
  ]));
  c.push(sp());
  c.push(h2("8.1 Log Locations"));
  c.push(tbl(["Service","Log File"],[
    ["PostgreSQL","/var/log/supervisor/postgresql.log"],
    ["zentra-server (stdout)","/opt/zentra/logs/api.log"],
    ["zentra-server (stderr)","/opt/zentra/logs/api-error.log"],
    ["Nginx access","/opt/zentra/logs/nginx_access.log"],
    ["Nginx error","/opt/zentra/logs/nginx_error.log"],
    ["Supervisord","/var/log/supervisor/supervisord.log"],
  ]));
  c.push(pb());

  // 9
  c.push(h1("9. License & Trial Period"));
  c.push(h2("9.1 How It Works"));
  c.push(b("First boot","Generates signed token: install:expiry:hostname:hmac"));
  c.push(b("HMAC key","SHA-256 hash of zentra-server binary"));
  c.push(b("Primary location","/var/lib/postgresql/data/.zentra-license"));
  c.push(b("Fallback","/opt/zentra/.license"));
  c.push(b("Trial duration","3 days (LICENSE_DAYS in entrypoint)"));
  c.push(sp());
  c.push(h2("9.2 Degraded Mode"));
  c.push(tbl(["Aspect","Behavior"],[
    ["Startup","Continues normally"],
    ["Dashboard","Fully functional"],
    ["Other pages","Hidden from navigation"],
    ["Non-dashboard API","Returns 402"],
    ["Frontend","Auto-redirects to /dashboard"],
    ["Warning banner","Amber bar at top"],
  ]));
  c.push(sp());
  c.push(h2("9.3 Renewal Options"));
  c.push(tbl(["Method","Steps","Data Loss"],[
    ["Fresh volume","docker volume rm zentra-data + re-run","Yes"],
    ["Rebuild image","New binary hash → new license","No"],
    ["Production license","Contact Zentra team","No"],
  ]));
  c.push(pb());

  // 10
  c.push(h1("10. Docker Image Hardening"));
  c.push(h2("10.1 Build-Time Hardening"));
  c.push(tbl(["Measure","Details"],[
    ["Non-root user","zentra user for API server"],
    ["Binary stripping","Debug symbols removed"],
    ["No source code","Verified — no .git, Cargo.toml, src/"],
    ["Restricted perms","Migrations 400, configs 600, binary 550"],
    ["SUID/SGID cleared","All setuid/setgid bits removed"],
    ["Core dumps disabled","* hard core 0"],
    ["Integrity manifest","SHA-256 checksums of critical files"],
  ]));
  c.push(sp());
  c.push(h2("10.2 Runtime Lockdown (D09)"));
  c.push(tbl(["Step","Action"],[
    ["1","Make license files immutable (chattr +i)"],
    ["2","Make config, binary, migrations, frontend immutable"],
    ["3","Remove package managers (dnf, yum, rpm)"],
    ["4","Remove editors (vi, vim, nano, ed)"],
    ["5","Remove scripting languages (perl, lua, ruby)"],
    ["6","Remove dangerous tools (wget, ftp, gcc, gdb, dd)"],
    ["7","Remove chattr itself (permanent lock)"],
    ["8","Schedule shell removal (5s after start)"],
  ]));
  c.push(sp());
  c.push(h2("10.3 Runtime Security Flags"));
  c.push(tbl(["Flag","Purpose"],[
    ["--read-only","Read-only root filesystem"],
    ["--tmpfs /tmp:noexec","Writable tmp, no execute"],
    ["--security-opt=no-new-privileges","No privilege escalation"],
    ["--cap-drop ALL","Drop all capabilities"],
    ["--cap-add LINUX_IMMUTABLE","Required for chattr +i"],
  ]));
  c.push(sp());
  c.push(h2("10.4 Debug Mode"));
  c.push(p("Set ZENTRA_SKIP_LOCKDOWN=1 to disable lockdown. NEVER use in production.", { bold: true }));
  c.push(pb());

  // 11
  c.push(h1("11. Datacenter Simulation Lab"));
  c.push(p("docker/simulation/ provides 12 containers simulating a real datacenter.", { bold: true }));
  c.push(sp());
  c.push(h2("11.1 Lab Components — Linux Host (10.88.11.x)"));
  c.push(tbl(["#","Component","Container","IP","Ports","Image","Role"],[
    ["1","Linux Server 01","srv-linux-01","10.88.11.201","22, 161","rockylinux:8","Web Server"],
    ["2","Linux Server 02","srv-linux-02","10.88.11.202","22, 161","ubuntu:22.04","App Server"],
    ["3","Windows Server 01","srv-win-01","10.88.11.203","22, 161","polinux/snmpd:alpine","AD/DNS"],
    ["4","Windows Server 02","srv-win-02","10.88.11.204","22, 161","polinux/snmpd:alpine","File Server"],
    ["5","Core Switch","sw-core-01","10.88.11.205","22, 161","polinux/snmpd:alpine","Cisco 9300"],
    ["6","Access Switch","sw-access-01","10.88.11.206","22, 161","polinux/snmpd:alpine","Arista 7050X"],
    ["7","Firewall","fw-01","10.88.11.207","22, 161","polinux/snmpd:alpine","Palo Alto"],
    ["8","Storage Array","stor-01","10.88.11.208","22, 161","polinux/snmpd:alpine","Dell PowerStore"],
    ["9","Nginx","mw-nginx-01","10.88.11.209","22, 80, 8080","nginx:1.25-alpine","Reverse Proxy"],
    ["10","Apache","mw-apache-01","10.88.11.210","22, 80, 8080","httpd:2.4-alpine","HTTP Server"],
    ["11","Tomcat","mw-tomcat-01","10.88.11.211","22, 8080, 9090","tomcat:9-jdk17","App Server"],
    ["12","Oracle DB","db-oracle-01","10.88.11.212","22, 1521","gvenzl/oracle-xe:21-slim","Oracle 21c"],
  ]));
  c.push(sp());
  c.push(h2("11.2 Lab Components — Windows Host (10.100.0.x)"));
  c.push(tbl(["#","Component","Container","IP","Ports","Role"],[
    ["1","Linux Server 01","srv-linux-01","10.100.0.11","22, 161","Rocky Linux 8"],
    ["2","Linux Server 02","srv-linux-02","10.100.0.12","22, 161","Ubuntu 22.04"],
    ["3","Windows Server 01","srv-win-01","10.100.0.13","22, 161","Windows 2022"],
    ["4","Windows Server 02","srv-win-02","10.100.0.14","22, 161","Windows 2019"],
    ["5","Core Switch","sw-core-01","10.100.0.15","22, 161","Cisco 9300"],
    ["6","Access Switch","sw-access-01","10.100.0.16","22, 161","Arista 7050X"],
    ["7","Firewall","fw-01","10.100.0.17","22, 161","Palo Alto"],
    ["8","Storage Array","stor-01","10.100.0.18","22, 161","Dell PowerStore"],
    ["9","Nginx","mw-nginx-01","10.100.0.19","22, 80, 8080","Reverse Proxy"],
    ["10","Apache","mw-apache-01","10.100.0.20","22, 80, 8080","HTTP Server"],
    ["11","Tomcat","mw-tomcat-01","10.100.0.21","22, 8080, 9090","App Server"],
    ["12","Oracle DB","db-oracle-01","10.100.0.22","22, 1521","Oracle 21c"],
  ]));
  c.push(sp());
  c.push(h2("11.3 Lab Credentials"));
  c.push(tbl(["Service","Username","Password","Notes"],[
    ["All (SSH)","root","zentra123","Port 22"],
    ["SNMP","—","Community: public","v2c, port 161"],
    ["Tomcat Manager","zentra","zentra123","http://<ip>:8080/manager"],
    ["Nginx Status","—","—","http://<ip>:8080/nginx_status"],
    ["Apache Status","—","—","http://<ip>:8080/server-status"],
    ["Oracle SYS","SYS","zentra123","SID: XE"],
    ["Oracle Monitor","zentra_mon","zentra123","Read-only"],
    ["Zentra UI","admin@zentra.local","changeme","Port 5010"],
  ]));
  c.push(sp());
  c.push(h2("11.4 Starting the Lab"));
  c.push(p("Linux:", { bold: true }));
  c.push(...cb(`cd docker/simulation && sudo ./setup.sh`));
  c.push(p("Windows:", { bold: true }));
  c.push(...cb(`cd docker\\simulation
powershell -ExecutionPolicy Bypass -File setup.ps1`));
  c.push(p("Stopping:", { bold: true }));
  c.push(...cb(`docker compose -f docker/simulation/docker-compose.generated.yml down`));
  c.push(sp());
  c.push(h2("11.5 Adding Lab Targets to Zentra"));
  c.push(tbl(["Target","Where in UI","Configuration"],[
    ["Servers","Infrastructure → Servers","IP, port 22, root/zentra123"],
    ["Switches","Infrastructure → Network","IP, SNMP 'public'"],
    ["Firewall","Infrastructure → Network","IP, SNMP 'public'"],
    ["Storage","Infrastructure → Storage","IP, SSH root/zentra123"],
    ["Nginx","Middleware → Nginx","<ip>:8080/nginx_status"],
    ["Apache","Middleware → Apache","<ip>:8080/server-status"],
    ["Tomcat","Middleware → Tomcat","<ip>:8080, zentra/zentra123"],
    ["Oracle","Database → Oracle","<ip>:1521, XE, zentra_mon"],
  ]));
  c.push(pb());

  // 12
  c.push(h1("12. Volume Management"));
  c.push(p("The Docker volume zentra-data (/var/lib/postgresql/data) persists:"));
  c.push(b("PostgreSQL data","All monitoring data, users, config"));
  c.push(b(".zentra-license","License token (survives container recreation)"));
  c.push(sp());
  c.push(...cb(`docker volume ls
docker volume inspect zentra-data
docker volume rm zentra-data  # DESTROYS ALL DATA`));
  c.push(pb());

  // 13
  c.push(h1("13. Backup & Recovery"));
  c.push(h2("13.1 Database Backup"));
  c.push(...cb(`docker exec zentra su - postgres -c \\
  "pg_dump -p 5434 zentra | gzip > /var/lib/postgresql/data/backup.sql.gz"
docker cp zentra:/var/lib/postgresql/data/backup.sql.gz ./backup.sql.gz`));
  c.push(sp());
  c.push(h2("13.2 Volume Backup"));
  c.push(...cb(`docker stop zentra
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \\
  tar czf /backup/zentra-data.tar.gz -C /data .
docker start zentra`));
  c.push(sp());
  c.push(h2("13.3 Restore"));
  c.push(...cb(`docker volume rm zentra-data
docker volume create zentra-data
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \\
  tar xzf /backup/zentra-data.tar.gz -C /data
docker run -d -p 5010:5010 -v zentra-data:/var/lib/postgresql/data zentra:latest`));
  c.push(pb());

  // 14
  c.push(h1("14. Health Checks"));
  c.push(p("Built-in: HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3"));
  c.push(sp());
  c.push(...cb(`# API health
curl -sf http://localhost:5010/api/health

# Container health
docker inspect --format='{{.State.Health.Status}}' zentra

# Database
curl -sf http://localhost:5010/api/system/db

# License
curl -sf http://localhost:5010/api/system/license-status`));
  c.push(pb());

  // 15
  c.push(h1("15. Upgrading"));
  c.push(...cb(`# 1. Build new image
docker build -f docker/Dockerfile -t zentra:latest .

# 2. Stop old container
docker stop zentra && docker rm zentra

# 3. Start with same volume (data preserved, migrations auto-run)
docker run -d --name zentra -p 5010:5010 \\
  -v zentra-data:/var/lib/postgresql/data zentra:latest`));
  c.push(sp());
  c.push(h2("15.1 Rollback"));
  c.push(...cb(`docker tag zentra:latest zentra:previous  # Before upgrading
# If upgrade fails:
docker stop zentra && docker rm zentra
docker run -d --name zentra -p 5010:5010 \\
  -v zentra-data:/var/lib/postgresql/data zentra:previous`));
  c.push(pb());

  // 16
  c.push(h1("16. Troubleshooting"));
  c.push(tbl(["Problem","Cause","Solution"],[
    ["License expired on fresh install","Binary hash mismatch","Delete volume, re-run"],
    ["Integrity verification failed","Files tampered","Use official image"],
    ["Container won't start","Port conflict / DB corruption","docker logs zentra"],
    ["API returns 402","Trial expired","Dashboard only; get license"],
    ["docker exec fails","Shells removed (D09)","ZENTRA_SKIP_LOCKDOWN=1"],
    ["Cannot install packages","Pkg managers removed","Rebuild image"],
    ["SSH monitoring fails","Keys not configured","Copy pub key to target"],
    ["OTLP 401","Invalid API key","Generate at /otel-security"],
    ["OTLP 429","Rate limit exceeded","Increase rate_limit_rpm"],
    ["Nginx 502","Backend not ready","Wait 30s; check api.log"],
  ]));
  c.push(sp());
  c.push(h2("16.1 Debug Checklist"));
  c.push(...cb(`# 1. Container status
docker ps -a | grep zentra

# 2. Health
docker inspect --format='{{.State.Health.Status}}' zentra

# 3. Startup logs
docker logs zentra 2>&1 | head -50

# 4. API
curl -sf http://localhost:5010/api/health

# 5. Interactive debug (dev only)
docker run -it --rm -e ZENTRA_SKIP_LOCKDOWN=1 \\
  -p 5010:5010 -v zentra-data:/var/lib/postgresql/data \\
  zentra:latest /bin/bash`));

  c.push(sp());
  c.push(new Paragraph({ alignment: AlignmentType.CENTER, spacing: { before: 600 }, children: [new TextRun({ text: "— End of Docker Runbook —", font: F, size: 20, color: "999999", italics: true })] }));

  return makeDoc("Zentra Docker Configuration Runbook", "Docker image setup, deployment & hardening", "May 2026", c);
}

// ═══════════════════════════════════════════════════════════════════════
// Generate both documents
// ═══════════════════════════════════════════════════════════════════════
async function main() {
  const doc1 = buildMainRunbook();
  const buf1 = await Packer.toBuffer(doc1);
  writeFileSync("C:/git/Zentra/docs/Zentra_Operations_Runbook.docx", buf1);
  console.log(`✅ Main Runbook: docs/Zentra_Operations_Runbook.docx (${(buf1.length/1024).toFixed(0)} KB)`);

  const doc2 = buildDockerRunbook();
  const buf2 = await Packer.toBuffer(doc2);
  writeFileSync("C:/git/Zentra/docs/Zentra_Docker_Runbook.docx", buf2);
  console.log(`✅ Docker Runbook: docs/Zentra_Docker_Runbook.docx (${(buf2.length/1024).toFixed(0)} KB)`);
}

main().catch(e => { console.error(e); process.exit(1); });
