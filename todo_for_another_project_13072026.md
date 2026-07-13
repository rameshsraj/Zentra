# TODO for Another Project - 13 July 2026

## Purpose

This document captures the work completed and lessons learned over the last 10 days, plus sanitized prompts that can be reused in another product without project-specific names or file references.

The main theme from this period was moving from "features exist" to "features behave like a production-grade operations platform":

- API-driven validation instead of manual or script-only checks
- real agent and telemetry flow testing
- user and RBAC validation
- dashboard redesign for scale
- runbook-quality documentation
- product gaps turned into explicit follow-up work

## Date Range

Covered period: 04 July 2026 through 13 July 2026.

## Completed Work Summary

### 1. End-to-end deployment and live flow validation

Completed:

- Built frontend and backend binaries on the Linux server rather than on the workstation.
- Verified application health and served frontend routes after deployment.
- Verified Linux agent and collector binaries were downloadable from the server.
- Created fresh enrollment keys through APIs.
- Created fresh telemetry ingest keys through APIs.
- Registered a Linux client agent against the server.
- Registered a Linux collector against the server.
- Disabled the older upstream collector service path on the client.
- Repaired a stale-key collector failure by clearing old key material and re-registering with a fresh key.
- Confirmed active agent and collector records through APIs.

Outcome:

- Server build and deployment path is working.
- Client agent registration is working.
- Collector registration is working after stale key cleanup.
- Product-owned collector path works for Linux.

### 2. OpenTelemetry validation

Completed:

- Verified telemetry status APIs.
- Created a dedicated ingest API key.
- Posted trace payloads to the trace ingest endpoint.
- Posted log payloads to the log ingest endpoint.
- Confirmed the test service appeared in telemetry resource APIs.
- Confirmed stats endpoints reflected telemetry activity.

Findings:

- Ingest works.
- Resource discovery works.
- Pipeline/topology view did not reflect the active collector path and remains a product gap.

### 3. Middleware installation and traffic simulation

Completed:

- Installed and started nginx, Apache, and HAProxy on a Linux client.
- Configured each service on a separate listener.
- Simulated traffic through all three services.
- Confirmed middleware discovery through APIs.
- Promoted discovered middleware components into monitored targets through APIs.

Findings:

- Middleware discovery works at a basic process/service level.
- Port attribution is inaccurate when middleware runs on non-default ports.
- Dashboard UI needed a more complete chart and selector experience.

### 4. User and RBAC testing

Completed:

- Created a new Admin user through APIs.
- Logged in as the Admin user.
- Created Viewer and Operator users through APIs.
- Verified Admin cannot grant another Admin role.
- Verified Viewer cannot create users, manage enrollment keys, or manage agents.
- Verified Operator cannot perform agent-management actions that require admin privileges.
- Inspected effective role, permission, and module access responses.

Findings:

- Role boundaries are mostly correct for the tested write paths.
- Non-admin users receive meaningful permission sets.
- Non-admin users returned empty module/menu projection, which may make the UI unusable unless module access is explicitly populated or derived.

### 5. Dashboard principle correction

Completed:

- Identified that several dashboards were mixing summary, inventory, and deep detail on one surface.
- Defined a dashboard design standard:
  - dashboards summarize
  - list/detail pages enumerate
  - dashboards use top-N, aggregates, charts, and links
  - dashboards must not render unbounded object cards
- Removed per-server detail cards from the infrastructure dashboard.
- Changed the server summary panel to aggregate fleet values instead of individual server rows.
- Redesigned middleware dashboard with:
  - KPI strip
  - service overview cards
  - service and instance dropdowns
  - selected-service telemetry panel
  - chart cards and empty states
- Changed the database dashboard to show database-type summaries and health distribution rather than listing every database instance.

Outcome:

- Dashboard surfaces now better support large fleets.
- The product rule is documented for future screens.

### 6. Documentation created or updated

Completed:

- Created a full operations runbook.
- Created a live validation report for the API, telemetry, agent, collector, middleware, and RBAC test pass.
- Created dashboard design principles.
- Created this sanitized todo handoff document.

## Current Verified State

Verified:

- TypeScript checks pass locally.
- Production frontend builds pass on the Linux server.
- Changed dashboard routes return HTTP 200.
- API login works with provided admin credentials.
- API-created keys work for enrollment and telemetry ingest.
- Client-side agent and collector can register successfully.
- Middleware services can be discovered and confirmed.
- Telemetry traces/logs can be ingested and queried via resource APIs.
- RBAC creation and denial cases were tested through APIs.

## Known Product Gaps

### P1 - Pipeline topology does not show active collector flow

Telemetry ingest and collector registration work, but the pipeline/topology endpoint did not show the active collector/flow.

Expected:

- Active collectors should appear in the pipeline view.
- Ingest paths should be visible in topology.
- The dashboard should explain when data exists but no pipeline graph is available.

### P1 - Middleware port detection is inaccurate

Middleware services running on non-default ports were discovered/promoted with default ports.

Expected:

- Prefer actual bound listener ports over process-name defaults.
- Preserve the service's runtime host and port in promoted targets.
- Avoid creating monitoring targets on incorrect default ports.

### P1 - Non-admin module projection is empty

Viewer and Operator users returned valid permissions but empty module/menu access arrays.

Expected:

- Non-admin users should receive module access derived from their permissions or seeded explicitly.
- UI should not become blank for valid non-admin users.

### P1 - Remaining upstream collector references

The Linux flow was shifted to the product-owned collector path, but some platform-specific install snippets still reference upstream collector binaries.

Expected:

- All supported platforms should use product-owned collector artifacts where available.
- If a platform is unsupported, the UI should state that clearly.

### P2 - Dashboard consistency needs a broader audit

The biggest dashboard violations were addressed, but other operational dashboards should still be reviewed against the new standard.

Expected:

- No dashboard should render unbounded object cards.
- List/detail views should own tables, search, filters, and pagination.

## TODO Backlog

### 1. Fix collector pipeline visibility

Tasks:

- Trace how active collectors are stored after registration.
- Connect collector state to the pipeline/topology API.
- Include collector status, signal flow, receiver/exporter edges, and last-seen timestamps.
- Add an empty state explaining missing pipeline data.
- Test with a live collector and ingested trace/log payloads.

Acceptance criteria:

- Active collector appears in the pipeline API response.
- Pipeline dashboard shows a meaningful graph or structured flow.
- Telemetry resource data and pipeline data agree on recent activity.

### 2. Correct middleware port attribution

Tasks:

- Review component discovery logic for process-name defaults.
- Prefer listening socket data where available.
- Preserve non-default service ports during component confirmation.
- Add tests for nginx, Apache, and HAProxy on non-default ports.

Acceptance criteria:

- A service running on a non-default port is discovered with that port.
- Confirmed target uses the actual port.
- No duplicate default-port target is created when a real listener exists.

### 3. Fix non-admin module/menu access

Tasks:

- Decide whether module access is explicit, permission-derived, or both.
- Seed or compute module access for Viewer and Operator roles.
- Add API tests for Admin, Viewer, and Operator.
- Add UI smoke tests to ensure non-admin users see their permitted surfaces.

Acceptance criteria:

- Viewer and Operator login returns usable module/menu data.
- Admin-only actions remain blocked for non-admin roles.
- Read-only roles cannot mutate protected resources.

### 4. Finish collector install parity across platforms

Tasks:

- Inventory all install wizard paths.
- Replace upstream collector references with product-owned artifacts where supported.
- Add clear unsupported-platform messaging where artifacts do not exist.
- Verify generated install commands use the same enrollment and key handling model.

Acceptance criteria:

- Linux install path uses product-owned agent and collector binaries.
- Other platform paths either use product-owned binaries or clearly state limitations.
- Install commands are copy-paste runnable.

### 5. Continue dashboard standardization

Tasks:

- Audit every page with "Dashboard" in the title.
- Remove unbounded lists/cards from dashboard surfaces.
- Replace object grids with aggregates, top-N sections, charts, and links.
- Keep tables and dense object views in inventory/detail pages.

Acceptance criteria:

- A dashboard remains readable with 100+ monitored objects.
- Every dashboard has stable summary cards and a clear drilldown path.
- No dashboard renders an unpaginated inventory table unless it is explicitly a list page.

### 6. Add automated end-to-end validation

Tasks:

- Automate API login, key creation, agent registration, telemetry ingestion, RBAC checks, and route checks.
- Keep the test API-only where possible.
- Produce a structured report artifact after each run.

Acceptance criteria:

- One command can validate the critical product flow in a test environment.
- Failures show endpoint, expected result, actual result, and remediation hints.

## Sanitized Prompts for Reuse

The following prompts intentionally avoid project names and specific filenames.

### Prompt 1 - Full API-driven validation

Run the full product flow through APIs only. Log in with the provided admin credentials, create fresh enrollment and telemetry keys, deploy the agent and collector to the client machine, verify registration, send telemetry, and confirm the results through API responses. If any step fails, fix the cause, rebuild on the Linux server, redeploy, and rerun the validation.

### Prompt 2 - Collector path correction

Review the agent installation wizard and ensure the Linux collector path uses the product-owned collector binary rather than downloading upstream collector binaries. Verify the generated commands use the server download endpoint, enrollment key, API endpoint, and datacenter name consistently. Build the UI bundle on the Linux server after the change.

### Prompt 3 - Middleware live test

Install nginx, Apache, and HAProxy on the client machine, configure them to run at the same time, generate traffic through all services, and validate discovery through APIs. Promote discovered middleware components to monitored targets and verify the dashboard reflects the result.

### Prompt 4 - OpenTelemetry ingest test

Create a telemetry API key through the API and post representative trace and log payloads to the ingest endpoints. Verify that telemetry resources and stats APIs show the new service and signals. Check whether the pipeline/topology view reflects the active flow and document any mismatch.

### Prompt 5 - User and role validation

Create test users through the API for admin, read-only, and operator-style roles. Log in as each user and verify both allowed and denied actions. Confirm that elevated roles cannot be granted by non-super-admin users and that non-admin users receive usable module/menu access.

### Prompt 6 - Dashboard scalability audit

Review every dashboard screen and remove unbounded object lists or card grids. A dashboard should summarize health, trends, distributions, and top-N issues. Detailed object enumeration must live in list or detail pages with search, filters, and pagination.

### Prompt 7 - Middleware dashboard redesign

Redesign the middleware dashboard so it matches the richer dashboard style used elsewhere. Keep a fixed KPI strip, service overview cards, dropdown selectors for service and instance, and a selected-service telemetry panel with charts and clear empty states. Do not render one widget per monitored object.

### Prompt 8 - Infrastructure dashboard scale fix

Change the infrastructure dashboard so it remains useful for large fleets. Remove per-server detail cards from the dashboard, show aggregate fleet health and utilization, and link to the server inventory page for the full list.

### Prompt 9 - Database dashboard scale fix

Change the database dashboard so it summarizes database types and health distribution instead of listing every database instance. Keep instance-level tables and details in dedicated database pages.

### Prompt 10 - Runbook documentation

Prepare an operations runbook covering product purpose, architecture, core APIs, standard operating flows, troubleshooting, RBAC behavior, telemetry flow, and known product gaps. Include commands and validation procedures only where they are reusable and safe.

### Prompt 11 - Dashboard principle documentation

Write a dashboard design standard for the product. Define what belongs on dashboards, what belongs on list/detail pages, how to handle large fleets, when to use top-N sections, and how dashboard cards should link into detailed workflows.

### Prompt 12 - Live validation report

After completing the end-to-end test, document what was validated, what passed, what failed, what was fixed, what remains incomplete, and which APIs were used. Keep the report practical enough for another engineer to repeat the test.

## Recommended Next Sequence

1. Fix pipeline/topology visibility.
2. Fix middleware non-default port attribution.
3. Fix non-admin module/menu projection.
4. Finish platform parity for collector install flows.
5. Continue dashboard standardization across remaining dashboard screens.
6. Convert the manual validation sequence into an automated regression workflow.

## Notes for the Next Project

- Do not treat a successful build as proof of product quality.
- Always validate with live API calls and role-specific users.
- Always test at scale mentally: 1 object, 10 objects, 100 objects.
- Dashboards should stay stable as object counts grow.
- Installation flows should use product-owned artifacts whenever possible.
- If UI data is empty, determine whether it is a product issue, API issue, or expected empty state.

