# Zentra Runbook

## What Zentra Is

Zentra is an infrastructure observability and operations platform. It combines:

- asset and component discovery
- agent-based monitoring
- agentless collector workflows
- logs, metrics, traces, and profiles through OpenTelemetry-style ingest
- fleet management for monitored machines and discovered middleware
- RBAC for platform administration and day-to-day operators

In practice, Zentra is trying to be the control plane for servers, middleware, databases, network targets, and telemetry pipelines from one product surface.

## Core Product Capabilities

### 1. Identity and access

- JWT-based authentication
- role-driven authorization
- user and role management
- org-scoped data access

Key roles observed in the live system:

- `Super Admin`
- `Admin`
- `Viewer`
- `Operator`
- `Editor`
- `Agent`

### 2. Fleet and discovery

- remote agent registration through org enrollment keys
- heartbeat and target refresh
- component discovery by host, process, and port
- component promotion into monitored targets

### 3. OpenTelemetry and ingest

- OTLP HTTP and gRPC endpoints
- traces, metrics, logs, and profiles
- API-key protected ingest
- OTel resource, stats, trace, and alert-rule APIs

### 4. Monitoring domains

- servers / Linux hosts
- middleware
- databases
- network targets
- logs and traces

## High-Level Architecture

### Control plane

- `zentra-api`
  - authentication
  - RBAC
  - user management
  - fleet management
  - OTel APIs
  - binary download endpoints under `/dl/...`

### Data plane

- `zentra-agent`
  - registers to server
  - heartbeats
  - discovers local components
  - collects metrics/logs
  - reports discovered components

- `zentra-collector`
  - remote/agentless-oriented collector path
  - uses Zentra-hosted registration flow
  - intended replacement for direct Linux upstream `otelcol-contrib` use in product-driven flows

### UI

- frontend app with install wizard, fleet surfaces, telemetry views, and admin areas

## Important Live Endpoints

### Auth

- `POST /api/auth/login`
- `POST /api/auth/change-password`
- `GET /api/auth/me`

### Users and roles

- `GET /api/users`
- `POST /api/users`
- `GET /api/roles`

### Agent fleet

- `POST /api/agents/register`
- `POST /api/agents/heartbeat`
- `GET /api/agents`
- `GET /api/agents/by-dc/:dc_name/components`
- `POST /api/agents/components/confirm`
- `POST /api/org-enrollment-keys`
- `GET /api/org-enrollment-keys`

### OpenTelemetry

- `GET /api/otel/status`
- `GET /api/otel/resources`
- `GET /api/otel/stats`
- `GET /api/otel/pipeline`
- `POST /api/otel/api-keys`
- `GET /api/otel/api-keys`
- `POST /v1/traces`
- `POST /v1/logs`
- `POST /v1/metrics`

### Binary distribution

- `GET /dl/zentra-agent-linux-x86_64`
- `GET /dl/zentra-collector-linux-x86_64`

## Standard Operating Flow

### 1. Admin login

Use:

- `admin@Zentra.local`

The login response returns:

- JWT token
- refresh token
- role context
- `must_change_password` flag

### 2. Create enrollment key

Use:

- `POST /api/org-enrollment-keys`

This key is used for initial remote registration by `zentra-agent` or `zentra-collector`.

### 3. Register client machine

For Linux client registration, the intended product path is:

- download `zentra-agent` from `/dl/zentra-agent-linux-x86_64`
- download `zentra-collector` from `/dl/zentra-collector-linux-x86_64`
- provide `ZENTRA_API_ENDPOINT`
- provide enrollment key
- provide datacenter name

### 4. Wait for discovery

Use:

- `GET /api/agents`
- `GET /api/agents/by-dc/:dc_name/components`

Expected outputs:

- Linux host component
- log directory components
- middleware / database / network components if present

### 5. Confirm discovered components

Use:

- `POST /api/agents/components/confirm`

This promotes discovered components into monitoring targets.

### 6. Validate telemetry

Use:

- `GET /api/otel/resources`
- `GET /api/otel/stats`
- `POST /v1/traces`
- `POST /v1/logs`

Expected outputs:

- new service names in resources
- updated telemetry counts

## Live Validation Performed On 2026-07-12

Validated machines:

- server `172.19.155.52`
- client `172.19.156.110`

Validated flows:

1. built frontend and Rust binaries on the Linux server
2. verified live download endpoints for `zentra-agent` and `zentra-collector`
3. created fresh org enrollment and OTLP API keys through the API
4. validated live agent registration on the Linux client
5. repaired and re-registered the client collector onto `zentra-collector`
6. installed nginx, Apache, and HAProxy on the client
7. generated traffic through all three
8. confirmed discovered middleware through the API
9. posted OTLP traces and logs through the live API
10. validated RBAC by creating Admin, Viewer, and Operator users through the API

## RBAC Model Observed Live

### Super Admin

Observed behavior:

- can create users
- must assign `Admin` when creating a user
- can manage platform-wide settings and organizations

### Admin

Observed behavior:

- can create lower-privilege users
- cannot assign `Admin` or `Super Admin`
- receives full module access in `/api/auth/me`

### Viewer

Observed behavior:

- login succeeds
- read-oriented permission set is returned from `/api/auth/me`
- cannot create users
- cannot manage enrollment keys
- cannot manage agent fleet

### Operator

Observed behavior:

- login succeeds
- broader read/write monitoring permissions returned from `/api/auth/me`
- cannot manage agent fleet operations guarded as admin-only

## Important Findings From Live Testing

### 1. Linux product flow should use Zentra collector, not upstream Linux collector

The Linux wizard path was still partially using upstream `otelcol-contrib`.

Status:

- corrected for Linux collector and Linux network snippets in `AgentInstallWizard.tsx`
- frontend rebuilt on server

Remaining gap:

- Windows collector snippets still reference upstream `otelcol-contrib`

### 2. Collector stale-key recovery is fragile

Observed on client:

- `zentra-collector.service` entered a 401 restart loop because stale key material was present

Operational recovery:

- remove stale collector key file
- disable old `otelcol-contrib.service`
- restart `zentra-collector` with fresh registration key

### 3. Pipeline API did not show active collector flow

Even after live collector registration and successful telemetry ingest:

- `GET /api/otel/pipeline` returned empty collector and flow lists

This is a product gap between backend state and pipeline visualization.

### 4. Middleware port attribution is inaccurate

Observed on client:

- Apache ran on `8080`
- nginx ran on `8081`
- HAProxy ran on `8088`

But discovered components were promoted using default port `80`.

This likely comes from process-name fallback logic overriding actual listener ports.

### 5. RBAC UI/module projection is likely incomplete for non-admin roles

Observed:

- `Viewer` and `Operator` returned valid permission lists in `/api/auth/me`
- both returned `module_access: []`

Impact:

- backend permission model exists
- UI/menu/module projection for non-admin users may be empty or inconsistent

## Known Good Commands and API Procedures

### Create org enrollment key

Request:

- `POST /api/org-enrollment-keys`

Use as bearer-authenticated admin request body with at least:

- `name`

### Create OTLP API key

Request:

- `POST /api/otel/api-keys`

Useful body:

- `name`
- `permissions`
- `expires_in_days`

### Create user

Request:

- `POST /api/users`

Body fields used in live test:

- `email`
- `username`
- `display_name`
- `password`
- `role_ids`
- `org_id` when caller is `Super Admin`

## Troubleshooting

### Symptom: collector restarts with 401 unauthorized

Likely cause:

- stale or revoked key material on client

Check:

- `/zentra/etc/collector.key`
- `/zentra/etc/zentra-collector.env`
- `journalctl -u zentra-collector -n 50 --no-pager`

Recovery:

1. stop collector
2. remove stale key files
3. use fresh org enrollment key
4. restart collector

### Symptom: middleware is discovered on wrong port

Likely cause:

- process-name heuristic mapping to default ports

Check:

- actual listeners with `ss -ltnp`
- discovered component payload from `/api/agents/by-dc/:dc_name/components`

### Symptom: user can log in but sees no product surface

Likely cause:

- role permissions exist but `module_access` rows are absent for non-admin path

Check:

- `GET /api/auth/me`
- compare `permissions` versus `module_access`

## Recommended Follow-up Work

1. finish replacing remaining Windows wizard references to upstream collector
2. fix pipeline API so it reflects active collector topology
3. fix middleware discovery to prefer real bound ports
4. define and populate `user_module_access` behavior for Viewer and Operator roles
5. add automated regression tests for:
   - stale collector key recovery
   - admin role assignment restrictions
   - non-admin module projection
