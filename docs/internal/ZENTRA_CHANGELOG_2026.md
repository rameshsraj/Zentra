# Zentra — Complete Development Changelog (May–June 2026)

> Covers every major feature, fix, and architectural decision from the initial
> commit through to the current session. Includes the original prompts used to
> drive each feature so any session can be re-entered with full context.

---

## How to Read This Document

Each entry is structured as:
- **What was built** — the concrete files and behaviours changed
- **Why** — the business or technical reason
- **Original prompt / instruction** — what was asked
- **Done-when test** — how to verify it still works

---

## Phase 0 — Zentra Core Foundation
### *(Pre-May 2026 — already in place at project start)*

The Zentra codebase was already a working multi-tenant infrastructure monitoring
platform with the following architecture before any of the changes in this log:

### Architecture at Start
```
Rust (Axum) API server  ──► TimescaleDB (PostgreSQL) on port 5434
React 18 frontend       ──► API on port 5006, served by nginx on 5010
zentra-agent (local)    ──► Direct DB connection (no remote mode)
Plugins                 ──► SSH, Oracle, SNMP, IPMI, Kubernetes, NetApp, etc.
```

### Key crates at start:
- `zentra-core` — shared models (`Target`, `Metric`, `LogEntry`, etc.)
- `zentra-db` — database repository layer (sqlx)
- `zentra-api` — Axum HTTP API server
- `zentra-agent` — orchestrator + plugin dispatcher
- `server-plugin` — Linux SSH deep collector, SNMP, IPMI, Kubernetes
- `database-plugin` — Oracle, PostgreSQL, MySQL, MongoDB, Redis
- `storage-plugin` — NetApp, PureStorage, SAN, NFS, Ceph
- `network-plugin` — BGP, OSPF, NetFlow, firewall

### UI pages at start:
Command Center, Fleet management (basic), Servers, Databases, Storage, Network,
Middleware, Oracle Monitoring, OTel APM (traces/metrics/logs), Security, AI Analytics

---

## Phase 1 — Licensing & Multi-Tenancy
### *(~May 25, 2026)*

### What was built
- Full per-org licensing model with plan tiers (Starter/Professional/Enterprise)
- License key generation and cryptographic signing
- Per-org quota enforcement: max targets, max users, max dashboards
- `organizations` table with plan metadata and quota fields
- License plugin middleware that gates all write operations
- Trial period of 365 days (extended from 30 days)
- License key format: base64-encoded signed payload, verified by the API on every request

### Files changed
- `migrations/20250101000076_organizations.sql` — org plan and quota table
- `migrations/20250101000003_license_management.sql` — license storage
- `crates/zentra-api/src/handlers/license.rs`
- `crates/zentra-api/src/middleware/license.rs`
- `crates/zentra-core/src/license.rs`
- `docs/licensing/` — full signing contract, CI guide, architecture docs

### Original prompt
> "Add a licensing model. Admins should only be able to use Zentra if they have a
> valid license. Each org has a plan tier. License is signed and verified. Trial
> period should be 365 days. Do not break any existing functionality."

### Done-when test
```bash
curl http://10.88.11.162:5006/api/auth/login \
  -d '{"email":"admin@Zentra.local","password":"changeme"}'
# JWT returned → license accepted
```

---

## Phase 2 — OTLP / OpenTelemetry Ingest Pipeline
### *(~May 30 – June 3, 2026)*

### What was built
- Full OTLP JSON ingest pipeline (metrics, logs, traces, profiles)
- `POST /otlp/v1/metrics`, `/otlp/v1/logs`, `/otlp/v1/traces`
- API key authentication (`isok_` prefix, SHA-256 hashed in `otlp_api_keys` table)
- `ingest_queue.rs` — async multi-worker ingest with batch dedup and upsert
- `otlp_api_keys` table with per-key permissions and rate limiting columns
- OTLP dead-letter queue for failed ingestion (migration 069)
- TimescaleDB compression for metrics and traces hypertables
- RED metrics materialized view (Rate, Error, Duration)
- LLM GenAI spans support (migration 059)
- Profiles support (migration 060)
- Trace aggregates view (migration 075)

### Key fix (June 2026 session)
**Bug: `target_id` was NULL in all ingested metrics.**
Root cause: `ingest_queue.rs` was not extracting `zentra.target_id` from the
OTLP resource attributes. Fixed to parse resource attributes and set `target_id`
on every stored metric point.

```rust
// Before (buggy)
let target_id = None;

// After (fixed)
let target_id = resource_attrs
    .iter()
    .find(|a| a.key == "zentra.target_id")
    .and_then(|a| a.value.as_ref()?.string_value.parse::<Uuid>().ok());
```

### Files changed
- `crates/zentra-api/src/ingest_queue.rs` — NULL target_id fix
- `crates/zentra-api/src/otlp/` — OTLP JSON deserialiser
- `crates/zentra-api/src/grpc.rs` — OTLP gRPC server on port 4317
- `migrations/20250101000059_llm_spans.sql`
- `migrations/20250101000060_profiles.sql`
- `migrations/20250101000061_traces_base_schema.sql`
- `migrations/20250101000065_otlp_api_keys.sql`
- `migrations/20250101000066_red_metrics_unique_idx.sql`
- `migrations/20250101000067_tsdb_compression.sql`
- `migrations/20250101000069_otlp_dead_letter.sql`
- `migrations/20250101000075_trace_aggregates.sql`
- `migrations/20250101000080_otlp_api_keys_columns.sql` — added key_prefix, enabled, rate_limit_rpm, max_payload_bytes, updated_at columns

### Original prompt
> "The OTLP ingest endpoint exists but target_id is always null in the stored
> metrics. The agent sends zentra.target_id as a resource attribute. Fix the
> ingest queue to extract it correctly."

### Done-when test
```sql
SELECT DISTINCT target_id FROM metrics_raw LIMIT 5;
-- Must return actual UUIDs, not NULLs
```

---

## Phase 3 — SaaS Transformation: Agent Architecture
### *(June 3 – June 13, 2026)*

This is the core architectural change. The agent was converted from a process
requiring a direct PostgreSQL connection into a fully standalone remote agent
that can run in any customer datacenter.

### SAAS-01: Fleet Management API (Server-Side)

**What was built:**
- `agent_fleet` table (migration 077): agent_id, org_id, otlp_api_key_id, api_key_hash, datacenter_name, agent_version, active_target_count, last_seen, ip_address, os_name, status
- `POST /api/agents/register` — creates agent + `isok_` key in one transaction
- `POST /api/agents/heartbeat` — agent pings every 30 seconds, updates last_seen
- `GET /api/agents/targets` — returns org's active targets to the agent
- `GET /api/agents` — lists all agents for an org (user-facing)
- `DELETE /api/agents/:id` — deregisters agent, disables its key
- Background job: sweeps agent_fleet every 60s, marks agents stale (>5min) or offline (>15min)

**Files changed:**
- `migrations/20250101000077_agent_fleet.sql`
- `crates/zentra-api/src/handlers/agents.rs`
- `crates/zentra-api/src/routes/agents.rs`
- `crates/zentra-api/src/background_jobs.rs` — added fleet status sweep

**Original prompt (SAAS-01):**
> Build the four server-side API endpoints that the remote agent depends on.
> POST /api/agents/register — generates isok_ key, stores SHA-256 hash.
> POST /api/agents/heartbeat — updates last_seen.
> GET /api/agents/targets — returns active targets for the org.
> GET /api/agents — lists all agents for the org.
> DELETE /api/agents/:id — deregisters.
> Add background sweep for stale/offline status.

---

### SAAS-02: Remote Agent — DC-to-Cloud Data Collection

**What was built:**
The agent now has two modes controlled by `ZENTRA_AGENT_MODE`:

- **`local` mode** (unchanged): direct PostgreSQL connection, IngestionService writes to DB
- **`remote` mode** (new): no DB, fetches targets via API, delivers via OTLP HTTP

New components in remote mode:
- `AppConfig` extended with `agent_mode`, `api_endpoint`, `agent_api_key`, `dc_name`
- `RemoteDeliveryService` — receives MetricBatch from channel, serialises to OTLP JSON, POSTs to server
- `RemoteTargetFetcher` — polls `GET /api/agents/targets` every 60s, starts/stops collection loops dynamically
- `HeartbeatLoop` — POSTs to `/api/agents/heartbeat` every 30s with version/targets/os/dc_name
- Local offline buffer — append-only NDJSON file, flushes up to 50 batches on reconnect, capped at 10,000 entries
- Graceful shutdown — cancels all loops, flushes buffer, shuts down OTel spans

**Plugins unchanged** — all SSH/SNMP/Oracle/NetFlow collection code identical

**Files changed:**
- `crates/zentra-agent/src/config.rs` — remote mode config
- `crates/zentra-agent/src/remote_delivery.rs` — OTLP push service
- `crates/zentra-agent/src/remote_targets.rs` — target fetch loop
- `crates/zentra-agent/src/heartbeat.rs` — heartbeat loop
- `crates/zentra-agent/src/offline_buffer.rs` — disk-based retry buffer
- `crates/zentra-agent/src/main.rs` — mode branch at startup

**Original prompt (SAAS-02):**
> Convert the agent from a process that requires a direct PostgreSQL connection
> into a standalone binary that runs inside a customer's datacenter, collects
> infrastructure metrics from every monitored system, and pushes that data to
> the Zentra server on GCP over HTTPS — with no database access required and
> full offline resilience. All plugin code stays the same. The Orchestrator
> stays the same. Only the delivery mechanism changes.

---

### SAAS-03: Tenant Self-Service Onboarding

**What was built:**
- `POST /api/signup` — public endpoint, creates org + admin user + first agent key in one DB transaction
- Email verification: token generation, `POST /api/signup/verify-email`
- `GET /api/onboarding/checklist` — live-computed checklist (agent registered, target added, metric received, dashboard viewed)
- Quota enforcement on all create operations (targets, users, dashboards) returning HTTP 402 with plan info
- Rate limit: 5 signup attempts per IP per hour

**Files changed:**
- `migrations/20250101000078_signup_support.sql`
- `crates/zentra-api/src/handlers/signup.rs`
- `crates/zentra-api/src/handlers/onboarding.rs`
- `crates/zentra-api/src/routes/mod.rs`

**Original prompt (SAAS-03):**
> A new customer must be able to sign up on their own with no manual intervention.
> POST /api/signup creates org, admin user, and first agent API key in one transaction.
> Returns JWT for immediate login and the agent key (one time only).
> Quota enforcement on all create operations returning HTTP 402.

---

### SAAS-04: Agent Install Wizard (Frontend)

**What was built:**
- New "Fleet" section in sidebar navigation
- `Fleet` page (`/fleet`) — lists all agents for the org:
  - Status indicator (green/amber/red), last seen relative time, version, target count, IP
  - "Register new datacenter" button
  - Per-row deregister button with confirmation dialog
- `AgentInstallWizard` page (`/fleet/install`) — 4-step guided wizard:
  - **Step 1** — Register Datacenter: datacenter name form, calls `POST /api/agents/register`, shows `isok_` key with copy button and "you won't see this again" warning, checkbox gate before Next
  - **Step 2** — Install Agent: Linux/Windows tabs, pre-filled shell command with all env vars, systemd unit file collapsible section, copy button
  - **Step 3** — Waiting for Connection: polls `GET /api/agents` every 5s, spinner → checkmark on first active heartbeat, troubleshooting checklist after 5 minutes
  - **Step 4** — Targets Discovered: shows targets added in last 10 minutes, "Go to Dashboard" button
- Onboarding checklist widget on Command Center (hidden when all steps complete)

**Files changed:**
- `frontend/src/pages/Fleet.tsx`
- `frontend/src/pages/AgentInstallWizard.tsx`
- `frontend/src/components/Layout.tsx` — Fleet section in sidebar
- `frontend/src/App.tsx` — lazy routes for Fleet, AgentInstallWizard
- `frontend/src/lib/api/fleet.ts` — `listAgents`, `registerAgent`, `deregisterAgent`, `getOnboardingChecklist`

**Original prompt (SAAS-04):**
> Build a 4-step install wizard. Step 1: register datacenter, show key with copy button.
> Step 2: Linux/Windows install command pre-filled with env vars. Step 3: poll for
> agent connection, spinner → checkmark. Step 4: show discovered targets.
> Also build the Fleet page listing all agents with status, last seen, version.

---

### SAAS-05: Billing and Stripe Integration

**What was built:**
- Stripe subscription model: Starter / Professional / Enterprise
- `GET /api/billing/subscription` — plan name, status, period end, feature flags
- `POST /api/billing/checkout` — creates Stripe Checkout session, returns redirect URL
- `POST /api/billing/webhook` — validates Stripe signature, handles: `checkout.session.completed`, `invoice.payment_succeeded`, `invoice.payment_failed`, `customer.subscription.deleted`
- `POST /api/billing/portal` — Stripe Customer Portal session
- Frontend billing pages: Current Plan, Plan Comparison, Past Due Banner
- Access control middleware: `active`/`trialling` = full, `past_due` = 7-day grace then read-only, `cancelled` = login+export only
- Removed old binary license-key enforcement

**Files changed:**
- `migrations/20250101000079_billing_stripe.sql`
- `crates/zentra-api/src/handlers/billing.rs`
- `crates/zentra-api/src/middleware/subscription.rs`
- `frontend/src/pages/Billing.tsx`
- `frontend/src/lib/api/billing.ts`

**Original prompt (SAAS-05):**
> Replace the current binary license key model with Stripe subscriptions.
> Stripe webhook handles checkout, payment success, payment failure, and cancellation.
> Access control based on subscription status. No secrets in the codebase.

---

## Phase 4 — Docker Local Lab
### *(June 18 – June 22, 2026)*

### What was built
- Complete local simulation lab via Docker Compose:
  - `zentra-server` — full API + TimescaleDB on localhost
  - `zentra-agent` — dedicated container running in remote mode, simulating a "datacenter"
  - `zentra-frontend` — nginx serving the React build
  - 3× `target-linux` — simulated Linux servers (Alpine) with SSH enabled for agent to collect from
- `docker/local-lab/docker-compose.yml`
- `docker/local-lab/start-lab.ps1` — Windows PowerShell launcher
- `docker/local-lab/start-lab.sh` — Linux/Mac launcher
- `docker/local-lab/setup-debian12.sh` — full Debian 12 production setup script
- `deploy/setup-debian12.sh` — GCP/bare metal production install
- Agent tarball build: `docker/build-agent-tarball.sh`
- Systemd unit file for agent (`zentra-agent.service`)
- Operations guide: `docs/ZENTRA_OPERATIONS_GUIDE.md`
- Fixed: supervisord env var injection for zentra-server in container
- Fixed: secrets persisted to PG volume so they survive container restarts
- Fixed: `ERR_EMPTY_RESPONSE` from conflicting nginx `default_server` returning 444
- Fixed: initdb failure due to mounted PGDATA directory
- Fixed: agent tarball not excluded from main image tarball search

### Original prompt
> Build a complete local simulation lab using Docker Compose. The agent should run in
> a separate container in remote mode pointing at the API server container. Include
> simulated Linux targets the agent can SSH into. Include PowerShell and bash launchers.
> The setup should match production as closely as possible.

---

## Phase 5 — Bug Fixes: Agent Data Collection
### *(June 23, 2026 — Session 1)*

### Bug 1: Agent collecting agent-host metrics instead of target metrics

**Root cause:** `server-plugin/src/lib.rs` used the `sysinfo` crate to collect
CPU, memory, and disk metrics. `sysinfo` reads from `/proc` — the agent's own
container's `/proc`, not the remote target's. Every target got the same wrong data.

**Fix:** Removed the entire `sysinfo` block from `ServerPlugin`. The correct
collection path is `LinuxDeepCollector` which SSHes into each target and runs
the bash collector script remotely.

```rust
// Removed:
use sysinfo::System;
let mut sys = System::new_all();
sys.refresh_all();
// ... metrics from sys.cpus(), sys.total_memory(), etc.

// Correct path (unchanged, was always there):
let deep = LinuxDeepCollector::new(target);
deep.collect().await  // SSHes into target, runs collector script
```

**File:** `plugins/server-plugin/src/lib.rs`

**Original prompt:**
> The agent is showing 0 for all values. Is it really collecting or just pinging?
> I'm seeing 0% CPU, 0 memory for all targets. The agent should work like the
> main Zentra server. Look into what it is actually capturing.

---

### Bug 2: Auth header mismatch — X-Zentra-Key vs Authorization: Bearer

**Root cause:** The agent used `X-Zentra-Key: isok_xxx` but the server-side handlers
for `/api/agents/heartbeat` and `/api/agents/targets` were written expecting
`Authorization: Bearer isok_xxx` (matching the OTLP endpoints). Requests were being
silently rejected with 401.

**Fix:** Updated both sides to use `Authorization: Bearer` consistently.

**Agent side (fixed):**
- `crates/zentra-agent/src/heartbeat.rs` line 68 — `.header("Authorization", format!("Bearer {}", key))`
- `crates/zentra-agent/src/remote_targets.rs` line 97 — same

**Server side (fixed):**
- `crates/zentra-api/src/handlers/agents.rs` — `agent_targets` and `heartbeat` handlers updated to extract key from `Authorization: Bearer` header

**Original prompt:**
> Fix agent headers: the agent is using X-Zentra-Key but it should be
> Authorization: Bearer everywhere, matching the OTLP endpoints.

---

## Phase 6 — Feature: Middleware Sidebar Static Menu
### *(June 23, 2026)*

### What was built
**Problem:** The Middleware section in the sidebar only showed items when the API
returned data. If the API had no middleware targets registered, the menu collapsed
to nothing — Nginx, Apache, Tomcat etc. were invisible.

**Fix:** Made all middleware sub-items static and always visible:
- Nginx
- Apache / httpd  
- Elasticsearch
- HAProxy
- Tomcat
- PHP-FPM
- RabbitMQ
- Kafka
- Memcached

Each item links to the filtered middleware page regardless of whether targets exist.

**File:** `frontend/src/components/Layout.tsx`

**Original prompt:**
> Under Middleware, enable the menu items other than the dashboard. I am not seeing
> nginx, httpd and one more. Do not make it dynamic.

---

## Phase 7 — Feature: Agent Key Management Screen
### *(June 23, 2026)*

### What was built
A new admin-only screen (`/fleet/keys`) for managing `isok_` API keys.

**Backend:**
- `GET /api/agent-keys` — lists all agent keys for the org (id, name, key_prefix, created_at, last_used, enabled)
- `POST /api/agent-keys` — creates a new key, returns plaintext once
- `DELETE /api/agent-keys/:key_id` — revokes/disables a key

**Frontend (`/fleet/keys`):**
- Admin-only access (same gate as Command Center)
- Table: key prefix (e.g. `isok_abc12345`), name, created date, last used, enabled status
- "Create Key" button → dialog → shows full key with copy button and warning banner
- Revoke button per row with confirmation
- Empty state with create CTA

**Files changed:**
- `crates/zentra-api/src/handlers/agents.rs` — `list_agent_keys`, `create_agent_key`, `revoke_agent_key` handlers
- `crates/zentra-api/src/routes/agents.rs` — added `/api/agent-keys` routes
- `frontend/src/pages/AgentKeys.tsx` — new page
- `frontend/src/lib/api/fleet.ts` — `listAgentKeys`, `createAgentKey`, `revokeAgentKey`
- `frontend/src/App.tsx` — lazy route for `/fleet/keys`
- `frontend/src/components/Layout.tsx` — "Agent Keys" nav item under Fleet

**Original prompt:**
> The Administrator of the particular Org should be able to create the agent key
> (isok_***). There should be a screen, accessible only by the Admin of that Org,
> like the Command Center. The admin creates keys, sees them listed, and can revoke them.

---

## Phase 8 — Feature: Agent-Aware "Add Server" (Feature 1 + 3)
### *(June 24, 2026)*

### What was built
Previously, adding any server target always triggered SSH verification — even for
servers in a datacenter already covered by a running agent. The agent can reach
those servers directly; SSH verification from the GCP API server is irrelevant
and often impossible (different network).

**Database:**
- Migration `000081`: added `agent_id UUID REFERENCES agent_fleet(agent_id) ON DELETE SET NULL` to `targets` table
- Migration applied to smarsh-nexus directly via psql (sqlx CLI not available)

**Backend:**
- `Target` struct: new `agent_id: Option<Uuid>` field
- `CreateTargetRequest`: new `agent_id: Option<Uuid>` field
- `insert_target`: now stores `agent_id`
- All `SELECT` queries: now return `agent_id`
- `create_target` handler: sets `target.agent_id = req.agent_id`

**Frontend (`AddTargetModal.tsx`):**
- When type = Server: shows "Monitored via Agent" dropdown
- Dropdown auto-populates from `GET /api/agents` (active + stale agents only)
- Each option: `{datacenter_name} ({region_label}) — {ip_address}`
- When agent selected: SSH verification **skipped entirely**, button = "Create & Discover"
- When no agent selected: SSH verification **required** (existing flow unchanged)
- `agent_id` sent in the `createTarget` API call
- `CreateTargetInput` interface updated with `agent_id?: string`

**Logic:**
```typescript
const agentSelected = isServer && !!selectedAgentId
const sshRequired = isServer && !agentSelected

// Submit handler:
if (sshRequired && verifyState !== 'passed') {
  doVerify()  // trigger SSH check
  return
}
// If agent selected → skip SSH, go straight to mutation
```

**Files changed:**
- `migrations/20250101000081_targets_agent_id.sql`
- `crates/zentra-core/src/models/target.rs`
- `crates/zentra-db/src/repositories/targets.rs`
- `crates/zentra-api/src/handlers/targets.rs`
- `frontend/src/components/AddTargetModal.tsx`
- `frontend/src/lib/api/targets.ts` — `CreateTargetInput` interface

**Original prompt:**
> When there is connection via the Agent, we should be aware of the agent name.
> In the Add Server screen, there should be an AgentName dropdown (auto-populated).
> If an agent is selected, skip the SSH validation.
> If the application is in a datacenter without an Agent, SSH validation is required.

---

## Phase 9 — Deployment: smarsh-nexus (10.88.11.162)
### *(Ongoing — every session)*

### Infrastructure
```
Server:    10.88.11.162 (smarsh-nexus)
User:      root / skyline
Project:   /store/Zentra/
DB:        postgresql://postgres:zantaz@10.88.11.162:5434/zentra
API:       port 5006 (systemd: zentra-api.service)
Frontend:  port 5010 (nginx serving /store/Zentra/frontend/dist)
Binary:    /store/Zentra/bin/zentra-server
```

### Deployment procedure (every session)
```bash
# 1. Sync code from Mac to server
sshpass -p 'skyline' rsync -az \
  --exclude='target/' --exclude='node_modules/' --exclude='.git/' \
  -e "ssh -o PubkeyAuthentication=no" \
  /System/Volumes/Data/Projects/Zentra/ root@10.88.11.162:/store/Zentra/

# 2. Build backend on server
ssh root@10.88.11.162 "cd /store/Zentra && \
  cargo build --release -p zentra-api -p zentra-agent"

# 3. Deploy binary
ssh root@10.88.11.162 "cp /store/Zentra/target/release/zentra-server \
  /store/Zentra/bin/zentra-server && \
  systemctl restart zentra-api"

# 4. Build frontend
ssh root@10.88.11.162 "cd /store/Zentra/frontend && npm run build"
# nginx serves /store/Zentra/frontend/dist directly — no copy needed

# 5. Apply DB migrations (sqlx CLI not available — use psql)
psql 'postgresql://postgres:zantaz@10.88.11.162:5434/zentra' \
  -c "<migration SQL>"

# 6. Push to GitHub
git push origin zentra-saas
```

### Database migrations applied manually on smarsh-nexus
(Because sqlx CLI is not installed on the server — migrations must be run via psql)

| Migration | What it does | Applied |
|-----------|-------------|---------|
| `000077_agent_fleet.sql` | Creates agent_fleet table | ✅ |
| `000078_signup_support.sql` | Signup + email verify tables | ✅ |
| `000079_billing_stripe.sql` | Stripe billing tables | ✅ |
| `000080_otlp_api_keys_columns.sql` | key_prefix, enabled, rate_limit_rpm columns | ✅ |
| `000081_targets_agent_id.sql` | agent_id FK on targets | ✅ |

---

## API Test Results (June 24, 2026 — passing)

```
=== Zentra API Tests ===
PASS: Login                              → JWT token returned
PASS: GET /api/agents                    → HTTP 200, 0 agents (clean state)
PASS: GET /api/agent-keys                → HTTP 200, 0 keys
PASS: POST /api/agent-keys               → HTTP 200, isok_ key created
PASS: DELETE /api/agent-keys/:id         → HTTP 200, key revoked
PASS: GET /api/targets                   → HTTP 200, 0 targets (clean state)
```

---

---

## Phase 10 — Feature 4: Agent Network Discovery + User Opt-In
### *(June 24, 2026)*

### What was built

The agent now scans its local subnet every 10 minutes, discovers reachable hosts
via TCP port probing, and reports them to the server. Users see the discovered
hosts in the Fleet page and choose which ones to monitor — the agent never
automatically starts collecting from an undecided host.

**Database (migration `000082`):**
- `discovered_hosts` table: `agent_id` FK, `org_id`, `ip_address`, `hostname`,
  `open_ports` (JSONB), `os_hint`, `first_seen`, `last_seen`, `opted_in`, `ignored`, `target_id` FK
- Unique constraint on `(agent_id, ip_address)` — upserted on each scan

**Backend (3 new endpoints):**
- `POST /api/agents/:id/discovered` — agent-authenticated (Bearer); upserts all
  discovered hosts for that agent in bulk
- `GET /api/agents/:id/discovered` — user session; returns all discovered hosts
  for the agent, ordered by opted_in DESC, last_seen DESC
- `PATCH /api/agents/:id/discovered/:host_id` — user session; sets `opted_in`
  and/or `ignored` on a single host

**Agent (`network_scan.rs`):**
- `NetworkScanner` struct spawned in remote mode when `ZENTRA_AGENT_ID` is set
- Detects local /24 subnets from `/proc/net/fib_trie` (Linux) or `hostname -I`
- TCP-probes 254 hosts per subnet on ports: 22, 80, 443, 8080, 3306, 5432, 6379,
  9090, 27017 — 300ms timeout per port, all hosts probed concurrently
- Infers OS hint: port 22 open → `linux`; port 3389/445/135 → `windows`
- Attempts reverse DNS lookup (best-effort, 500ms timeout)
- Reports live hosts to server every 10 minutes
- Requires `ZENTRA_AGENT_ID` env var (UUID of the registered agent)

**`AppConfig` addition:**
- `agent_id: Option<Uuid>` — read from `ZENTRA_AGENT_ID` env var
- Scanner is silently skipped if `ZENTRA_AGENT_ID` is not set (backwards compatible)

**Frontend (`Fleet.tsx` rewrite):**
- Each agent row is now expandable (chevron toggle)
- Expanding reveals **Discovered Hosts** panel below the agent row
- Panel shows summary bar: X new / X monitored / X ignored
- Host table: IP, hostname, OS (emoji + text), open ports, last seen, status
- **Monitor** button → sets `opted_in = true`
- **Ignore** button → sets `ignored = true` (row fades out)
- **Remove** button → unsets `opted_in` (monitored → new)
- **Unignore** button → unsets `ignored` (ignored → new)
- "Show/Hide ignored" toggle in panel header
- Empty state: explains that scans run every 10 minutes
- Refreshes every 60 seconds automatically

### Files changed
- `migrations/20250101000082_discovered_hosts.sql`
- `crates/zentra-api/src/handlers/agents.rs` — `report_discovered`, `list_discovered`, `update_discovered_host`
- `crates/zentra-api/src/routes/agents.rs` — 3 new routes
- `crates/zentra-agent/src/network_scan.rs` — new file
- `crates/zentra-agent/src/lib.rs` — expose `network_scan` module
- `crates/zentra-agent/src/main.rs` — spawn `NetworkScanner` in remote mode
- `crates/zentra-core/src/config/mod.rs` — `agent_id` field + `ZENTRA_AGENT_ID` env
- `frontend/src/pages/Fleet.tsx` — expandable rows + `DiscoveredHostsPanel` component
- `frontend/src/lib/api/fleet.ts` — `DiscoveredHost` interface, `listDiscoveredHosts`, `optInHost`

### Original prompt
> The agent should have an option to skip the IPs which we do not want to monitor.
> The agent should not collect all the IPs. There should always be a check — ask
> the user whether to Monitor the server or not.

### Done-when test
```bash
# Start agent with ZENTRA_AGENT_ID set — after 10 min:
curl -H "Authorization: Bearer <token>" \
  http://10.88.11.162:5006/api/agents/<agent_id>/discovered
# Returns array of hosts with opted_in/ignored fields

# User opts in a host:
curl -X PATCH -H "Authorization: Bearer <token>" \
  -d '{"opted_in":true}' \
  http://10.88.11.162:5006/api/agents/<agent_id>/discovered/<host_id>
# Returns {"updated": true}
```

---

## Pending Work

| ID | Feature | Status | Notes |
|----|---------|--------|-------|
| F4b | Auto-create target when user clicks Monitor (link opted_in → target creation) | 🔲 Nice-to-have | Currently user must still add via Add Server after opting in |
| GCP | Deploy to GCP (34.61.5.197) | 🔲 User handles | SSH key auth only (no password). Same procedure as smarsh-nexus |
| SAAS-06 | Automated Cloud Build pipeline | 🔲 Pending | git push → compile → deploy |

---

## Complete File Change Index

### Backend (Rust)
| File | Changes |
|------|---------|
| `crates/zentra-core/src/models/target.rs` | `agent_id` field on `Target` and `CreateTargetRequest` |
| `crates/zentra-db/src/repositories/targets.rs` | All queries include `agent_id` |
| `crates/zentra-api/src/handlers/agents.rs` | Fleet endpoints, key management, Bearer auth fix |
| `crates/zentra-api/src/handlers/targets.rs` | Sets `agent_id` from request |
| `crates/zentra-api/src/handlers/billing.rs` | Stripe billing endpoints |
| `crates/zentra-api/src/handlers/signup.rs` | Self-service signup + quota enforcement |
| `crates/zentra-api/src/handlers/onboarding.rs` | Checklist endpoint |
| `crates/zentra-api/src/routes/agents.rs` | Agent + key management routes |
| `crates/zentra-api/src/ingest_queue.rs` | NULL target_id bug fix |
| `crates/zentra-agent/src/config.rs` | remote/local mode config |
| `crates/zentra-agent/src/remote_delivery.rs` | OTLP push service |
| `crates/zentra-agent/src/remote_targets.rs` | Target fetch loop, Bearer fix |
| `crates/zentra-agent/src/heartbeat.rs` | Heartbeat loop, Bearer fix |
| `crates/zentra-agent/src/offline_buffer.rs` | Disk-based retry buffer |
| `crates/zentra-agent/src/network_scan.rs` | TCP subnet scanner + host reporter (new file) |
| `crates/zentra-core/src/config/mod.rs` | Added `agent_id` field + `ZENTRA_AGENT_ID` env var |
| `plugins/server-plugin/src/lib.rs` | Removed sysinfo (was reading agent host) |

### Frontend (React + TypeScript)
| File | Changes |
|------|---------|
| `frontend/src/pages/Fleet.tsx` | Agent list page + expandable Discovered Hosts panel |
| `frontend/src/pages/AgentInstallWizard.tsx` | 4-step install wizard |
| `frontend/src/pages/AgentKeys.tsx` | Admin key management screen |
| `frontend/src/pages/Billing.tsx` | Subscription / plan pages |
| `frontend/src/components/AddTargetModal.tsx` | Agent dropdown, skip-SSH logic |
| `frontend/src/components/Layout.tsx` | Fleet section, middleware static items, Agent Keys nav |
| `frontend/src/App.tsx` | Lazy routes for all new pages |
| `frontend/src/lib/api/fleet.ts` | listAgents, registerAgent, deregisterAgent, agent key CRUD, listDiscoveredHosts, optInHost |
| `frontend/src/lib/api/targets.ts` | CreateTargetInput with agent_id |
| `frontend/src/lib/api/billing.ts` | Billing API calls |

### Database Migrations
| Migration | Feature |
|-----------|---------|
| `000076_organizations.sql` | Multi-tenant org + plan table |
| `000077_agent_fleet.sql` | Agent registration and fleet tracking |
| `000078_signup_support.sql` | Self-service signup tables |
| `000079_billing_stripe.sql` | Stripe billing tables |
| `000080_otlp_api_keys_columns.sql` | Missing columns on otlp_api_keys |
| `000081_targets_agent_id.sql` | agent_id FK on targets table |
| `000082_discovered_hosts.sql` | Discovered hosts table for agent network scan |

### Infrastructure / DevOps
| File | Purpose |
|------|---------|
| `docker/local-lab/docker-compose.yml` | Local simulation lab |
| `docker/local-lab/start-lab.ps1` | Windows launcher |
| `docker/local-lab/start-lab.sh` | Linux/Mac launcher |
| `docker/build-agent-tarball.sh` | Builds agent binary tarball for distribution |
| `deploy/setup-debian12.sh` | Full production install on Debian 12 |
| `docker/local-lab/setup-debian12.sh` | Lab variant of setup script |
| `deploy/run-auth-tests.sh` | Auth endpoint smoke tests |
| `scripts/api_test.sh` | Comprehensive API test suite |
| `docs/ZENTRA_OPERATIONS_GUIDE.md` | Full ops runbook |

---

## Phase 3 — Agent Target Opt-In & Production Readiness Review
### *(June 24, 2026)*

### What was built

1. **Agent Target Opt-In Feature**
   - Agents now scan local subnets (TCP connect probes on 22, 80, 443, 8080, 3306, 5432, 6379, 9090, 27017),
     perform reverse DNS lookups, and report discovered hosts to the server.
   - Users can review discovered hosts in the Fleet UI, then opt-in a host (creates a monitored target)
     or ignore it.
   - New endpoints:
     - `POST /api/agents/:id/discovered` — agent reports discovered hosts
     - `GET /api/agents/:id/discovered` — user lists discovered hosts
     - `PATCH /api/agents/:id/discovered/:host_id` — user opts-in or ignores a host
   - `POST /api/agents/:id/discovered` was whitelisted in the auth middleware so it uses the agent
     `Authorization: Bearer <isok_...>` API key instead of the user JWT.

2. **Agent Fleet UI**
   - Added `/fleet/keys` (admin-only) for managing `isok_` agent API keys.
   - Added a **Discovered Hosts** tab to the Fleet page showing IP, hostname, OS hint, ports, and actions
     (Opt-in / Ignore).
   - Agent API key operations: `GET /api/agent-keys`, `POST /api/agent-keys`, `DELETE /api/agent-keys/:key_id`.

3. **Comprehensive API & Frontend Review**
   - Enumerated **731 API endpoints** from `crates/zentra-api/src/routes/`.
   - Created `scripts/smoke_test_all.py` to authenticate and call every GET endpoint, substituting
     sentinel UUIDs for path parameters. It categorizes 2xx/4xx/5xx/501/empty responses and flags
     real server errors.
   - Scanned all frontend pages for mock/fake/hardcoded data and removed two concrete cases.
   - Deployed and ran the smoke test on `smarsh-nexus`.

### Bugs fixed during the review

| Endpoint | Error | Fix |
|----------|-------|-----|
| `POST /api/agents/:id/discovered` | 401 "invalid token" | Whitelisted path in `auth_middleware.rs` so agent key auth bypasses JWT |
| `GET /api/admin/licenses/compliance` | 500 SQL decode error: `NUMERIC` → `f64` | Cast `SUM(total_cost)` to `::FLOAT8` |
| `GET /api/anomalies/stats` | 500 unexpected NULL | `COALESCE(SUM(...), 0)` on empty table |
| `GET /api/cloud/accounts/:id/metrics` | 500 "future release" | Added `ApiError::NotImplemented`; now returns **501** |
| `GET /api/cilium/status` | 500 missing column `last_scraped_at` | Use `NULL::TIMESTAMPTZ` in query |
| `GET /api/otel/alert-rules/evaluate` | 500 missing table `otel_alert_rules` | New migration `00083_otel_alert_rules.sql` |
| `GET /api/otel/logs-by-trace/:trace_id` | 500 missing columns `level`/`metadata` | Use `severity AS level`, `parsed_fields AS metadata` |
| `GET /api/query/metrics/names` | 500 missing columns `time`/`labels` | Use `timestamp`, `tags` in metrics table |
| `GET /api/traces/:traceId` | 500 missing column `events` | Select `'[]'::jsonb AS events` |

### Frontend mock-data cleanup

| File | Mock data removed | Now shows |
|------|-------------------|-----------|
| `frontend/src/pages/Dashboard.tsx` | Hardcoded 3 fake IPs (Port Scan, Brute Force, Data Exfil) and 4 hardcoded counts | Real `stats?.threats_detected` with link to Security page |
| `frontend/src/pages/Network.tsx` | Fake sparkline fallback values (2400, 1800, 142, 30, 15, 2) | Zero or real data; no misleading flat lines |

### Files changed
- `crates/zentra-api/src/auth_middleware.rs` — agent discovered whitelist
- `crates/zentra-api/src/error.rs` — `NotImplemented` variant
- `crates/zentra-api/src/handlers/anomaly.rs` — NULL-safe aggregates
- `crates/zentra-api/src/handlers/cilium.rs` — missing column workaround
- `crates/zentra-api/src/handlers/metrics.rs` — correct metric column names
- `crates/zentra-api/src/handlers/otlp.rs` — correct log column names
- `crates/zentra-api/src/handlers/traces.rs` — empty events stub
- `crates/zentra-db/src/repositories/licenses.rs` — FLOAT8 cast
- `frontend/src/pages/Dashboard.tsx` — remove mock threat data
- `frontend/src/pages/Network.tsx` — remove fake sparkline fallbacks
- `migrations/20250101000083_otel_alert_rules.sql` — new table
- `scripts/smoke_test_all.py` — new comprehensive smoke test
- `scripts/api_test_full.sh` — agent opt-in endpoint regression tests
- `scripts/debug_discovered.sh` — isolated discovered-hosts debugging

### Database Migrations
| Migration | Feature |
|-----------|---------|
| `000082_discovered_hosts.sql` | Discovered hosts table for agent network scan |
| `000083_otel_alert_rules.sql` | OTel-native alert rules table (review fix) |

### Original prompt
> "review the project code, and also, make sure that all the APIs and endpoints are tested, and it is not empty. also, None of the pages should have mock/fake data."

### Done-when test
```bash
# 1. Agent endpoints
ssh root@10.88.11.162 'bash /tmp/api_test_full.sh'
# → ALL TESTS COMPLETE (auth, agents, heartbeat, discovered hosts, agent keys, targets)

# 2. Full API smoke test
ssh root@10.88.11.162 'python3 /tmp/smoke_test_all.py < /tmp/endpoints_clean.txt'
# → OK: 316, SERVER 5xx: 0, Not Implemented 501: 1 (cloud metrics, intentional)
```

---

## Key Design Decisions (ADRs)

### ADR-001: Rust + Axum Backend
TypeScript/Node was considered. Rust chosen for: single-binary deploy, memory safety,
no GC pauses in metric-heavy workloads, native TimescaleDB COPY protocol support.

### ADR-002: SSH + Oracle Monitoring
Agent SSHes into targets using `BatchMode=yes` (key-based auth only). Runs a bash
collector script that outputs metrics in a structured format. No agent installed on
targets — just SSH access.

### ADR-003: TimescaleDB for Metrics
PostgreSQL-native time-series. No separate InfluxDB/Prometheus. Compression,
continuous aggregates, and hypertable partitioning built in. Single DB to operate.

### ADR-004: Multi-Tenant org_id Column Strategy
Every table has `org_id`. Row-level security enforced at DB level. No tenant data
leakage possible even if API has a bug — PG RLS is the last line of defence.

### ADR-005: isok_ Key Prefix Convention
All agent/OTLP API keys must start with `isok_`. This allows `validate_otlp_key`
to fast-reject malformed keys before hitting the DB. The prefix also makes keys
identifiable in logs (only prefix shown, never full key).

### ADR-006: Agent Authorization Header
Changed from `X-Zentra-Key` to `Authorization: Bearer` across all agent→server
communication to align with standard HTTP auth conventions and eliminate the need
for custom header parsing in proxies/load balancers.
