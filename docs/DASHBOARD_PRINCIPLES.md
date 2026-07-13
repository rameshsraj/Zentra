# Dashboard Design Principles

Dashboards in Zentra should summarize operational posture. They should not become inventory pages.

## Core Rule

A dashboard must answer:

- How healthy is the system?
- What changed?
- What needs attention?
- Where should the operator go next?

A dashboard should not enumerate every monitored object when the count can grow without bound.

## Scalable Dashboard Pattern

Use dashboards for:

- KPI strips
- aggregate health counts
- trend charts
- distribution charts
- top-N problem areas
- small drilldown links to list/detail pages

Use list/detail pages for:

- every server
- every database
- every storage array
- every trace
- every alert
- every connection
- every collector or agent

## UI Rules

1. If a page can show 100+ cards, it is not a dashboard.
2. Show top-N only when the purpose is ranking or triage.
3. Use dropdown selectors for switching monitored type, instance, region, or time window.
4. Keep summary cards fixed in count and stable in layout.
5. Detail pages can use tables, filters, pagination, and dense views.
6. Dashboard cards should link into the relevant list/detail surface.

## Current Application Mapping

- Command Center: executive dashboard.
- Infrastructure Dashboard: aggregate server/storage/network posture.
- Middleware Dashboard: service-type selector plus selected service charts.
- Database Dashboard: database-type summaries and health distribution.
- Servers: detailed server inventory.
- Fleet: detailed agent and component inventory.
- Network: detailed network operations and connection analysis.
- Storage pages: storage inventory and storage detail workflows.
