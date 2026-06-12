# Zentra — Pure SaaS Transformation TODO Prompts

> Each prompt is self-contained and copy-pasteable into a new Claude Code session.
> Execute in order: SAAS-01 → SAAS-06.
> Deployment pipeline is intentionally last (SAAS-06) — the product is built first.
> Each task ends with a **Done when** acceptance check.
>
> **Context:** Zentra is already running on a GCP Ubuntu/Debian server.
> The goal: remote agents installed in customer datacenters collect infrastructure
> metrics and inject them into the Zentra server on GCP over HTTPS.
> Customers self-onboard. Billing is subscription-based.

---

## SAAS-01 — Fleet Management API (Server-Side Agent Endpoints)

**What this does:** Builds the four server-side API endpoints that the remote agent
(built in SAAS-02) depends on. These must exist on the Zentra server before the
remote agent can be fully tested against a real datacenter.

**Why first:** SAAS-02 (the agent) calls these endpoints. Building the server side
first means you can test the agent end-to-end as soon as it is implemented.

### TODO

1. Add a new database migration that creates an `agent_fleet` table with these
   columns: unique agent UUID (primary key), org_id (foreign key to organizations),
   hashed API key (SHA-256 of the raw key — never store plaintext), datacenter name,
   agent software version, count of active collection targets, last heartbeat timestamp,
   connecting IP address, operating system string, status (`active`, `stale`, `offline`),
   created_at, updated_at. Add indexes on org_id and on last_seen timestamp.

2. Add `POST /api/agents/register` — requires an authenticated user session (Org Admin
   or Super Admin role only):
   - Accepts: datacenter name, optional description, optional region label.
   - Generates a cryptographically random API key with the prefix `isok_` — this prefix
     is required because the existing OTLP key validation function checks for it.
   - Stores the SHA-256 hash of the key in `agent_fleet`.
   - Also inserts a corresponding row into the existing `otlp_api_keys` table so the
     OTLP ingest endpoints (which already validate that table) accept pushes from
     this agent without any additional changes to the ingest pipeline.
   - Returns the plaintext API key exactly once. It cannot be retrieved again.

3. Add `POST /api/agents/heartbeat` — authenticated by the agent API key passed in
   the `X-Zentra-Key` header (not a user session). The existing `validate_otlp_key`
   function handles this validation — reuse it:
   - Accepts: agent software version string, count of targets currently being
     collected, OS name, datacenter name.
   - Upserts the matching row in `agent_fleet`: update last_seen to now, version,
     target count, status to `active`, IP from the request.
   - Returns HTTP 200 with the current server UTC timestamp so the agent can log
     clock drift.
   - Add a background task (runs every 60 seconds) that sets status to `stale` for
     agents where last_seen is between 5 and 15 minutes ago, and `offline` for
     agents where last_seen is more than 15 minutes ago.

4. Add `GET /api/agents/targets` — authenticated by agent API key in `X-Zentra-Key`:
   - Resolves the org_id from the API key using `validate_otlp_key`.
   - Returns all active targets for that org in the same JSON shape the existing
     target list endpoint returns — the agent deserialises them into the same
     `Target` struct it already uses internally.
   - Only return targets where `is_active = true` and `status` is not `Critical`
     after 5 or more consecutive failures.

5. Add `GET /api/agents` — requires user session (Org Admin or higher):
   - Returns all agents registered for the requesting user's org.
   - Each row: datacenter name, status, last_seen as ISO timestamp, version,
     active target count, IP, region label.

6. Add `DELETE /api/agents/:agent_id` — Org Admin or higher:
   - Sets the agent status to `deregistered` in `agent_fleet`.
   - Sets `enabled = false` on the matching row in `otlp_api_keys`.
   - Subsequent heartbeats or OTLP pushes from that key receive 401.

**Done when:**
- `POST /api/agents/register` returns a key starting with `isok_` that is not
  stored in plaintext anywhere in the database.
- That key, when used in `X-Zentra-Key`, successfully passes `validate_otlp_key`
  and returns the correct org_id.
- `GET /api/agents/targets` returns the org's active targets when called with the key.
- Heartbeat updates `last_seen` in real time. An agent silent for 5 minutes turns
  `stale` automatically; silent for 15 minutes turns `offline`.
- Deleting an agent causes the next request from that key to return 401.

---

## SAAS-02 — Remote Agent: DC-to-Cloud Data Collection

**What this does:** Converts the agent from a process that requires a direct PostgreSQL
connection into a standalone binary that runs inside a customer's datacenter, collects
infrastructure metrics from every monitored system, and pushes that data to the Zentra
server on GCP over HTTPS — with no database access required and full offline resilience.

**This is the core SaaS change. Read the architecture section fully before implementing.**

---

### Zentra Application Architecture — Agent Context

The implementer must understand the following before writing a single line of code.

#### How Zentra Works Today (Current Flow)

```
zentra-agent (local mode)
    │
    ├── reads targets ──────────────────────► PostgreSQL DB (direct connection)
    │
    ├── dispatches to plugins
    │     ├── server-plugin   (SSH)
    │     ├── database-plugin (Oracle, MySQL, PostgreSQL, MongoDB, Redis)
    │     ├── storage-plugin  (NetApp, Pure, SAN, NFS, Ceph)
    │     └── network-plugin  (SNMP, BGP, OSPF, NetFlow, Firewall)
    │
    ├── Orchestrator runs one collection loop per target
    │     ├── calls plugin.initialize(target)    — opens connection
    │     ├── calls plugin.collect_metrics(target) — returns Vec<Metric>
    │     ├── calls plugin.collect_logs(target)    — returns Vec<LogEntry>
    │     ├── calls plugin.health_check(target)    — bool
    │     └── sleeps plugin.polling_interval()     — default 60s
    │
    ├── sends MetricBatch via mpsc channel
    │
    └── IngestionService (channel consumer)
          └── writes metrics ──────────────────► PostgreSQL DB (direct)
```

#### How It Must Work After This Prompt (Remote Mode)

```
zentra-agent (remote mode, runs in customer DC)
    │
    ├── fetches targets ─────────────────────► GET /api/agents/targets
    │                                          (X-Zentra-Key: isok_xxx header)
    │
    ├── dispatches to plugins (UNCHANGED — all plugin code stays the same)
    │     ├── server-plugin   (SSH)
    │     ├── database-plugin (Oracle, MySQL, PostgreSQL, MongoDB, Redis)
    │     ├── storage-plugin  (NetApp, Pure, SAN, NFS, Ceph)
    │     └── network-plugin  (SNMP, BGP, OSPF, NetFlow, Firewall)
    │
    ├── Orchestrator (UNCHANGED — same retry, backoff, health check logic)
    │
    ├── sends MetricBatch via mpsc channel (UNCHANGED)
    │
    └── RemoteDeliveryService (replaces IngestionService)
          ├── serialises MetricBatch → OTLP JSON format
          ├── if online:  POST /otlp/v1/metrics  (X-Zentra-Key header)
          │               POST /otlp/v1/logs      (X-Zentra-Key header)
          └── if offline: write to local disk buffer → retry when reconnected
```

#### The Data Models the Agent Works With

**`Target`** — what the agent collects from. Key fields:
- `target_id: Uuid` — unique ID, must be passed back in every metric
- `org_id: Uuid` — tenant identifier, must be passed back in every metric
- `target_type: TargetType` — `Server`, `Database`, `Storage`, `Network`, `Middleware`
- `target_subtype: TargetSubtype` — the specific system: `Linux`, `Oracle`, `NetApp`,
  `Interface`, `Redis`, etc. (full enum in the core crate)
- `host: String` — IP or hostname to connect to
- `port: u16` — connection port
- `username / password` — credentials for SSH, database, SNMP community string, etc.
- `is_active: bool` — only active targets should be collected
- `check_interval_seconds: Option<u32>` — per-target override for polling interval

**`Metric`** — a single time-series data point. Key fields:
- `timestamp: DateTime<Utc>`
- `org_id: Uuid` — must match the target's org_id
- `target_id: Uuid` — must match the target being collected
- `plugin_id: String` — which plugin produced this (e.g. `"server-plugin"`)
- `resource_type: String` — what kind of resource (e.g. `"cpu"`, `"disk"`, `"memory"`)
- `resource_id: String` — which specific resource (e.g. `"cpu0"`, `"sda1"`, `"eth0"`)
- `metric_name: String` — the metric name (e.g. `"cpu_usage_percent"`)
- `value: f64`
- `unit: String`
- `tags: BTreeMap<String, String>` — arbitrary key-value labels
- `metadata: Option<serde_json::Value>` — extra structured data

**`MetricBatch`** — what the Orchestrator sends via channel:
- `metrics: Vec<Metric>`
- `source_plugin: String`
- `collected_at: DateTime<Utc>`

#### Plugin Dispatch Rules (which plugin handles which TargetSubtype)

The existing `create_plugin_for_target` function in the agent crate handles this
mapping. Do not change it. The mapping is:

| TargetType  | TargetSubtype examples                           | Plugin           |
|-------------|--------------------------------------------------|------------------|
| Server      | Linux, Windows, Container, Kubernetes, VSphere   | server-plugin    |
| Server      | Snmp, Ipmi, BMC, ESXi, VCenter                   | server-plugin    |
| Database    | PostgreSQL, MySQL, MariaDB, Oracle, MongoDB, Redis | database-plugin |
| Storage     | LocalDisk, NFS, SAN, NetApp, PureStorage, Ceph   | storage-plugin   |
| Network     | Interface, Switch, Router, Firewall, BGP, OSPF   | network-plugin   |
| Network     | NetFlow, SFlow, Ipfix, Dns, Dhcp, Vpn            | network-plugin   |
| Middleware  | Nginx, Apache, Elasticsearch, HAProxy            | server-plugin    |

#### The Orchestrator (DO NOT MODIFY)

`Orchestrator::run_collection_loop` handles:
- Plugin initialisation with retry and exponential backoff (base 1s, max 60s)
- Stopping after 5 consecutive initialisation failures
- Collecting metrics on each tick via `plugin.collect_metrics(target)`
- Health checks after each collection
- Stopping after 5 consecutive collection failures
- Periodic eBPF profiling for Server targets (every 1 hour)
- Graceful shutdown via CancellationToken

This code must not be changed. The Orchestrator sends collected batches into the
`mpsc::Sender<MetricBatch>` channel. The delivery mechanism on the other end of
that channel is what changes.

#### The OTLP Ingest Endpoint on the Zentra Server

The server already has these endpoints (no changes needed on the server side):

```
POST /otlp/v1/metrics   — accepts OTLP JSON format, validates X-Zentra-Key header
POST /otlp/v1/logs      — same
POST /otlp/v1/traces    — same
```

The server's `validate_otlp_key` function:
- Requires the key to start with `isok_`
- SHA-256 hashes the key and looks it up in `otlp_api_keys`
- Returns `(org_id, permissions)` — the org_id is used to scope all stored data

Authentication header: `X-Zentra-Key: isok_<key>`

#### The OTLP JSON Format the Server Expects for Metrics

```json
{
  "resourceMetrics": [{
    "resource": {
      "attributes": [
        {"key": "service.name",    "value": {"stringValue": "zentra-agent"}},
        {"key": "zentra.org_id",   "value": {"stringValue": "<org_id_uuid>"}},
        {"key": "zentra.target_id","value": {"stringValue": "<target_id_uuid>"}},
        {"key": "host.name",       "value": {"stringValue": "<target.host>"}},
        {"key": "zentra.dc_name",  "value": {"stringValue": "<ZENTRA_DC_NAME env var>"}}
      ]
    },
    "scopeMetrics": [{
      "scope": {"name": "<plugin_id>", "version": "1.0.0"},
      "metrics": [{
        "name": "<metric_name>",
        "unit": "<unit>",
        "gauge": {
          "dataPoints": [{
            "timeUnixNano": "<timestamp as nanoseconds>",
            "asDouble": <value>,
            "attributes": [
              {"key": "resource_type", "value": {"stringValue": "<resource_type>"}},
              {"key": "resource_id",   "value": {"stringValue": "<resource_id>"}},
              <one attribute per entry in metric.tags>
            ]
          }]
        }
      }]
    }]
  }]
}
```

Group all metrics from the same `(org_id, target_id, plugin_id)` combination into a
single `resourceMetrics` entry. Do not send one HTTP request per metric — batch them.

For logs, map `LogEntry` fields to OTLP `resourceLogs` format equivalently.

---

### TODO

1. Add three new fields to the `AppConfig` struct and its `from_env()` loader:
   - `agent_mode`: reads `ZENTRA_AGENT_MODE`, accepts `"local"` or `"remote"`,
     defaults to `"local"`. In local mode, the current behaviour is completely
     unchanged.
   - `api_endpoint`: reads `ZENTRA_API_ENDPOINT`. Required when mode is `remote`.
     Fail at startup with a clear error message if mode is remote and this is unset.
   - `agent_api_key`: reads `ZENTRA_AGENT_API_KEY`. Required when mode is `remote`.
     Must start with `isok_` — validate this at startup and exit if it does not.
   - `dc_name`: reads `ZENTRA_DC_NAME`, defaults to the machine hostname. This is
     sent in every heartbeat and included as a resource attribute in every OTLP push.

2. In the agent startup sequence, branch on `agent_mode`:
   - **`local` mode**: run exactly as today — create DB pool, start IngestionService,
     load targets from DB, run advisory lock election. Zero changes to this path.
   - **`remote` mode**: the new path described in the steps below. No DB pool is
     created. No advisory lock is attempted.

3. Implement a `RemoteDeliveryService` that replaces `IngestionService` in remote mode:
   - Receives `MetricBatch` from the same `mpsc::Receiver<MetricBatch>` channel the
     Orchestrator already sends to — the channel interface is unchanged.
   - Receives `LogBatch` from the `mpsc::Receiver<LogBatch>` channel.
   - For each received batch, serialises it into OTLP JSON format as described in
     the architecture section above. Group metrics by target to minimise request count.
   - Attempts to POST to `/otlp/v1/metrics` (or `/otlp/v1/logs`) with:
     - `Content-Type: application/json`
     - `X-Zentra-Key: <agent_api_key>`
   - On success (HTTP 200 or 202): discard the batch, increment a success counter.
   - On failure (network error, 5xx, timeout): write the serialised batch to the
     local offline buffer (step 4) and log a warning. Do not block the channel.
   - On 401 or 403: log a fatal error and stop the agent — the key is invalid or
     revoked. Do not retry authentication failures.

4. Implement a local offline buffer for network outages:
   - Use a simple append-only file in the agent's working directory, one JSON line
     per batch (newline-delimited JSON). This requires no external dependencies.
   - Before processing new batches from the channel, attempt to flush the buffer
     first — oldest entries first.
   - Flush up to 50 buffered batches per flush cycle to avoid overwhelming the server
     after a long outage.
   - Cap the buffer at 10,000 entries. When full, drop the oldest entry and log a
     warning. Never grow the buffer unbounded.
   - On clean shutdown (ctrl-c received), flush the buffer before exiting.

5. Implement remote target fetching:
   - On startup (remote mode), call `GET /api/agents/targets` with the agent API key
     in the header. Parse the response as a JSON array of `Target` objects — the
     response shape matches the existing `Target` struct exactly.
   - Start a collection loop for each target using the existing Orchestrator and
     `create_plugin_for_target` — these are unchanged.
   - Every 60 seconds, re-fetch the target list. For each target ID in the new list
     that is not already running, start a new collection loop. For each target ID
     that was running but is no longer in the list, cancel its CancellationToken.
     This allows adding and removing targets from the Zentra UI without restarting
     the agent.

6. Implement the heartbeat loop:
   - Every 30 seconds, POST to `/api/agents/heartbeat` with:
     - `agent_version`: the binary's version string (from `Cargo.toml`)
     - `active_targets`: current count of running collection loops
     - `os_name`: operating system (read from `std::env::consts::OS`)
     - `dc_name`: value of `ZENTRA_DC_NAME` or hostname
   - On success: log the returned server timestamp and check for clock drift > 60s.
   - On failure: log a warning, continue — a failed heartbeat must never stop collection.

7. Remove the PostgreSQL advisory lock (`leader_election::try_acquire`) from the
   remote mode code path entirely. In remote mode there is one agent per datacenter —
   no distributed election is needed or possible without a shared database.

8. The eBPF profiler inside the Orchestrator calls the database pool directly
   (`EbpfProfiler::new(pool, org_id)`). In remote mode, pass a no-op stub or
   `sqlx::PgPool` that is unconnected. The profiler's calls will fail silently and
   the rest of the collection loop will continue unaffected.

9. On graceful shutdown (ctrl-c):
   - Cancel all collection loop tokens.
   - Wait up to 5 seconds for in-flight plugin calls to finish.
   - Flush the offline buffer.
   - Call `zentra_core::telemetry::shutdown_telemetry()` to flush OTel spans.
   - Exit cleanly.

**Done when:**
- Starting the agent with `ZENTRA_AGENT_MODE=remote`, a valid API endpoint, and a
  valid `isok_` key causes it to start with no database connection and no advisory
  lock messages in the logs.
- Targets added in the Zentra UI appear in the agent's collection loops within 90
  seconds, without restarting the agent.
- Metrics from a monitored server appear in the Zentra dashboard within 2 collection
  cycles of the agent starting.
- Disconnecting the agent's network for 5 minutes and reconnecting causes all
  queued batches to flush to the server with no data loss.
- An agent with a revoked API key stops immediately with a clear log message rather
  than retrying indefinitely.
- An agent running in `local` mode is completely unaffected — no regression.
- The Orchestrator's retry, backoff, health check, and shutdown logic is identical
  to local mode — it has no knowledge of whether delivery is local or remote.

---

## SAAS-03 — Tenant Self-Service Onboarding

**What this does:** Today creating a new tenant requires a Super Admin to call the
tenant API manually. For SaaS, a new customer must be able to sign up on their own
with no manual intervention — their org, admin user, and first agent API key are
created automatically in one step.

### TODO

1. Add a new public endpoint `POST /api/signup` (no authentication required):
   - Accepts: organisation name, admin email, admin password, optional plan name
     (defaults to `starter`).
   - Validates email is not already registered, password is at least 12 characters.
   - In a single database transaction:
     a. Creates a new row in `organizations` using the plan quota logic already
        defined in the existing tenant handler's `plan_defaults()` function.
     b. Creates the admin user with `Org Admin` role, hashed password.
     c. Runs the existing provisioning logic to seed default dashboards for the org.
     d. Calls the agent registration logic from SAAS-01 to generate the first agent
        API key automatically.
   - Returns: org_id, a signed JWT for immediate login, and the first agent API key
     (plaintext, one time only, same as `POST /api/agents/register`).
   - On any failure, rolls back the entire transaction — no partial state left behind.

2. Add email verification:
   - After successful signup, generate a time-limited token (24 hours) and send a
     verification email if an SMTP host is configured in environment variables.
   - If no SMTP is configured, skip the email and mark the org as verified immediately
     so local and test environments work without an email service.
   - Add `POST /api/signup/verify-email` that accepts the token and marks the org
     email as verified.
   - Unverified orgs are fully functional but the frontend shows a banner.

3. Add `GET /api/onboarding/checklist` (requires user session):
   - Returns completion status for each onboarding step, derived from live DB state:
     email verified, first agent registered and active, first target added, first
     metric received in the last hour, first dashboard viewed.
   - No separate tracking table — compute each boolean from existing tables.

4. Enforce all plan quotas consistently across every create operation:
   - On `POST /api/targets`: check `org_settings.max_targets`. Return HTTP 402 if exceeded.
   - On user invite: check `org_settings.max_users`. Return HTTP 402 if exceeded.
   - On dashboard create: check `org_settings.max_dashboards`. Return HTTP 402 if exceeded.
   - The error response must name the specific limit, the current count, and the plan
     that would unlock more.

5. Rate-limit `POST /api/signup` to 5 attempts per IP address per hour to prevent
   abuse. This limit is separate from the general API rate limiter.

**Done when:**
- A completely new user can POST to `/api/signup` and immediately use the returned
  JWT to access the application and the returned API key with the agent.
- The new org's default dashboards exist after signup.
- Attempting to add more targets than the plan allows returns HTTP 402 with a
  message that names the limit and the plan tier that unlocks more.
- Signing up twice with the same email returns a clear validation error, not a 500.

---

## SAAS-04 — Agent Install Wizard (Frontend)

**What this does:** Closes the end-to-end customer experience. After signup, a
guided UI walks the customer through registering their datacenter, copying the agent
install command, and confirms when the agent has connected and started collecting.

### TODO

1. Add a new page to the frontend accessible from the sidebar under a "Fleet" section.
   Present it automatically after signup when no agents are yet registered.
   The page is a four-step wizard with a progress indicator.

2. **Step 1 — Register Datacenter:**
   - Form: datacenter name, optional region label.
   - On submit, calls `POST /api/agents/register`.
   - Shows the returned API key in a read-only box with a one-click copy button.
   - Shows a prominent, styled warning: "This key will not be shown again. Copy it now."
   - A checkbox the user must tick confirming they have copied the key before
     the Next button becomes active.

3. **Step 2 — Install the Agent:**
   - Two tabs: Linux and Windows.
   - Linux tab: a single shell command that downloads the agent binary from the
     known public download URL, makes it executable, and runs it with all required
     environment variables pre-filled — ZENTRA_API_ENDPOINT set to the current
     server's URL, ZENTRA_AGENT_API_KEY set to the key from Step 1,
     ZENTRA_DC_NAME set to the datacenter name entered in Step 1,
     ZENTRA_AGENT_MODE set to `remote`.
   - Include a one-click copy button on the command block.
   - A collapsible section below the command showing the equivalent systemd unit
     file content for running the agent as a persistent background service.

4. **Step 3 — Waiting for Connection:**
   - Poll `GET /api/agents` every 5 seconds looking for the newly registered agent
     to appear with status `active`.
   - Show a spinner: "Waiting for agent to connect…"
   - On success: animate a checkmark and advance to Step 4 automatically.
   - After 5 minutes with no connection: show a troubleshooting checklist — check
     firewall allows outbound HTTPS, verify the API key was copied correctly, verify
     the server URL is reachable from the DC.

5. **Step 4 — Targets Discovered:**
   - Call `GET /api/targets` and show targets added in the last 10 minutes.
   - Show a list of discovered servers, databases, and devices.
   - A "Go to Dashboard" button navigates to the Command Center.
   - If no targets yet: show a "Add target manually" button.

6. Add a **Fleet page** listing all agents for the org. For each agent: datacenter name,
   status indicator (colour-coded), last seen as a relative duration ("3 minutes ago"),
   version, active target count, IP address. A "Register new datacenter" button links
   to the wizard. A deregister button per row calls `DELETE /api/agents/:id` with a
   confirmation dialog before proceeding.

7. Add an **Onboarding checklist widget** to the Command Center dashboard, visible until
   all steps are complete. Reads from `GET /api/onboarding/checklist`. Each incomplete
   step is a clickable item linking to the relevant page. Once all steps are complete
   the widget is hidden permanently for that org.

**Done when:**
- A new user can complete signup → install agent → see first metrics in under 10
  minutes with no documentation.
- The waiting spinner in Step 3 transitions to success automatically when the agent
  first heartbeats.
- Deregistering an agent from the Fleet page causes its next OTLP push to return 401.

---

## SAAS-05 — Billing and Plan Enforcement (Stripe)

**What this does:** Replaces the current binary license key model with Stripe
subscriptions. Customers select a plan, pay, and their account is upgraded automatically.
Downgrade, cancellation, and payment failure are all handled by Stripe webhooks with
no manual intervention.

### TODO

1. Remove the existing license-plugin enforcement middleware from the API request
   pipeline. Replace it with the subscription status check described in step 3.

2. Configure three Stripe products (Starter, Professional, Enterprise) with monthly
   prices in the Stripe dashboard. Store the Stripe secret key, webhook signing secret,
   and each price ID in Secret Manager. Never put these values in the codebase.

3. Add `GET /api/billing/subscription` (requires user session):
   - Returns: plan name, status (`active`, `trialling`, `past_due`, `cancelled`),
     current period end date, and a map of which features are enabled.
   - This is the single source of truth for all feature gating in the backend.

4. Add `POST /api/billing/checkout` (Org Admin only):
   - Accepts the desired plan name.
   - Creates a Stripe Checkout Session for the matching price ID.
   - Returns the Stripe-hosted checkout URL. The frontend redirects the user to it.

5. Add `POST /api/billing/webhook` (public — Stripe calls this directly):
   - Validate the Stripe webhook signature using the signing secret. Return 400 on
     invalid signatures — never process unsigned events.
   - Handle: `checkout.session.completed` → set org status to `active`, update plan.
   - Handle: `invoice.payment_succeeded` → extend period, ensure status is `active`.
   - Handle: `invoice.payment_failed` → set status to `past_due`. 7-day grace period
     before restricting write access.
   - Handle: `customer.subscription.deleted` → set status to `cancelled`. Downgrade
     quotas to free limits. Preserve all data.
   - Respond HTTP 200 immediately and process asynchronously. Never block on DB writes.

6. Add `POST /api/billing/portal` (Org Admin only):
   - Creates a Stripe Customer Portal session and returns the URL.
   - Customers use this to update payment methods, view invoices, and cancel.

7. Add billing pages to the frontend:
   - **Current Plan** — plan name, status, renewal date, usage gauges (targets used
     vs max, users used vs max). Upgrade button for non-enterprise plans. Manage
     Billing button opens the Stripe portal.
   - **Plan Comparison** — table of all three tiers with limits and pricing.
   - **Past Due Banner** — shown site-wide when status is `past_due`, links to
     the billing portal to resolve the payment issue.

8. Implement access control based on subscription status in a single middleware
   function applied to all write endpoints (POST, PUT, PATCH, DELETE):
   - `active` / `trialling`: full access.
   - `past_due`: read-only for 7 days from first failure, then same as cancelled.
   - `cancelled`: login and data export only. No monitoring operations.

**Done when:**
- A new signup can select a plan, pay with a Stripe test card, and receive `active`
  status — no manual action required on your side.
- A failed payment transitions the org to `past_due` and shows the banner.
- A cancelled subscription blocks new targets immediately.
- No binary license key code remains in the codebase.

---

## SAAS-06 — Automated Build and Deploy Pipeline (Cloud Build → GCP VM)

**What this does:** Every push to the main branch automatically compiles the Rust
backend and React frontend on GCP's build infrastructure (Linux x86_64 — no local
compilation involved), packages them into Docker images, pushes to Artifact Registry,
and redeploys on the running GCP server automatically.

**Why last:** The product is fully built and validated before automating its deployment.

### TODO

1. Create a Cloud Build trigger in GCP that fires on every push to the `main` branch.

2. Write the Cloud Build pipeline configuration with these steps:
   - **Step 1:** Compile both Rust binaries (`zentra-api` and `zentra-agent`) using the
     official Rust Docker image on Linux x86_64. Use the production feature flags as
     defined in the existing Makefile. Enable Cargo registry and target directory
     caching between builds so subsequent builds complete in under 5 minutes.
   - **Step 2:** Build the backend Docker image using the existing backend Dockerfile
     in the deploy folder. Tag with the git commit SHA.
   - **Step 3:** Build the frontend Docker image using the existing frontend Dockerfile.
     Tag with the git commit SHA.
   - **Step 4:** Push both images to GCP Artifact Registry.
   - **Step 5:** SSH into the GCP VM using a key stored in Secret Manager, pull the
     new images, and restart the services. Wait for the health endpoint at `/api/health`
     to return healthy before considering the deploy successful.

3. The deploy step on the VM must:
   - Accept the new image tag as a parameter.
   - Pull new backend and frontend images.
   - Restart backend first, wait for health check, then restart frontend.
   - Clean up images older than the last 3 tags to prevent disk exhaustion.

4. Store all secrets used by Cloud Build (SSH key, any deployment credentials) in
   Secret Manager. Grant the Cloud Build service account permission to read them.
   No secrets in the repository or in the build config file.

5. Store all Zentra runtime secrets (database password, JWT secret, Stripe keys,
   agent API keys) in Secret Manager. The deploy script injects them as environment
   variables at container start — not from a file on disk.

6. Set a GCP Budget Alert at $150 so you are notified before reaching half the
   $300 free credit.

**Done when:**
- A `git push` to `main` triggers Cloud Build automatically.
- The build compiles and produces both binaries and images without errors.
- Both images are visible in Artifact Registry tagged with the commit SHA.
- The GCP VM serves the new version within 3 minutes of the build completing.
- The `/api/health` endpoint returns healthy after each deploy.
- Rolling back is possible by re-running the deploy step with an older image tag.
