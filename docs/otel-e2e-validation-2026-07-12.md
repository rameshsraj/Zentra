# Zentra API-Driven OTel and Agent Validation

Date: 2026-07-12

## Scope

Validated against:

- Server: `172.19.155.52`
- Client: `172.19.156.110`
- API credentials: `admin@Zentra.local`

Goals:

- Build on the Linux server, not on the Windows workstation.
- Create fresh enrollment and OTLP keys through the API.
- Register the Linux client through the product flow.
- Use `zentra-agent` and `zentra-collector` instead of the upstream Linux `otelcol-contrib` installer path.
- Install nginx, Apache, and HAProxy on the client, generate traffic, and verify discovery.
- Confirm that OpenTelemetry ingestion works through the server APIs.
- Create users through the API and validate role boundaries.

## What Was Verified

### Server build

Built on `172.19.155.52`:

- frontend via Vite build
- `zentra-api`
- `zentra-agent`
- `zentra-collector`

Verified live binary download endpoints:

- `/dl/zentra-agent-linux-x86_64`
- `/dl/zentra-collector-linux-x86_64`

### API-driven key creation

Created through live APIs:

- org enrollment key for remote registration
- OTLP API key for telemetry ingest

### Client registration

Verified active agents:

- `silent-172-19-156-110`
- `silent-172-19-156-110-collector`

### Middleware installation and traffic

Installed on `172.19.156.110`:

- Apache
- nginx
- HAProxy

Configured ports:

- Apache: `8080`
- nginx: `8081`
- HAProxy: `8088`

Generated traffic locally on the client with repeated `curl` requests to all three listeners.

### Discovery and confirmation

The client agent discovered:

- Apache
- nginx
- HAProxy
- Linux host
- log directories

Confirmed middleware components through:

- `POST /api/agents/components/confirm`

The main client agent converged to `active_target_count = 4`.

### OpenTelemetry ingest

Posted OTLP telemetry directly to the server:

- `POST /v1/traces`
- `POST /v1/logs`

Verified in:

- `GET /api/otel/resources`
- `GET /api/otel/stats`

Observed service:

- `middleware-demo`

### RBAC and user lifecycle

Created through live APIs:

- `rbac-admin-20260712@zentra.local` with role `Admin`
- `rbac-viewer-20260712@zentra.local` with role `Viewer`
- `rbac-operator-20260712@zentra.local` with role `Operator`

Validated:

- `Super Admin` could create `Admin`
- `Admin` could create `Viewer`
- `Admin` could create `Operator`
- `Admin` could not create another `Admin`
- `Viewer` could log in but was forbidden from:
  - `POST /api/users`
  - `GET /api/org-enrollment-keys`
  - `GET /api/agents`
- `Operator` could log in but was forbidden from:
  - `POST /api/agents/components/confirm`
  - `GET /api/agents`

## Issues Found

### 1. Existing collector install was broken by stale key material

On the client, `zentra-collector.service` was in a restart loop with:

- `Heartbeat rejected (401 Unauthorized): API key revoked or invalid`

Cause:

- stale key file / prior key material under `/zentra/etc/collector.key`
- old upstream `otelcol-contrib` path still enabled beside the Zentra collector path

Fix applied:

- disabled `otelcol-contrib.service`
- removed stale collector key material
- restarted `zentra-collector` with a fresh API-created enrollment key

Result:

- `zentra-collector.service` became active
- collector registered as `silent-172-19-156-110-collector`

### 2. Linux installer path in Agent Install Wizard was still pointing to upstream OTel in some flows

Updated locally and on the server build for Linux collector/network snippets:

- switched Linux collector install flow to `zentra-collector`
- switched Linux network install flow to `zentra-collector`

Relevant file:

- `frontend/src/pages/AgentInstallWizard.tsx`

### 3. Wizard still references upstream collector for Windows flows

Still present in the wizard:

- Windows collector install snippet
- Windows network/SNMP snippet

Those sections still download `otelcol-contrib_windows_amd64.tar.gz`.

This is a product gap unless a hosted Windows `zentra-collector` artifact exists and the API exposes it.

### 4. Pipeline API does not reflect active collector state

After the collector was active and telemetry ingest was working, this still returned empty:

- `GET /api/otel/pipeline`

Observed response:

- `{"collectors":[],"flow":[]}`

This looks like a missing integration between live collector registration / OTLP activity and the pipeline view.

### 5. Middleware detection port mapping is not accurate for non-default ports

The discovered middleware components were promoted with port `80` based on process heuristics, even though the services were actually configured on:

- Apache `8080`
- nginx `8081`
- HAProxy `8088`

This likely comes from the process-name fallback logic in component discovery and can create incorrect monitoring targets when middleware runs on non-default ports.

### 6. Non-admin users return permissions but empty module projection

For both `Viewer` and `Operator`, `GET /api/auth/me` returned meaningful permission sets, but:

- `module_access: []`
- `menu_items: []`

This suggests the UI/menu projection for non-admin roles is incomplete or not automatically populated even though permissions exist.

## Recommended Product Follow-ups

1. Finish replacing remaining wizard references to upstream `otelcol-contrib` where Zentra-hosted artifacts exist.
2. Add a supported Windows `zentra-collector` artifact if Windows collector installs are expected.
3. Fix collector/pipeline linkage so `GET /api/otel/pipeline` reflects real active collectors and flows.
4. Improve component discovery to prefer bound listening ports over process-name defaults when both are available.
5. Define and populate non-admin `module_access` so Viewer and Operator users get a usable product surface.
6. Add an automated regression test for stale key recovery on collector restart.

## Final State

Working:

- server-side build on Linux
- API login
- API key creation
- Linux client agent registration
- Linux client collector registration
- middleware installation
- middleware discovery
- middleware target confirmation
- traffic generation
- OTLP trace/log ingest
- user creation and RBAC validation

Still missing or incomplete in product behavior:

- Windows wizard still points to upstream collector
- pipeline view does not show the active flow
- middleware port attribution is inaccurate on non-default ports
- non-admin module projection is empty even when permissions exist
