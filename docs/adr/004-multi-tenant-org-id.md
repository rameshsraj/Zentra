# ADR 004: Multi-Tenant Isolation via org_id

## Status: Accepted (enhanced with RLS in Phase 5)
## Date: 2025-01-01

## Context
Zentra must support multiple organizations in a single database instance.

## Decision
Use `org_id UUID` column on all tenant-scoped tables. Extract from JWT claims in middleware.

## Consequences
- **Positive:** Simple implementation, no schema-per-tenant complexity
- **Positive:** Supports shared infrastructure reporting
- **Negative:** Relies on application code to always add WHERE org_id = $1
- **Negative:** Cross-tenant query possible if a handler forgets the filter
- **Mitigation:** PostgreSQL Row-Level Security added in Phase 5 as defense-in-depth
