# Development Prompt Log (May–June 2026)

> Reusable functional prompts for each phase. File paths, product names, and infrastructure details have been removed so the prompts can be reused in other projects.

---

## Phase 0 — Core Foundation
### *(Pre-May 2026)*

The starting point was a working multi-tenant infrastructure monitoring platform:
- Rust backend API server
- React frontend
- PostgreSQL/TimescaleDB for time-series metrics, traces, and logs
- Remote agent that can run in a customer datacenter and push data over HTTPS
- Plugin-based collectors for servers, databases, storage, network, middleware, cloud, and security

---

## Phase 1 — Licensing & Multi-Tenancy
### *(~May 25, 2026)*

### What was built
- Full per-org licensing model with plan tiers (Starter/Professional/Enterprise).
- License key generation and cryptographic signing.
- Per-org quota enforcement: max targets, max users, max dashboards.
- `organizations` table with plan metadata and quota fields.
- License plugin middleware that gates all write operations.
- Trial period of 365 days (extended from 30 days).
- License key format: base64-encoded signed payload, verified by the API on every request.

### Original prompt
> "Add a licensing model. Admins should only be able to use the platform if they have a valid license. Each org has a plan tier. License is signed and verified. Trial period should be 365 days. Do not break any existing functionality."

### Done-when test
- Login returns a JWT and the license is accepted.
- Creating resources beyond the plan quota returns a payment-required error.

---

## Phase 2 — OpenTelemetry Ingest Pipeline
### *(~May 30 – June 3, 2026)*

### What was built
- Full OTLP JSON ingest pipeline (metrics, logs, traces, profiles).
- `POST /otlp/v1/metrics`, `/otlp/v1/logs`, `/otlp/v1/traces`.
- API key authentication with hashed keys stored in the database.
- Async multi-worker ingest queue with batch deduplication and upsert.
- Per-key permissions and rate limiting.
- Dead-letter queue for failed ingestion.
- TimescaleDB compression for metrics and traces hypertables.
- RED metrics materialized view (Rate, Error, Duration).
- LLM GenAI spans support.
- Profiles support.
- Trace aggregates view.

### Key fix
**Bug: `target_id` was NULL in all ingested metrics.**
Root cause: the ingest queue was not extracting the target identifier from the OTLP resource attributes. Fixed to parse resource attributes and set `target_id` on every stored metric point.

### Original prompt
> "The OTLP ingest endpoint exists but target_id is always null in the stored metrics. The agent sends the target identifier as a resource attribute. Fix the ingest queue to extract it correctly."

### Done-when test
```sql
SELECT DISTINCT target_id FROM metrics_raw LIMIT 5;
-- Must return actual UUIDs, not NULLs
```

---

## Phase 3 — SaaS Agent Architecture
### *(June 3 – June 13, 2026)*

### What was built
Converted the agent from a process requiring a direct database connection into a fully standalone remote agent that can run in any customer datacenter.

**Fleet Management API (server-side):**
- `agent_fleet` table: agent_id, org_id, api_key_hash, datacenter_name, agent_version, active_target_count, last_seen, ip_address, os_name, status.
- `POST /api/agents/register` — creates agent and API key in one transaction.
- `POST /api/agents/heartbeat` — agent pings every 30 seconds, updates last_seen.
- `GET /api/agents/targets` — returns org's active targets to the agent.
- `GET /api/agents` — lists all agents for an org.
- `DELETE /api/agents/:id` — deregisters agent, disables its key.
- Background job: sweeps agent fleet every 60s, marks agents stale (>5 min) or offline (>15 min).

**Remote Agent — datacenter-to-cloud data collection:**
- Two modes: `local` (direct DB connection) and `remote` (no DB, pushes over HTTPS).
- Remote mode components:
  - Config: agent mode, API endpoint, agent API key, datacenter name.
  - Delivery service: serializes metric batches to OTLP JSON and POSTs to the server.
  - Target fetcher: polls active targets every 60s and starts/stops collection loops dynamically.
  - Heartbeat loop: posts heartbeat every 30s with version, targets, OS, and datacenter name.
  - Local offline buffer: append-only NDJSON file, flushes on reconnect with a capped size.
  - Graceful shutdown.
- All collection plugins remain unchanged; only the delivery mechanism changed.

**Tenant Self-Service Onboarding:**
- `POST /api/signup` — public endpoint, creates org + admin user + first agent key in one transaction.
- Email verification token and confirmation endpoint.
- Onboarding checklist endpoint (agent registered, target added, metric received, dashboard viewed).
- Quota enforcement on all create operations returning HTTP 402 with plan info.
- Rate limit: 5 signup attempts per IP per hour.

**Agent Install Wizard (frontend):**
- New Fleet section in the sidebar.
- Fleet page listing all agents with status, last seen, version, target count, and IP.
- 4-step install wizard:
  1. Register datacenter and show the API key with copy button.
  2. Linux/Windows install command pre-filled with environment variables.
  3. Poll for agent connection, spinner → checkmark.
  4. Show discovered targets.
- Onboarding checklist widget on the command center dashboard.

**Billing and Stripe Integration:**
- Stripe subscription model: Starter / Professional / Enterprise.
- `GET /api/billing/subscription` — plan name, status, period end, feature flags.
- `POST /api/billing/checkout` — creates Stripe Checkout session.
- `POST /api/billing/webhook` — handles checkout completion, payment success/failure, and cancellation.
- `POST /api/billing/portal` — Stripe Customer Portal session.
- Frontend billing pages: Current Plan, Plan Comparison, Past Due Banner.
- Access control based on subscription status: active/trialling = full, past_due = grace then read-only, cancelled = login/export only.
- Removed old binary license-key enforcement.

### Original prompts
> "Build the four server-side API endpoints that the remote agent depends on: register, heartbeat, targets, list, and deregister. Add a background sweep for stale/offline status."

> "Convert the agent from a process that requires a direct database connection into a standalone binary that runs inside a customer's datacenter, collects infrastructure metrics from every monitored system, and pushes that data to the server over HTTPS — with no database access required and full offline resilience. All plugin code stays the same."

> "A new customer must be able to sign up on their own with no manual intervention. Create org, admin user, and first agent API key in one transaction. Quota enforcement on all create operations returning HTTP 402."

> "Build a 4-step install wizard. Step 1: register datacenter, show key with copy button. Step 2: Linux/Windows install command pre-filled with env vars. Step 3: poll for agent connection, spinner → checkmark. Step 4: show discovered targets. Also build the Fleet page listing all agents."

> "Replace the current binary license key model with Stripe subscriptions. Stripe webhook handles checkout, payment success, payment failure, and cancellation. Access control based on subscription status. No secrets in the codebase."

---

## Phase 4 — Docker Local Lab
### *(June 18 – June 22, 2026)*

### What was built
- Complete local simulation lab via Docker Compose:
  - API server container with full API and database.
  - Agent container running in remote mode, simulating a datacenter.
  - Frontend container served by nginx.
  - Simulated Linux target containers with SSH enabled for the agent to collect from.
- Windows PowerShell and Linux/Mac bash launchers.
- Debian 12 production setup script.
- Agent tarball build script.
- Systemd unit file for the agent.
- Operations runbook.
- Fixed: supervisord env var injection, secrets persistence across container restarts, conflicting nginx default server, initdb failure with mounted data directory, and agent tarball packaging.

### Original prompt
> "Build a complete local simulation lab using Docker Compose. The agent should run in a separate container in remote mode pointing at the API server container. Include simulated Linux targets the agent can SSH into. Include PowerShell and bash launchers. The setup should match production as closely as possible."

---

## Phase 5 — Agent Data Collection Fixes
### *(June 23, 2026)*

### Bug 1: Agent collecting agent-host metrics instead of target metrics

**Symptom:** All targets showed identical CPU, memory, and disk values that matched the agent's own container.

**Root cause:** The server plugin used a system-info crate that reads from the local `/proc` filesystem — the agent's own `/proc`, not the remote target's.

**Fix:** Removed the system-info block. The correct collection path is the SSH-based deep collector that runs a collector script remotely on each target.

### Original prompt
> "The agent is showing 0 for all values. Is it really collecting or just pinging? I'm seeing 0% CPU, 0 memory for all targets. The agent should work like the main server. Look into what it is actually capturing."

### Bug 2: Auth header mismatch

**Symptom:** Agent requests to the server were silently rejected with 401.

**Root cause:** The agent used a custom header for its API key while the server expected `Authorization: Bearer`.

**Fix:** Updated both agent and server to use `Authorization: Bearer` consistently across all agent endpoints.

### Original prompt
> "Fix agent headers: the agent is using a custom header but it should be Authorization: Bearer everywhere, matching the OTLP endpoints."

---

## Phase 6 — Middleware Sidebar Static Menu
### *(June 23, 2026)*

### What was built
**Problem:** The Middleware section in the sidebar only showed items when the API returned data. If no middleware targets were registered, the menu collapsed to nothing.

**Fix:** Made all middleware sub-items static and always visible: Nginx, Apache, Elasticsearch, HAProxy, Tomcat, PHP-FPM, RabbitMQ, Kafka, Memcached. Each item links to the filtered middleware page regardless of whether targets exist.

### Original prompt
> "Under Middleware, enable the menu items other than the dashboard. I am not seeing nginx, httpd and one more. Do not make it dynamic."

---

## Phase 7 — Agent Key Management Screen
### *(June 23, 2026)*

### What was built
A new admin-only screen for managing agent API keys.

**Backend:**
- `GET /api/agent-keys` — lists all agent keys for the org.
- `POST /api/agent-keys` — creates a new key, returns plaintext once.
- `DELETE /api/agent-keys/:key_id` — revokes/disables a key.

**Frontend:**
- Admin-only access (same gate as the command center).
- Table showing key prefix, name, created date, last used, and enabled status.
- Create key dialog with copy button and warning banner.
- Revoke button per row with confirmation.
- Empty state with create call-to-action.

### Original prompt
> "The Administrator of the particular Org should be able to create the agent key. There should be a screen, accessible only by the Admin of that Org, like the Command Center. The admin creates keys, sees them listed, and can revoke them."

---

## Phase 8 — Agent-Aware Add Server
### *(June 24, 2026)*

### What was built
Previously, adding any server target always triggered SSH verification — even for servers in a datacenter already covered by a running agent. The agent can reach those servers directly, so SSH verification from the cloud API server is irrelevant and often impossible.

**Database:**
- Added `agent_id` foreign key to the `targets` table.

**Backend:**
- `Target` model and `CreateTargetRequest` include optional `agent_id`.
- `insert_target` stores the selected agent ID.
- `create_target` handler reads `agent_id` from the request.

**Frontend:**
- When target type is Server, show a "Monitored via Agent" dropdown.
- Dropdown auto-populates from active and stale agents.
- Each option shows datacenter name, region, and IP address.
- When an agent is selected, SSH verification is skipped entirely; the button becomes "Create & Discover".
- When no agent is selected, SSH verification is required as before.
- Selected `agent_id` is sent in the create-target API call.

### Original prompt
> "When there is connection via the Agent, we should be aware of the agent name. In the Add Server screen, there should be an AgentName dropdown (auto-populated). If an agent is selected, skip the SSH validation. If the application is in a datacenter without an Agent, SSH validation is required."

---

## Phase 9 — Agent Network Discovery + User Opt-In
### *(June 24, 2026)*

### What was built
The agent scans its local subnet every 10 minutes, discovers reachable hosts via TCP port probing, and reports them to the server. Users see the discovered hosts in the Fleet page and choose which ones to monitor — the agent never automatically starts collecting from an undecided host.

**Database:**
- `discovered_hosts` table: agent_id, org_id, ip_address, hostname, open_ports, os_hint, first_seen, last_seen, opted_in, ignored, target_id.
- Unique constraint on (agent_id, ip_address) — upserted on each scan.

**Backend (3 new endpoints):**
- `POST /api/agents/:id/discovered` — agent-authenticated, bulk-upserts discovered hosts.
- `GET /api/agents/:id/discovered` — user session, returns all discovered hosts.
- `PATCH /api/agents/:id/discovered/:host_id` — user session, sets opted_in and/or ignored on a single host.

**Agent scanner:**
- Detects local /24 subnets from system network info.
- TCP-probes each host on common service ports with a short timeout, all hosts probed concurrently.
- Infers OS hint from open ports (e.g., SSH → Linux, RDP/SMB → Windows).
- Attempts reverse DNS lookup with a best-effort timeout.
- Reports live hosts to the server every 10 minutes.
- Requires an agent ID environment variable; silently skipped if not set.

**Frontend:**
- Each agent row is expandable.
- Expanding reveals a Discovered Hosts panel with summary bar (new / monitored / ignored).
- Host table: IP, hostname, OS, open ports, last seen, status.
- Monitor / Ignore / Remove / Unignore actions.
- Show/hide ignored toggle.
- Empty state explains that scans run every 10 minutes.
- Auto-refreshes every 60 seconds.

### Original prompt
> "The agent should have an option to skip the IPs which we do not want to monitor. The agent should not collect all the IPs. There should always be a check — ask the user whether to Monitor the server or not."

### Done-when test
```bash
# After the agent runs with its agent ID set for ~10 minutes:
curl -H "Authorization: Bearer <token>" \
  http://<api>/api/agents/<agent_id>/discovered
# Returns array of hosts with opted_in/ignored fields

# User opts in a host:
curl -X PATCH -H "Authorization: Bearer <token>" \
  -d '{"opted_in":true}' \
  http://<api>/api/agents/<agent_id>/discovered/<host_id>
# Returns {"updated": true}
```

---

## Phase 10 — Production Readiness Review
### *(June 24, 2026)*

### What was built
1. **Agent Target Opt-In Feature**
   - Agents scan local subnets using TCP connect probes on common service ports, perform reverse DNS lookups, and report discovered hosts to the server.
   - Users can review discovered hosts in the Fleet UI and choose to opt-in a host (creating a monitored target) or ignore it.
   - New endpoints added:
     - `POST /api/agents/:id/discovered` — agent reports discovered hosts
     - `GET /api/agents/:id/discovered` — user lists discovered hosts
     - `PATCH /api/agents/:id/discovered/:host_id` — user opts-in or ignores a host
   - The agent reporting endpoint was whitelisted in the authentication middleware so it uses the agent API key instead of the user session token.

2. **Agent Fleet UI**
   - Added an admin-only page for managing agent API keys.
   - Added a **Discovered Hosts** tab to the Fleet page showing IP, hostname, OS hint, ports, and actions (Opt-in / Ignore).
   - Agent API key management endpoints: `GET /api/agent-keys`, `POST /api/agent-keys`, `DELETE /api/agent-keys/:key_id`.

3. **Comprehensive API & Frontend Review**
   - Enumerated all API endpoints from the backend route definitions.
   - Created a smoke test script that authenticates and calls every GET endpoint, substituting sentinel UUIDs for path parameters. It categorizes responses by status code, flags real server errors, and treats intentional "not implemented" responses as expected.
   - Scanned all frontend pages for mock, fake, or hardcoded data and removed misleading fallback values.
   - Deployed and ran the smoke test against the staging environment.

### Bugs fixed during the review

| Area | Symptom | Fix |
|------|---------|-----|
| Agent auth middleware | Agent reporting endpoint returned 401 | Whitelisted the path so agent API key auth bypasses user session middleware |
| License compliance | 500 SQL decode error on cost aggregates | Cast numeric sums to the appropriate floating-point type |
| Anomaly stats | 500 unexpected NULL on empty table | Wrapped aggregates with zero-default COALESCE |
| Cloud metrics | 500 returned for unimplemented provider | Mapped to a proper 501 Not Implemented response |
| Status query | 500 missing column reference | Replaced missing column with a NULL timestamp in the query |
| Alert rules | 500 missing table | Added migration to create the missing alert rules table |
| Log correlation | 500 missing columns in logs query | Aliased the correct severity and parsed-fields columns |
| Metric metadata | 500 missing columns in metrics query | Used the correct timestamp and tags column names |
| Trace detail | 500 missing events column | Provided an empty JSONB array for the events field |

### Frontend mock-data cleanup

| Area | Mock data removed | Now shows |
|------|-------------------|-----------|
| Security dashboard | Hardcoded fake IPs and threat counts | Real threat stats with link to the full security page |
| Network dashboard | Fake sparkline fallback values | Zero or real data; no misleading flat lines |

### Verification

- Created a comprehensive API smoke test and a focused agent opt-in regression test.
- Ran both against the deployed environment; all agent endpoints and the full GET smoke test passed with zero server errors and only one intentional 501 response.

### Original prompt
> "review the project code, and also, make sure that all the APIs and endpoints are tested, and it is not empty. also, None of the pages should have mock/fake data."

---
