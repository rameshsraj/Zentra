import {
  Document, Packer, Paragraph, TextRun, HeadingLevel, Table, TableRow, TableCell,
  WidthType, BorderStyle, AlignmentType, PageBreak, Header, Footer,
  ShadingType, TableLayoutType, convertInchesToTwip
} from "docx";
import { writeFileSync, readFileSync } from "fs";

// ── Styling constants ──────────────────────────────────────────────────
const FONT = "Calibri";
const CODE_FONT = "Consolas";
const BLUE = "1F4E79";
const DARK = "2D2D2D";
const LIGHT_BLUE = "D6E4F0";
const LIGHT_GRAY = "F2F2F2";
const WHITE = "FFFFFF";
const TABLE_HEADER_BG = "1F4E79";
const TABLE_ALT_BG = "EDF2F7";

// ── Helper functions ───────────────────────────────────────────────────
function heading(text, level = HeadingLevel.HEADING_1) {
  return new Paragraph({ heading: level, spacing: { before: 300, after: 120 }, children: [new TextRun({ text, font: FONT, bold: true, color: BLUE })] });
}

function h1(text) { return heading(text, HeadingLevel.HEADING_1); }
function h2(text) { return heading(text, HeadingLevel.HEADING_2); }
function h3(text) { return heading(text, HeadingLevel.HEADING_3); }

function para(text, opts = {}) {
  return new Paragraph({
    spacing: { after: 80 },
    ...opts.pOpts,
    children: [new TextRun({ text, font: FONT, size: 22, color: DARK, ...opts })]
  });
}

function bold(text) { return para(text, { bold: true }); }

function bullet(text, level = 0) {
  return new Paragraph({
    bullet: { level },
    spacing: { after: 40 },
    children: [new TextRun({ text, font: FONT, size: 22, color: DARK })]
  });
}

function bulletBold(label, desc, level = 0) {
  return new Paragraph({
    bullet: { level },
    spacing: { after: 40 },
    children: [
      new TextRun({ text: label + " — ", font: FONT, size: 22, bold: true, color: DARK }),
      new TextRun({ text: desc, font: FONT, size: 22, color: DARK })
    ]
  });
}

function code(text) {
  const lines = text.split("\n");
  return lines.map(line => new Paragraph({
    spacing: { after: 0 },
    shading: { type: ShadingType.SOLID, color: "F5F5F5", fill: "F5F5F5" },
    indent: { left: convertInchesToTwip(0.3) },
    children: [new TextRun({ text: line || " ", font: CODE_FONT, size: 18, color: "333333" })]
  }));
}

function codeBlock(text) {
  return [
    new Paragraph({ spacing: { before: 80, after: 0 }, children: [] }),
    ...code(text),
    new Paragraph({ spacing: { before: 0, after: 80 }, children: [] }),
  ];
}

function spacer() { return new Paragraph({ spacing: { after: 60 }, children: [] }); }

function pageBreak() { return new Paragraph({ children: [new PageBreak()] }); }

// Table helper
function makeTable(headers, rows) {
  const headerRow = new TableRow({
    tableHeader: true,
    children: headers.map(h => new TableCell({
      shading: { type: ShadingType.SOLID, color: TABLE_HEADER_BG, fill: TABLE_HEADER_BG },
      width: { size: Math.floor(100 / headers.length), type: WidthType.PERCENTAGE },
      children: [new Paragraph({ spacing: { after: 40 }, children: [new TextRun({ text: h, font: FONT, size: 20, bold: true, color: WHITE })] })]
    }))
  });

  const dataRows = rows.map((row, idx) => new TableRow({
    children: row.map(cell => new TableCell({
      shading: idx % 2 === 1 ? { type: ShadingType.SOLID, color: TABLE_ALT_BG, fill: TABLE_ALT_BG } : undefined,
      children: [new Paragraph({ spacing: { after: 40 }, children: [new TextRun({ text: String(cell), font: FONT, size: 20, color: DARK })] })]
    }))
  }));

  return new Table({
    width: { size: 100, type: WidthType.PERCENTAGE },
    layout: TableLayoutType.AUTOFIT,
    rows: [headerRow, ...dataRows]
  });
}

// ── Build the document ─────────────────────────────────────────────────
const children = [];

// ── TITLE PAGE ─────────────────────────────────────────────────────────
children.push(new Paragraph({ spacing: { before: 3000 }, children: [] }));
children.push(new Paragraph({
  alignment: AlignmentType.CENTER,
  spacing: { after: 200 },
  children: [new TextRun({ text: "ZENTRA", font: FONT, size: 72, bold: true, color: BLUE })]
}));
children.push(new Paragraph({
  alignment: AlignmentType.CENTER,
  spacing: { after: 100 },
  children: [new TextRun({ text: "Operations Runbook", font: FONT, size: 44, color: BLUE })]
}));
children.push(new Paragraph({
  alignment: AlignmentType.CENTER,
  spacing: { after: 60 },
  children: [new TextRun({ text: "Version 1.0.0  •  May 2026", font: FONT, size: 24, color: "666666" })]
}));
children.push(new Paragraph({
  alignment: AlignmentType.CENTER,
  spacing: { after: 60 },
  children: [new TextRun({ text: "Internal Use Only — Learning & Operations", font: FONT, size: 22, color: "999999", italics: true })]
}));
children.push(pageBreak());

// ── TABLE OF CONTENTS (manual) ─────────────────────────────────────────
children.push(h1("Table of Contents"));
const tocItems = [
  "1.  Project Overview",
  "2.  Architecture",
  "3.  Technology Stack",
  "4.  Crate & Plugin Structure",
  "5.  Environment Configuration",
  "6.  Docker Deployment",
  "7.  Docker Image Hardening",
  "8.  License & Trial Period",
  "9.  Authentication & RBAC",
  "10. API Reference",
  "11. Frontend Modules",
  "12. OpenTelemetry Configuration",
  "13. Database Schema",
  "14. Monitoring Targets",
  "15. Alerting System",
  "16. AI & Analytics",
  "17. Security Module",
  "18. Network Monitoring",
  "19. Middleware Monitoring",
  "20. Connection Pooling",
  "21. Backup & Recovery",
  "22. Troubleshooting",
];
tocItems.forEach(t => children.push(para(t, { pOpts: { spacing: { after: 60 } } })));
children.push(pageBreak());

// ── 1. PROJECT OVERVIEW ────────────────────────────────────────────────
children.push(h1("1. Project Overview"));
children.push(para("Zentra is an all-in-one infrastructure monitoring platform built with a Rust backend and React frontend. It runs as a single Docker container that includes:"));
children.push(bulletBold("Backend API", "Rust/Axum on port 5006 (internal)"));
children.push(bulletBold("Frontend SPA", "React/Vite served by Nginx on port 5010"));
children.push(bulletBold("Database", "PostgreSQL 16 + TimescaleDB (internal, port 5434)"));
children.push(bulletBold("Process Manager", "Supervisord managing all three services"));
children.push(spacer());
children.push(para("The platform monitors servers, databases (PostgreSQL, Oracle), middleware (Nginx, Apache, Tomcat, HAProxy, Redis, Kafka, etc.), network devices, storage arrays, and security events. It includes OpenTelemetry ingest, AI-powered analytics, and a built-in alerting engine."));
children.push(pageBreak());

// ── 2. ARCHITECTURE ────────────────────────────────────────────────────
children.push(h1("2. Architecture"));
children.push(para("All services run inside a single Docker container based on Rocky Linux 8:"));
children.push(spacer());
children.push(...codeBlock(
`┌─────────────────────────────────────────────────────┐
│              Docker Container (Rocky Linux 8)        │
│                                                      │
│  ┌─────────┐    ┌──────────────┐   ┌────────────┐   │
│  │  Nginx   │───>│ zentra-server│──>│ PostgreSQL │   │
│  │  :5010   │    │   :5006      │   │   :5434    │   │
│  └─────────┘    │  (Rust/Axum) │   │ +TimescaleDB│  │
│                  └──────────────┘   └────────────┘   │
│                         │                            │
│                  ┌──────┴───────┐                    │
│                  │  Supervisord  │                   │
│                  └──────────────┘                    │
└─────────────────────────────────────────────────────┘`
));
children.push(spacer());
children.push(h3("Key Design Principles"));
children.push(bullet("Single container, zero external dependencies"));
children.push(bullet("Only port 5010 exposed externally (Nginx proxy)"));
children.push(bullet("API and database are internal-only (127.0.0.1)"));
children.push(bullet("Signed license with tamper detection"));
children.push(bullet("Self-lockdown after startup (no shells, immutable files)"));
children.push(spacer());
children.push(h3("OTLP Ingest Topology"));
children.push(...codeBlock(
`OTLP Clients ──> :4317 (gRPC) ──> zentra-server (ingest pipeline)
              ──> :4318 (HTTP) ──> zentra-server (/v1/traces, /v1/metrics, /v1/logs)`
));
children.push(pageBreak());

// ── 3. TECHNOLOGY STACK ────────────────────────────────────────────────
children.push(h1("3. Technology Stack"));
children.push(makeTable(
  ["Layer", "Technology", "Version"],
  [
    ["Backend", "Rust + Axum", "1.95+ / Axum 0.7"],
    ["Frontend", "React + TypeScript + Vite", "React 18"],
    ["UI Framework", "TailwindCSS + Lucide Icons", "TailwindCSS 3"],
    ["State Management", "TanStack React Query", "v5"],
    ["Database", "PostgreSQL + TimescaleDB", "PG 16 + TSDB 2"],
    ["Process Manager", "Supervisord", "4.2"],
    ["Reverse Proxy", "Nginx", "Latest (Rocky 8)"],
    ["Container", "Docker (Rocky Linux 8)", "—"],
    ["Telemetry", "OpenTelemetry SDK", "0.27"],
    ["Crypto", "Ed25519 (ring), SHA-256, AES-256-GCM", "—"],
    ["Auth", "JWT (HS256) + MFA (TOTP)", "—"],
    ["SSH", "OpenSSH + sshpass", "—"],
  ]
));
children.push(pageBreak());

// ── 4. CRATE & PLUGIN STRUCTURE ────────────────────────────────────────
children.push(h1("4. Crate & Plugin Structure"));
children.push(h2("4.1 Backend Crates (crates/)"));
children.push(makeTable(
  ["Crate", "Purpose"],
  [
    ["zentra-api", "Main Axum HTTP server, routes, handlers, middleware"],
    ["zentra-core", "Shared models, crypto (AES, license verification), telemetry init, SSH pool"],
    ["zentra-db", "Database pool management, repository layer, migration support"],
    ["zentra-stream", "Event bus (tokio broadcast), notification routing, alert rule evaluation"],
    ["zentra-agent", "Standalone agent binary for remote server metric collection"],
    ["zentra-ingestion", "OTLP payload parsing and batch ingest pipeline"],
    ["zentra-query", "Query services for metrics, logs, traces"],
    ["zentra-scrape", "Prometheus-compatible scrape target management"],
    ["zentra-ml", "ML model integration for anomaly detection"],
    ["zentra-predict", "Predictive alerting and capacity forecasting"],
    ["zentra-log-engine", "Log parsing, discovery rules, structured log processing"],
    ["zentra-netflow", "NetFlow/sFlow/IPFIX collector and analysis"],
    ["zentra-drilldown", "Deep-dive analytics for infrastructure metrics"],
  ]
));
children.push(spacer());
children.push(h2("4.2 Plugins (plugins/)"));
children.push(makeTable(
  ["Plugin", "Purpose"],
  [
    ["server-plugin", "Linux/Windows server metrics (CPU, memory, disk, network) via SSH"],
    ["network-plugin", "SNMP polling, device discovery, topology mapping"],
    ["database-plugin", "PostgreSQL, Oracle, MySQL monitoring"],
    ["storage-plugin", "Storage array monitoring (Dell, NetApp, Pure Storage)"],
    ["security-plugin", "IDS/IPS rules, firewall logs, threat detection"],
    ["cloud-plugin", "AWS, Azure, GCP cloud resource monitoring"],
    ["ai-plugin", "AI/ML insights, anomaly detection, RCA"],
    ["license-plugin", "Third-party software license tracking (Oracle, VMware, etc.)"],
  ]
));
children.push(pageBreak());

// ── 5. ENVIRONMENT CONFIGURATION ───────────────────────────────────────
children.push(h1("5. Environment Configuration"));
children.push(h2("5.1 Required Environment Variables"));
children.push(makeTable(
  ["Variable", "Default", "Description"],
  [
    ["POSTGRES_USER", "postgres", "PostgreSQL superuser"],
    ["POSTGRES_PASSWORD", "Auto-generated", "Database password"],
    ["POSTGRES_DB", "zentra", "Database name"],
    ["POSTGRES_PORT", "5434", "PostgreSQL port (internal)"],
    ["ZENTRA_API_PORT", "5006", "Backend API port (internal)"],
    ["ZENTRA_JWT_SECRET", "Auto-generated", "JWT signing secret (min 32 chars for production)"],
    ["ZENTRA_ALLOW_INSECURE_JWT", "1", "Set to 0 in production to enforce strong JWT secret"],
    ["ZENTRA_ORG_ID", "00000000-...0001", "Default organization UUID"],
    ["ZENTRA_ALLOW_ORIGINS", "*", "CORS allowed origins"],
    ["ZENTRA_BIND_ADDR", "127.0.0.1", "API bind address (internal only)"],
    ["RUST_LOG", "info", "Log verbosity (trace/debug/info/warn/error)"],
    ["DATABASE_URL", "Auto-constructed", "Full PostgreSQL connection string"],
    ["ZENTRA_MASTER_KEY", "Auto-generated", "AES-256 key for credential encryption (64 hex chars)"],
  ]
));
children.push(spacer());
children.push(h2("5.2 OpenTelemetry Environment Variables"));
children.push(makeTable(
  ["Variable", "Default", "Description"],
  [
    ["OTEL_EXPORTER_OTLP_ENDPOINT", "—", "OTLP collector endpoint (e.g., http://collector:4317)"],
    ["OTEL_EXPORTER_OTLP_PROTOCOL", "grpc", "Protocol: grpc or http/protobuf"],
    ["ZENTRA_OTEL_SELF_ENABLED", "true", "Enable self-monitoring telemetry"],
    ["ZENTRA_OTEL_SAMPLE_RATIO", "1.0", "Trace sampling ratio (0.0–1.0)"],
    ["ZENTRA_OTLP_SAMPLE_RATIO", "1.0", "Receiver-side sampling for ingest"],
    ["ZENTRA_METRICS_CARDINALITY_CAP_*", "1000", "Per-key metric label cardinality cap"],
  ]
));
children.push(spacer());
children.push(h2("5.3 Example .env File"));
children.push(...codeBlock(
`POSTGRES_USER=postgres
POSTGRES_PASSWORD=MySecureP@ssw0rd!
POSTGRES_DB=zentra
POSTGRES_PORT=5434
ZENTRA_API_PORT=5006
ZENTRA_JWT_SECRET=your-super-secret-jwt-key-at-least-32-chars-long!!
ZENTRA_ALLOW_INSECURE_JWT=0
ZENTRA_ORG_ID=00000000-0000-0000-0000-000000000001
ZENTRA_ALLOW_ORIGINS=https://zentra.yourcompany.com
RUST_LOG=info`
));
children.push(pageBreak());

// ── 6. DOCKER DEPLOYMENT ──────────────────────────────────────────────
children.push(h1("6. Docker Deployment"));
children.push(h2("6.1 Quick Start (Zero Config)"));
children.push(...codeBlock(
`docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 \\
  --security-opt=no-new-privileges:true \\
  -v zentra-data:/var/lib/postgresql/data \\
  zentra:latest`
));
children.push(para("Access at http://localhost:5010"));
children.push(para("Default login: admin@zentra.local / changeme", { bold: true }));
children.push(spacer());

children.push(h2("6.2 Production Deployment (Hardened)"));
children.push(...codeBlock(
`docker run -d \\
  --name zentra \\
  --restart unless-stopped \\
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
  -p 5010:5010 \\
  -v zentra-data:/var/lib/postgresql/data \\
  zentra:latest`
));
children.push(spacer());

children.push(h2("6.3 Air-Gapped Deployment"));
children.push(...codeBlock(
`# On a machine with no internet
docker load -i zentra-latest.tar.gz
docker run -d -p 5010:5010 --name zentra \\
  --security-opt=no-new-privileges:true \\
  -v zentra-data:/var/lib/postgresql/data \\
  zentra:latest`
));
children.push(spacer());

children.push(h2("6.4 Building the Docker Image"));
children.push(...codeBlock(
`# On the build server (10.88.11.162)
cd /store/Zentra

# 1. Build frontend
cd frontend && npm ci && npx vite build && cd ..

# 2. Build backend (release)
cargo build --release --bin zentra-server

# 3. Build Docker image
docker build -f docker/Dockerfile -t zentra:latest .

# 4. Export image as tarball
docker save zentra:latest | gzip > docker/zentra-latest.tar.gz`
));
children.push(spacer());

children.push(h2("6.5 Container Services (Supervisord)"));
children.push(makeTable(
  ["Service", "User", "Port", "Auto-restart"],
  [
    ["postgresql", "postgres", "5434", "Yes"],
    ["zentra-server", "zentra", "5006", "Yes"],
    ["nginx", "root", "5010", "Yes"],
  ]
));
children.push(pageBreak());

// ── 7. DOCKER IMAGE HARDENING ──────────────────────────────────────────
children.push(h1("7. Docker Image Hardening"));
children.push(para("The Docker image implements D01–D11 hardening prompts for production security."));
children.push(spacer());
children.push(h2("7.1 Self-Lockdown (D09/D11)"));
children.push(para("After the entrypoint completes initialization, the container locks itself down:"));
children.push(makeTable(
  ["Measure", "Details"],
  [
    ["Immutable files", "Config, license, binary, migrations, and frontend files locked with chattr +i. Even root cannot modify."],
    ["No shells", "bash, sh, csh, zsh removed 5 seconds after startup. docker exec becomes useless."],
    ["No package managers", "dnf, yum, rpm removed. No software can be installed at runtime."],
    ["No editors", "vi, vim, nano, ed, perl, lua, ruby all removed."],
    ["No dangerous tools", "wget, ftp, telnet, dd, strings, gcc, make, strace, gdb all removed."],
    ["chattr removed", "After making files immutable, chattr itself is deleted — immutable bits can never be reversed."],
    ["SSH preserved", "Only sshpass, ssh, scp kept (required for monitoring target servers)."],
    ["Tamper protection", "SHA-256 integrity manifest verifies critical files on every startup."],
    ["Binary-verified license", "Rust binary reads license from disk, verifies HMAC against its own SHA-256 hash."],
    ["Anti-deletion", "If license file missing but DB exists → treated as bypass attempt → expired."],
  ]
));
children.push(spacer());
children.push(h2("7.2 Debugging Mode"));
children.push(para("Set ZENTRA_SKIP_LOCKDOWN=1 to disable self-lockdown (development only, NEVER use in production).", { bold: true }));
children.push(pageBreak());

// ── 8. LICENSE & TRIAL PERIOD ──────────────────────────────────────────
children.push(h1("8. License & Trial Period"));
children.push(h2("8.1 How It Works"));
children.push(bullet("First boot: docker-entrypoint.sh generates a signed license token"));
children.push(bullet("Format: install_epoch:expiry_epoch:hostname:sha256_hmac", 1));
children.push(bullet("HMAC key: SHA-256 hash of the zentra-server binary", 1));
children.push(bullet("Stored at /var/lib/postgresql/data/.zentra-license (PG volume)", 1));
children.push(bullet("Fallback: /opt/zentra/.license", 1));
children.push(bullet("Subsequent boots: Entrypoint verifies the HMAC signature and checks expiry"));
children.push(bullet("Trial duration: 3 days (configurable via LICENSE_DAYS in entrypoint)"));
children.push(spacer());

children.push(h2("8.2 Graceful Degradation After Expiry"));
children.push(para("After the trial expires, the container still starts but in degraded mode:"));
children.push(bullet("Entrypoint logs a WARNING and sets ZENTRA_TRIAL_EXPIRED=1"));
children.push(bullet("Backend middleware returns 402 Payment Required for non-essential API routes"));
children.push(bullet("Frontend hides all navigation except the Command Center (Dashboard)"));
children.push(bullet("An amber warning banner is displayed at the top of the page"));
children.push(spacer());

children.push(h2("8.3 Allowed Routes in Degraded Mode"));
children.push(makeTable(
  ["Route", "Purpose"],
  [
    ["/api/health", "Health check"],
    ["/api/auth/*", "Login, logout, token refresh"],
    ["/api/system/license-status", "License status (includes trial_expired flag)"],
    ["/api/config/mode", "App mode check"],
    ["/api/system/status", "System status"],
    ["/api/system/db", "Database health"],
    ["/metrics", "Prometheus metrics"],
  ]
));
children.push(spacer());

children.push(h2("8.4 Binary-Level Verification (Hardened)"));
children.push(para("The Rust binary (license_verifier.rs) independently reads the license file from disk:"));
children.push(bullet("Computes its own SHA-256 hash"));
children.push(bullet("Verifies the HMAC signature"));
children.push(bullet("Checks the expiry epoch"));
children.push(bullet("Does NOT trust the ZENTRA_TRIAL_EXPIRED environment variable in Docker"));
children.push(bullet("If the license file is missing but the DB directory exists → treated as bypass attempt → expired"));
children.push(bullet("Result is cached in OnceLock (one disk read per process lifetime)"));
children.push(spacer());

children.push(h2("8.5 Renewing the License"));
children.push(bullet("Rebuild and redeploy — Build a fresh image; license resets on first boot with a new volume."));
children.push(bullet("Reset the volume — docker volume rm zentra-data and re-run (data will be lost)."));
children.push(bullet("Contact Zentra team — For extended or permanent licenses."));
children.push(pageBreak());

// ── 9. AUTHENTICATION & RBAC ──────────────────────────────────────────
children.push(h1("9. Authentication & RBAC"));
children.push(h2("9.1 Authentication Flow"));
children.push(bullet("User submits email/password to POST /api/auth/login"));
children.push(bullet("Server validates credentials, returns JWT token + refresh token"));
children.push(bullet("Frontend stores tokens in localStorage (Zentra_token, Zentra_refresh_token)"));
children.push(bullet("All API requests include Authorization: Bearer <token> header"));
children.push(bullet("Token auto-refreshes 5 minutes before expiry (proactive refresh)"));
children.push(bullet("MFA (TOTP) support via POST /api/auth/mfa/setup and POST /api/auth/mfa/verify"));
children.push(spacer());

children.push(h2("9.2 Two-Layer RBAC"));
children.push(para("Layer 1 — Module Access (which top-level sidebar modules a user sees):"));
children.push(makeTable(
  ["Module ID", "Sidebar Label"],
  [
    ["command_center", "Command Center"],
    ["infrastructure", "Infrastructure"],
    ["database", "Database"],
    ["middleware", "Middleware"],
    ["security", "Security"],
    ["logs", "Logs"],
    ["alerts_events", "Alerts & Events"],
    ["ai_analytics", "AI & Analytics"],
    ["opentelemetry", "OpenTelemetry"],
    ["admin", "Admin"],
  ]
));
children.push(spacer());
children.push(para("Layer 2 — Sub-Module Menu Items (dotted convention, e.g., infrastructure.servers):"));
children.push(bullet("If a user has a module but NO sub-items → sees ALL children"));
children.push(bullet("If sub-items exist → only those are shown"));
children.push(spacer());

children.push(h2("9.3 Key Auth Endpoints"));
children.push(makeTable(
  ["Endpoint", "Method", "Purpose"],
  [
    ["/api/auth/login", "POST", "Login (email + password)"],
    ["/api/auth/logout", "POST", "Invalidate session"],
    ["/api/auth/refresh", "POST", "Refresh JWT token"],
    ["/api/auth/me", "GET", "Get current user profile + module_access + menu_items"],
    ["/api/auth/mfa/setup", "POST", "Generate MFA QR code"],
    ["/api/auth/mfa/verify", "POST", "Verify MFA TOTP code"],
    ["/api/auth/sessions", "GET", "List active sessions"],
    ["/api/auth/sessions/:id", "DELETE", "Revoke a session"],
    ["/api/auth/password", "POST", "Change password"],
  ]
));
children.push(pageBreak());

// ── 10. API REFERENCE ──────────────────────────────────────────────────
children.push(h1("10. API Reference"));
children.push(h2("10.1 Health & System"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/api/health", "GET", "Health check (returns {\"status\":\"healthy\"})"],
    ["/api/system/status", "GET", "System status (internal only)"],
    ["/api/system/db", "GET", "Database connectivity check"],
    ["/api/system/license-status", "GET", "License status with trial_expired flag"],
    ["/api/system/pool-status", "GET", "Connection pool statistics"],
    ["/api/config/mode", "GET/POST", "Demo/Live mode toggle"],
  ]
));
children.push(spacer());

children.push(h2("10.2 Targets & Infrastructure"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/api/targets", "GET/POST", "List/create monitoring targets"],
    ["/api/targets/:id", "GET/PUT/DELETE", "Target CRUD"],
    ["/api/targets/:id/verify-ssh", "POST", "Test SSH connectivity"],
    ["/api/scrape-targets", "GET/POST", "Prometheus scrape targets"],
    ["/api/collectors", "GET/POST", "Collector management"],
    ["/api/service-discovery", "GET", "Auto-discovered services"],
    ["/api/healthchecks", "GET/POST", "HTTP health checks"],
    ["/api/maintenance-windows", "GET/POST", "Maintenance window scheduling"],
  ]
));
children.push(spacer());

children.push(h2("10.3 Alerts & Notifications"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/api/alerts", "GET", "List active alerts"],
    ["/api/alerts/:id/acknowledge", "POST", "Acknowledge an alert"],
    ["/api/alerts/:id/resolve", "POST", "Resolve an alert"],
    ["/api/alert-rules", "GET/POST", "Alert rule CRUD"],
    ["/api/notification-channels", "GET/POST", "Notification channel management"],
    ["/api/alert-templates", "GET/POST", "Alert template management"],
    ["/api/silences", "GET/POST", "Alert silencing rules"],
  ]
));
children.push(spacer());

children.push(h2("10.4 OpenTelemetry Ingest"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/v1/traces", "POST", "OTLP trace ingest (JSON or Protobuf)"],
    ["/v1/metrics", "POST", "OTLP metrics ingest"],
    ["/v1/logs", "POST", "OTLP logs ingest"],
    ["/v1/profiles", "POST", "OTLP profiles ingest"],
    [":4317", "gRPC", "gRPC OTLP ingest (all signals)"],
  ]
));
children.push(spacer());

children.push(h2("10.5 AI & Analytics"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/api/ai/chat", "POST", "AI-assisted chat"],
    ["/api/ai/insights", "GET", "AI-generated insights"],
    ["/api/ai/predictive-alerts", "GET", "Predictive alert analysis"],
    ["/api/ai/rca", "POST", "Root cause analysis"],
    ["/api/ai/global-search", "GET", "Cross-module search"],
  ]
));
children.push(spacer());

children.push(h2("10.6 Admin"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/api/admin/users", "GET/POST", "User management"],
    ["/api/admin/users/:id", "GET/PUT/DELETE", "User CRUD"],
    ["/api/admin/groups", "GET/POST", "Group management"],
    ["/api/admin/roles", "GET/POST", "Role management"],
  ]
));
children.push(pageBreak());

// ── 11. FRONTEND MODULES ──────────────────────────────────────────────
children.push(h1("11. Frontend Modules"));
children.push(h2("11.1 Navigation Structure"));
children.push(makeTable(
  ["Module", "Route", "Sub-pages"],
  [
    ["Command Center", "/dashboard", "Overview dashboard with fleet cards"],
    ["Infrastructure", "/infrastructure", "Dashboard, Servers, Storage, Network, eBPF"],
    ["Database", "/databases", "Dashboard, Oracle Monitoring, Metric Editor"],
    ["Middleware", "/middleware", "Dashboard + dynamic sub-pages per detected service"],
    ["Security", "/security/dashboard", "Dashboard, Threat Hunting, Firewall"],
    ["Logs", "/log-explorer", "Explorer, Sources, Alerts, Analytics"],
    ["Alerts & Events", "/alerts", "Active Alerts, Rules, Channels"],
    ["AI & Analytics", "/ai-insights", "Insights, Predictive, RCA, RunBooks, LLM, Profiling"],
    ["OpenTelemetry", "/otel-dashboard", "Health, Traces, Resources, Collector, SDK, Connectors, Pipeline, RED, Alerts, Security"],
    ["Admin", "/admin/users", "Users, Groups, Roles, My Profile"],
  ]
));
children.push(spacer());

children.push(h2("11.2 Frontend Architecture"));
children.push(bulletBold("Auth Context (AuthContext.tsx)", "Manages user state, JWT tokens, hasModule(), hasMenuItem(), isAdmin"));
children.push(bulletBold("Layout (Layout.tsx)", "Sidebar navigation with RBAC filtering, license status check, trial expired banner"));
children.push(bulletBold("API Client (_client.ts)", "Centralized fetchJSON() with auto-refresh on 401, proactive token renewal"));
children.push(bulletBold("React Query", "All data fetching uses TanStack React Query with stale times and refetch intervals"));
children.push(bulletBold("Theme System", "Multiple themes via ThemeContext, selectable from sidebar"));
children.push(pageBreak());

// ── 12. OPENTELEMETRY CONFIGURATION ────────────────────────────────────
children.push(h1("12. OpenTelemetry Configuration"));
children.push(para("This section provides a comprehensive guide to configuring OpenTelemetry (OTel) with Zentra, both as a producer (self-monitoring) and consumer (ingest from external apps).", { bold: true }));
children.push(spacer());

children.push(h2("12.1 Overview"));
children.push(para("Zentra has full OpenTelemetry support covering all four signals: Traces, Metrics, Logs, and Profiles. It supports both gRPC and HTTP OTLP protocols."));
children.push(spacer());

children.push(h2("12.2 Self-Monitoring (Producer)"));
children.push(para("Zentra instruments itself using the OpenTelemetry SDK. This is configured in zentra-core/src/telemetry.rs and initialized in the API server's main.rs:"));
children.push(...codeBlock(
`// main.rs initialization
let otel_layer = zentra_core::telemetry::init_telemetry("zentra-api");
tracing_subscriber::registry()
    .with(env_filter)
    .with(fmt_layer)
    .with(otel_layer)
    .init();`
));
children.push(spacer());
children.push(para("The telemetry layer creates:"));
children.push(bullet("TracerProvider with service.name, service.version, service.namespace=zentra"));
children.push(bullet("Unique service.instance.id (UUID v7) per process"));
children.push(bullet("ParentBased(TraceIdRatioBased) sampler for efficient sampling"));
children.push(bullet("OTLP gRPC exporter to OTEL_EXPORTER_OTLP_ENDPOINT"));
children.push(bullet("Log bridge via opentelemetry-appender-tracing (tracing events → OTLP LogRecords)"));
children.push(spacer());

children.push(para("Key environment variables for self-monitoring:", { bold: true }));
children.push(...codeBlock(
`OTEL_EXPORTER_OTLP_ENDPOINT=http://collector:4317
OTEL_EXPORTER_OTLP_PROTOCOL=grpc
ZENTRA_OTEL_SELF_ENABLED=true
ZENTRA_OTEL_SAMPLE_RATIO=0.1   # Sample 10% in production`
));
children.push(spacer());

children.push(h2("12.3 OTLP Ingest (Consumer)"));
children.push(para("Zentra can receive telemetry from any OTLP-compatible application:"));
children.push(spacer());
children.push(h3("HTTP Ingest Endpoints"));
children.push(makeTable(
  ["Endpoint", "Content Types", "Description"],
  [
    ["POST /v1/traces", "application/json, application/x-protobuf", "Trace spans"],
    ["POST /v1/metrics", "application/json, application/x-protobuf", "Metric data points"],
    ["POST /v1/logs", "application/json, application/x-protobuf", "Log records"],
    ["POST /v1/profiles", "application/json, application/x-protobuf", "Profiling data"],
  ]
));
children.push(spacer());

children.push(h3("gRPC Ingest"));
children.push(makeTable(
  ["Port", "Protocol", "Signals"],
  [
    ["4317", "gRPC (HTTP/2)", "Traces, Metrics, Logs, Profiles"],
  ]
));
children.push(spacer());

children.push(h3("Authentication"));
children.push(para("All ingest endpoints require an OTLP API key in the x-otlp-api-key header:"));
children.push(...codeBlock(
`curl -X POST http://zentra:5010/v1/traces \\
  -H "Content-Type: application/json" \\
  -H "x-otlp-api-key: YOUR_API_KEY" \\
  -d @traces.json`
));
children.push(spacer());

children.push(h2("12.4 Async Ingest Pipeline"));
children.push(para("The ingest pipeline uses an async architecture with bounded channels and batch processing:"));
children.push(...codeBlock(
`Client → HTTP/gRPC handler → mpsc channel (10k buffer) → Background worker
                                                              │
                                                    ┌─────────┴────────┐
                                                    │  Batch INSERT    │
                                                    │  via UNNEST      │
                                                    │  (1024 per batch)│
                                                    └─────────┬────────┘
                                                              │
                                                    ┌─────────┴────────┐
                                                    │  PostgreSQL +    │
                                                    │  TimescaleDB     │
                                                    │  (hypertables)   │
                                                    └─────────┬────────┘
                                                              │
                                                    On failure (3 retries):
                                                    ┌─────────┴────────┐
                                                    │  Dead Letter      │
                                                    │  Queue (DLQ)      │
                                                    │  otlp_dead_letter │
                                                    └──────────────────┘`
));
children.push(spacer());

children.push(h2("12.5 Quota Enforcement"));
children.push(para("OTLP ingest supports per-API-key quotas:"));
children.push(makeTable(
  ["Setting", "Description"],
  [
    ["rate_limit_rpm", "Max requests per minute per API key"],
    ["max_payload_bytes", "Max request body size per API key"],
  ]
));
children.push(para("When exceeded, returns 429 Too Many Requests or 413 Payload Too Large."));
children.push(spacer());

children.push(h2("12.6 OTel Collector Configuration"));
children.push(para("For deployments with an external OTel Collector, use docker/otel-collector-config.yaml:"));
children.push(...codeBlock(
`receivers:
  otlp:
    protocols:
      grpc:
        endpoint: "0.0.0.0:4317"
      http:
        endpoint: "0.0.0.0:4318"

processors:
  memory_limiter:
    check_interval: 5s
    limit_mib: 512
    spike_limit_mib: 128
  batch:
    send_batch_size: 1024
    timeout: 5s
  tail_sampling:
    decision_wait: 10s
    policies:
      - name: errors-policy
        type: status_code
        status_code: { status_codes: [ERROR] }
      - name: latency-policy
        type: latency
        latency: { threshold_ms: 5000 }
      - name: probabilistic-policy
        type: probabilistic
        probabilistic: { sampling_percentage: 10 }

exporters:
  otlphttp:
    endpoint: "http://zentra-api:5006"
    tls: { insecure: true }
  prometheus:
    endpoint: "0.0.0.0:8889"

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, tail_sampling, batch]
      exporters: [otlphttp]
    metrics:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp, prometheus]
    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [otlphttp]`
));
children.push(spacer());

children.push(h2("12.7 Instrumenting Your Application"));
children.push(spacer());
children.push(h3("Python (Flask/Django)"));
children.push(...codeBlock(
`from opentelemetry import trace
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter

provider = TracerProvider()
exporter = OTLPSpanExporter(
    endpoint="http://zentra-host:4317",
    headers={"x-otlp-api-key": "YOUR_API_KEY"}
)
provider.add_span_processor(BatchSpanProcessor(exporter))
trace.set_tracer_provider(provider)`
));
children.push(spacer());

children.push(h3("Node.js"));
children.push(...codeBlock(
`const { OTLPTraceExporter } = require('@opentelemetry/exporter-trace-otlp-grpc');
const exporter = new OTLPTraceExporter({
  url: 'http://zentra-host:4317',
  headers: { 'x-otlp-api-key': 'YOUR_API_KEY' }
});`
));
children.push(spacer());

children.push(h3("Java (Spring Boot)"));
children.push(...codeBlock(
`# application.properties
otel.exporter.otlp.endpoint=http://zentra-host:4317
otel.exporter.otlp.headers=x-otlp-api-key=YOUR_API_KEY
otel.service.name=my-java-app`
));
children.push(spacer());

children.push(h3(".NET"));
children.push(...codeBlock(
`// Program.cs
builder.Services.AddOpenTelemetry()
    .WithTracing(tracing => tracing
        .AddAspNetCoreInstrumentation()
        .AddOtlpExporter(o => {
            o.Endpoint = new Uri("http://zentra-host:4317");
            o.Headers = "x-otlp-api-key=YOUR_API_KEY";
        }));`
));
children.push(spacer());

children.push(h2("12.8 Frontend RUM (Real User Monitoring)"));
children.push(para("Zentra's frontend includes built-in OpenTelemetry RUM (frontend/src/lib/otel/init.ts):"));
children.push(bullet("WebTracerProvider with FetchInstrumentation"));
children.push(bullet("DocumentLoad and UserInteraction auto-instrumentation"));
children.push(bullet("W3C traceparent header propagation"));
children.push(bullet("Configurable via VITE_OTEL_ENDPOINT env var"));
children.push(spacer());

children.push(h2("12.9 OTLP Security (API Key Management)"));
children.push(para("Manage OTLP API keys via the UI at /otel-security or the API:"));
children.push(makeTable(
  ["Endpoint", "Method", "Description"],
  [
    ["/api/otel/security/api-keys", "GET/POST", "List/create API keys"],
    ["/api/otel/security/api-keys/:id", "DELETE", "Revoke an API key"],
    ["/api/otel/security/audit", "GET", "Audit log of API key usage"],
  ]
));
children.push(spacer());

children.push(h2("12.10 Background Jobs"));
children.push(makeTable(
  ["Job", "Interval", "Description"],
  [
    ["MV Refresh", "60s", "Refreshes span_red_metrics_1m materialized view (advisory lock 48201)"],
    ["Alert Eval", "5 min", "Evaluates OTel alert rules against span metrics (lock 48202)"],
    ["Cache Prune", "1 hour", "Prunes stale OTLP API key cache entries (lock 48203)"],
  ]
));
children.push(spacer());

children.push(h2("12.11 TimescaleDB Storage"));
children.push(makeTable(
  ["Table", "Hypertable", "Retention", "Compression"],
  [
    ["traces", "Yes (timestamp)", "90 days", "Enabled"],
    ["otlp_metrics", "Yes", "90 days", "Enabled"],
    ["otlp_logs", "Yes", "30 days", "Enabled"],
    ["span_events", "No", "—", "—"],
    ["span_links", "No", "—", "—"],
    ["otlp_dead_letter", "No", "—", "—"],
  ]
));
children.push(spacer());

children.push(h2("12.12 OTel UI Pages"));
children.push(makeTable(
  ["Page", "Route", "Description"],
  [
    ["Telemetry Health", "/otel-dashboard", "Overview of all signal health, throughput, error rates"],
    ["Traces Explorer", "/traces", "Search, filter, and explore distributed traces"],
    ["Resources", "/otel-resources", "Discovered services and resource attributes"],
    ["Collector Setup", "/otel-config", "OTel Collector configuration guide"],
    ["SDK Guide", "/otel-sdk-guide", "Instrumentation guides for all languages"],
    ["Connectors", "/otel-connectors", "Connector management (span→metrics, etc.)"],
    ["Pipeline Topology", "/otel-pipeline", "Visual pipeline topology viewer"],
    ["Span Metrics RED", "/otel-span-metrics", "Rate, Errors, Duration metrics per service"],
    ["Signal Alerts", "/otel-alert-rules", "Alert rules for OTel signals"],
    ["OTLP Security", "/otel-security", "API key management and audit"],
  ]
));
children.push(pageBreak());

// ── 13. DATABASE SCHEMA ────────────────────────────────────────────────
children.push(h1("13. Database Schema"));
children.push(h2("13.1 Key Tables"));
children.push(makeTable(
  ["Table", "Purpose"],
  [
    ["users", "User accounts (PK: user_id)"],
    ["user_module_access", "RBAC module assignments"],
    ["targets", "Monitoring targets (servers, devices)"],
    ["alerts", "Active and historical alerts"],
    ["alert_rules", "Alert rule definitions"],
    ["notification_channels", "Email, Slack, PagerDuty, etc."],
    ["traces", "OTLP trace spans (TimescaleDB hypertable)"],
    ["otlp_metrics", "OTLP metric data points"],
    ["otlp_logs", "OTLP log records"],
    ["otlp_api_keys", "OTLP ingest API keys"],
    ["otlp_dead_letter", "Failed ingest payloads"],
    ["system_settings", "Key-value system configuration"],
  ]
));
children.push(spacer());

children.push(h2("13.2 Migrations"));
children.push(para("SQL migrations are stored in /opt/zentra/migrations/ inside the container and in migrations/ in the source. They are applied automatically by zentra-server on startup."));
children.push(para("Naming convention: YYYYMMDD######_description.sql (e.g., 20250101000070_traces_base_schema.sql)"));
children.push(pageBreak());

// ── 14. MONITORING TARGETS ─────────────────────────────────────────────
children.push(h1("14. Monitoring Targets"));
children.push(h2("14.1 Adding a Target"));
children.push(bullet("Navigate to Infrastructure → Servers"));
children.push(bullet("Click Add Target"));
children.push(bullet("Fill in: Hostname/IP, SSH port, SSH credentials"));
children.push(bullet("Click Verify SSH to test connectivity"));
children.push(bullet("Save"));
children.push(spacer());

children.push(h2("14.2 Target Types"));
children.push(makeTable(
  ["Type", "Protocol", "Metrics Collected"],
  [
    ["Linux Server", "SSH", "CPU, memory, disk, network, processes"],
    ["Windows Server", "WMI/SSH", "CPU, memory, disk, services"],
    ["Network Device", "SNMP v2c/v3", "Interfaces, traffic, CPU, memory"],
    ["Oracle Database", "OCI + SSH", "Tablespaces, sessions, waits, ASM"],
    ["PostgreSQL", "SQL", "Connections, queries, replication"],
    ["Storage Array", "SSH/API", "Volumes, pools, disks, IOPS"],
  ]
));
children.push(spacer());

children.push(h2("14.3 SSH Key Setup"));
children.push(para("The container auto-generates SSH keys on first boot at /root/.ssh/id_rsa. To set up monitoring:"));
children.push(...codeBlock(
`# Get the public key from the container
docker exec zentra cat /root/.ssh/id_rsa.pub

# Add it to the target server
ssh-copy-id -i <key> user@target-server`
));
children.push(pageBreak());

// ── 15. ALERTING SYSTEM ────────────────────────────────────────────────
children.push(h1("15. Alerting System"));
children.push(h2("15.1 Alert Flow"));
children.push(...codeBlock(
`Metric Collection → Rule Evaluation (zentra-stream) → Alert Creation
       ↓                                                     ↓
  Notification Router                              Alert UI (/alerts)
       ↓
  Channels (Email, Slack, PagerDuty, Webhook, Teams)`
));
children.push(spacer());

children.push(h2("15.2 Alert States"));
children.push(makeTable(
  ["State", "Description"],
  [
    ["firing", "Alert condition is currently true"],
    ["acknowledged", "Operator has seen the alert"],
    ["resolved", "Alert condition cleared (auto or manual)"],
    ["silenced", "Matched by a silence rule"],
  ]
));
children.push(spacer());

children.push(h2("15.3 Creating Alert Rules"));
children.push(bullet("Go to Alerts & Events → Rules"));
children.push(bullet("Click Create Rule"));
children.push(bullet("Configure: metric, condition, threshold, duration, severity"));
children.push(bullet("Assign notification channels"));
children.push(bullet("Save"));
children.push(pageBreak());

// ── 16. AI & ANALYTICS ────────────────────────────────────────────────
children.push(h1("16. AI & Analytics"));
children.push(makeTable(
  ["Feature", "Route", "Description"],
  [
    ["AI Insights", "/ai-insights", "AI-generated operational insights"],
    ["Predictive Alerts", "/predictive-alerts", "Forecast capacity issues before they occur"],
    ["Root Cause Analysis", "/rca", "Automated RCA for incidents"],
    ["RunBooks", "/runbooks", "Automated remediation playbooks"],
    ["LLM Observability", "/llm-observability", "Monitor LLM/GenAI workloads (OTel gen_ai.* semconv)"],
    ["Profiling", "/profiling", "Application profiling with flamegraph visualization"],
  ]
));
children.push(pageBreak());

// ── 17. SECURITY MODULE ───────────────────────────────────────────────
children.push(h1("17. Security Module"));
children.push(makeTable(
  ["Feature", "Route", "Description"],
  [
    ["Dashboard", "/security/dashboard", "Security posture overview"],
    ["Threat Hunting", "/security/hunt", "Interactive threat investigation"],
    ["Firewall", "/security/firewall", "Firewall log analysis and rules"],
    ["IDS/IPS", "via API", "Intrusion detection/prevention rules and alerts"],
    ["File Analysis", "via API", "File integrity monitoring"],
  ]
));
children.push(pageBreak());

// ── 18. NETWORK MONITORING ─────────────────────────────────────────────
children.push(h1("18. Network Monitoring"));
children.push(makeTable(
  ["Feature", "Route", "Description"],
  [
    ["Network Dashboard", "/network", "Device status, topology map"],
    ["Deep Analysis", "via API", "Packet-level analysis"],
    ["Device Discovery", "via API", "Auto-discover network devices via SNMP"],
    ["NetFlow", "via API", "Flow analysis (NetFlow/sFlow/IPFIX)"],
  ]
));
children.push(spacer());
children.push(h2("18.1 SNMP Configuration"));
children.push(para("Target devices must have SNMP enabled. Zentra supports SNMP v2c (community string) and SNMP v3 (user/auth/priv)."));
children.push(pageBreak());

// ── 19. MIDDLEWARE MONITORING ──────────────────────────────────────────
children.push(h1("19. Middleware Monitoring"));
children.push(para("Middleware services are auto-discovered and dynamically added to the sidebar:"));
children.push(makeTable(
  ["Service", "Metrics"],
  [
    ["Nginx", "Active connections, requests/sec, status codes"],
    ["Apache", "Workers, requests, scoreboard"],
    ["Tomcat", "Threads, sessions, JVM memory"],
    ["HAProxy", "Backends, health, queue depth"],
    ["Redis", "Memory, keyspace, commands/sec"],
    ["Elasticsearch", "Indices, shards, cluster health"],
    ["Kafka", "Topics, partitions, consumer lag"],
    ["Docker", "Containers, CPU, memory per container"],
    ["Kubernetes", "Pods, deployments, resource usage"],
  ]
));
children.push(pageBreak());

// ── 20. CONNECTION POOLING ─────────────────────────────────────────────
children.push(h1("20. Connection Pooling"));
children.push(para("Zentra manages multiple connection pools for efficient resource usage:"));
children.push(makeTable(
  ["Pool", "Type", "Configuration"],
  [
    ["Main PG", "SQLx PgPool", "200 max connections"],
    ["TimescaleDB", "SQLx PgPool", "Shared with main"],
    ["Target DB Cache", "LRU DashMap", "Max 64 targets, 5-min TTL"],
    ["Oracle OCI", "Native pool registry", "Per-target pools"],
    ["SSH Mux", "ControlMaster", "ControlPersist=600s, ServerAlive=30s"],
    ["HTTP Shared", "reqwest::Client", "pool_idle_per_host=32, idle_timeout=90s"],
  ]
));
children.push(spacer());
children.push(para("Monitor pool health at GET /api/system/pool-status."));
children.push(pageBreak());

// ── 21. BACKUP & RECOVERY ──────────────────────────────────────────────
children.push(h1("21. Backup & Recovery"));
children.push(h2("21.1 Database Backup"));
children.push(...codeBlock(
`# From inside the container (before shell lockdown)
docker exec zentra su - postgres -c \\
  "pg_dump -p 5434 zentra | gzip > /var/lib/postgresql/data/zentra-backup.sql.gz"

# Copy backup out
docker cp zentra:/var/lib/postgresql/data/zentra-backup.sql.gz ./zentra-backup.sql.gz`
));
children.push(spacer());

children.push(h2("21.2 Volume Backup"));
children.push(...codeBlock(
`# Stop container
docker stop zentra

# Backup the volume
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \\
  tar czf /backup/zentra-data-backup.tar.gz -C /data .

# Restart
docker start zentra`
));
children.push(spacer());

children.push(h2("21.3 Recovery"));
children.push(...codeBlock(
`# Remove old volume
docker volume rm zentra-data

# Restore from backup
docker volume create zentra-data
docker run --rm -v zentra-data:/data -v $(pwd):/backup alpine \\
  tar xzf /backup/zentra-data-backup.tar.gz -C /data

# Start container
docker run -d -p 5010:5010 -v zentra-data:/var/lib/postgresql/data zentra:latest`
));
children.push(pageBreak());

// ── 22. TROUBLESHOOTING ────────────────────────────────────────────────
children.push(h1("22. Troubleshooting"));
children.push(h2("22.1 Common Issues"));
children.push(makeTable(
  ["Problem", "Cause", "Solution"],
  [
    ["\"License expired\" warning", "Trial period exceeded", "Rebuild image or reset volume"],
    ["\"Integrity verification failed\"", "Image files modified", "Use official image tarball"],
    ["Container won't start", "DB corruption or port conflict", "Check docker logs zentra"],
    ["API returns 402", "Trial expired (degraded mode)", "Only Dashboard available; get a license"],
    ["\"docker exec\" fails", "Shells removed by lockdown", "By design — use ZENTRA_SKIP_LOCKDOWN=1 for debug"],
    ["Cannot install packages", "Package managers removed", "By design — rebuild image instead"],
    ["SSH monitoring fails", "SSH keys not configured", "Copy container's public key to target"],
    ["OTLP ingest returns 401", "Invalid or missing API key", "Generate key at /otel-security"],
    ["OTLP ingest returns 429", "Rate limit exceeded", "Increase rate_limit_rpm for the API key"],
    ["Slow queries", "Missing indexes or large data", "Check TimescaleDB compression/retention"],
  ]
));
children.push(spacer());

children.push(h2("22.2 Viewing Logs"));
children.push(...codeBlock(
`# Container logs (stdout)
docker logs zentra

# If lockdown hasn't fired yet (within 5s of startup):
docker exec zentra cat /opt/zentra/logs/api.log
docker exec zentra cat /opt/zentra/logs/nginx_access.log
docker exec zentra cat /opt/zentra/logs/nginx_error.log
docker exec zentra supervisorctl status`
));
children.push(spacer());

children.push(h2("22.3 Health Checks"));
children.push(...codeBlock(
`# API health
curl -sf http://localhost:5010/api/health

# Database health
curl -sf http://localhost:5010/api/system/db

# All services status (before lockdown)
docker exec zentra supervisorctl status`
));
children.push(spacer());
children.push(spacer());

// Footer
children.push(new Paragraph({
  alignment: AlignmentType.CENTER,
  spacing: { before: 600 },
  children: [new TextRun({ text: "— End of Runbook —", font: FONT, size: 20, color: "999999", italics: true })]
}));
children.push(new Paragraph({
  alignment: AlignmentType.CENTER,
  spacing: { after: 60 },
  children: [new TextRun({ text: "Zentra Team  •  Internal Use Only", font: FONT, size: 18, color: "999999" })]
}));

// ── Create Document ────────────────────────────────────────────────────
const doc = new Document({
  creator: "Zentra Team",
  title: "Zentra Operations Runbook",
  description: "Comprehensive operations runbook for the Zentra monitoring platform",
  styles: {
    paragraphStyles: [
      { id: "Normal", run: { font: FONT, size: 22, color: DARK } },
    ],
  },
  sections: [{
    properties: {
      page: {
        margin: { top: convertInchesToTwip(0.8), bottom: convertInchesToTwip(0.8), left: convertInchesToTwip(0.9), right: convertInchesToTwip(0.9) },
      },
    },
    headers: {
      default: new Header({
        children: [new Paragraph({
          alignment: AlignmentType.RIGHT,
          children: [new TextRun({ text: "Zentra Operations Runbook  •  v1.0.0", font: FONT, size: 16, color: "AAAAAA", italics: true })]
        })]
      })
    },
    footers: {
      default: new Footer({
        children: [new Paragraph({
          alignment: AlignmentType.CENTER,
          children: [new TextRun({ text: "Internal Use Only — Zentra Team", font: FONT, size: 16, color: "AAAAAA" })]
        })]
      })
    },
    children,
  }],
});

// ── Write to file ──────────────────────────────────────────────────────
const buffer = await Packer.toBuffer(doc);
writeFileSync("C:/git/Zentra/docs/Zentra_Operations_Runbook.docx", buffer);
console.log("✅ Word document generated: docs/Zentra_Operations_Runbook.docx");
console.log(`   Size: ${(buffer.length / 1024).toFixed(0)} KB`);
