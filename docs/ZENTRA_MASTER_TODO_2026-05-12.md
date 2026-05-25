# Zentra — Comprehensive TODO List
## Date: 2026-05-12
## Project: Zentra (Enterprise Monitoring & Operations Platform)

---

## PHASE 1: CRITICAL SECURITY FIXES (Week 1)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 1 | Fix SQL/command injection in Oracle SSH queries — use stdin pipe instead of format!() | crates/zentra-api/src/handlers/oracle.rs | Critical | Medium |
| 2 | Fix SSH password injection in expect script — use env var exclusively | crates/zentra-api/src/ssh_utils.rs | Critical | Low |
| 3 | Block SSRF via arbitrary SSH host — validate against RFC1918, 127.0.0.0/8, 169.254.169.254 | crates/zentra-api/src/handlers/targets.rs | Critical | Low |
| 4 | Remove all hardcoded credentials from deploy scripts | deploy/*.sh, deploy/k8s/*.yaml | Critical | Low |
| 5 | Fail-hard when ZENTRA_MASTER_KEY not set in production | crates/zentra-core/src/crypto.rs | Critical | Low |
| 6 | Force password change on default admin account | crates/zentra-api/src/handlers/auth.rs, migrations | Critical | Medium |
| 7 | Rotate all exposed credentials (DB, Oracle SYS, JWT, admin) | Operations | Critical | Low |
| 8 | Implement JWT token revocation — check jti against user_sessions | crates/zentra-api/src/handlers/auth.rs | High | Low |
| 9 | Add password strength requirements (min 12 chars, mixed) | crates/zentra-api/src/handlers/auth.rs | High | Low |

---

## PHASE 2: INPUT VALIDATION & ERROR HANDLING (Week 2)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 10 | Add validator crate with derive macros on all request structs | crates/zentra-api/Cargo.toml, handlers | High | Medium |
| 11 | Validate SSH target hosts — whitelist CIDRs, block private IPs | crates/zentra-api/src/handlers/targets.rs | High | Low |
| 12 | Add ReDoS protection for user-supplied regex | crates/zentra-log-engine/src/ | Medium | Low |
| 13 | Add query time-range limits (max 90 days, min 60s bucket) | crates/zentra-db/src/ | Medium | Low |
| 14 | Validate file paths in log engine — whitelist directories | crates/zentra-log-engine/src/ | Medium | Low |
| 15 | Standardize all handlers to use ApiError enum | crates/zentra-api/src/handlers/ | Medium | Medium |
| 16 | Stop exposing raw sqlx errors to API clients | All handlers | Medium | Low |
| 17 | Add frontend error boundaries per feature area | Frontend pages | Medium | Medium |
| 18 | Add error/loading/empty state handling to all pages | Frontend pages | Medium | Medium |
| 19 | Fix silent .catch(() => {}) swallowing — add toast notifications | Frontend API layer | High | Low |

---

## PHASE 3: TESTING & CI/CD (Weeks 3-4)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 20 | Create GitHub Actions CI pipeline (fmt, clippy, test, build) | .github/workflows/ci.yml | High | Medium |
| 21 | Add frontend CI steps (typecheck, build, npm audit) | .github/workflows/ci.yml | High | Low |
| 22 | Add cargo-audit for dependency vulnerability scanning | CI pipeline | High | Low |
| 23 | Add pre-commit hooks (format, lint, test) | .pre-commit-config.yaml | Medium | Low |
| 24 | Write auth integration tests (login, refresh, MFA, lockout) | crates/zentra-api/tests/ | High | High |
| 25 | Write multi-tenant isolation tests | crates/zentra-api/tests/ | High | High |
| 26 | Write SQL injection test suite for Oracle SSH path | crates/zentra-api/tests/ | High | Medium |
| 27 | Write SSRF validation tests | crates/zentra-api/tests/ | High | Low |
| 28 | Write NetFlow parser tests (malformed packets) | crates/zentra-netflow/src/ | Medium | Medium |
| 29 | Write SNMP BER decoder tests | crates/zentra-ingestion/src/ | Medium | Medium |
| 30 | Write PromQL parser edge-case tests | crates/zentra-query/src/ | Medium | Medium |
| 31 | Add Vitest + React Testing Library to frontend | frontend/package.json | Medium | Low |
| 32 | Write frontend auth flow component tests | frontend/src/__tests__/ | Medium | Medium |
| 33 | Write frontend API client tests | frontend/src/lib/api/__tests__/ | Medium | Medium |

---

## PHASE 4: MEMORY SAFETY & RELIABILITY (Weeks 5-6)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 34 | Add eviction/TTL to AnomalyDetector::events Vec | crates/zentra-predict/src/ | High | Low |
| 35 | Add eviction to BaselineEngine history HashMap | crates/zentra-predict/src/ | High | Low |
| 36 | Bound NetFlow aggregation HashMap (100K LRU) | crates/zentra-netflow/src/ | High | Medium |
| 37 | Add eviction to seasonal baselines in zentra-ml | crates/zentra-ml/src/ | High | Low |
| 38 | Bound webhook event log retention in zentra-stream | crates/zentra-stream/src/ | Medium | Low |
| 39 | Bound Prometheus scrape health map | crates/zentra-scrape/src/ | Medium | Low |
| 40 | Rate-limit SNMP trap task spawning (max 1000 concurrent) | crates/zentra-ingestion/src/ | Medium | Low |
| 41 | Fix .unwrap() on JSON in provisioning.rs | crates/zentra-api/src/provisioning.rs | Medium | Low |
| 42 | Fix PostgreSQL CSV parser unwrap on truncated lines | crates/zentra-log-engine/src/ | Medium | Low |
| 43 | Fix custom regex parser unwrap on Regex::new() | crates/zentra-log-engine/src/ | Medium | Low |
| 44 | Fix NetFlow v5 record offset bounds checking | crates/zentra-netflow/src/ | Medium | Low |

---

## PHASE 5: DATABASE & MIGRATION FIXES (Week 7)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 45 | Resolve migration 006/007 conflict (dashboard_variables) | migrations/ | High | Medium |
| 46 | Resolve migration 001/007 conflict (dashboards table) | migrations/ | High | Medium |
| 47 | Resolve migration 010/012 dependency (user_roles) | migrations/ | High | Medium |
| 48 | Consolidate migration 012/015 (auth fix) | migrations/ | Medium | Medium |
| 49 | Add DOWN migrations for all 22 schemas | migrations/ | Medium | High |
| 50 | Add PostgreSQL Row-Level Security (RLS) for org_id | migrations/ | Medium | Medium |
| 51 | Add migration verification script | deploy/verify-migrations.sh | Medium | Low |
| 52 | Add database backup script (pg_dump + WAL) | deploy/backup-db.sh | High | Medium |
| 53 | Document point-in-time recovery procedure | Documentation | High | Low |

---

## PHASE 6: PERFORMANCE OPTIMIZATION (Weeks 8-9)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 54 | Increase React Query staleTime to 30s | frontend/src/main.tsx | High | Low |
| 55 | Implement WebSocket push for real-time dashboards | Backend + Frontend | High | High |
| 56 | Add request debouncing for search/filter inputs (300ms) | Frontend pages | Medium | Low |
| 57 | Add React.memo to chart components | frontend/src/components/charts/ | Medium | Low |
| 58 | Add API pagination on all list endpoints (?page=&limit=) | Backend handlers + Frontend | Medium | Medium |
| 59 | Add feature-based code splitting | frontend/src/App.tsx | Low | Medium |
| 60 | Parallelize Prometheus scraper (futures::join_all) | crates/zentra-scrape/src/ | High | Low |
| 61 | Cache compiled regex patterns in log engine | crates/zentra-log-engine/src/ | Medium | Low |
| 62 | Batch SNMP trap DB inserts (1s accumulation) | crates/zentra-ingestion/src/ | Medium | Medium |
| 63 | Add K-means convergence check (early exit) | crates/zentra-ml/src/ | Low | Low |
| 64 | Reuse buffer in NetFlow listeners | crates/zentra-netflow/src/ | Low | Low |
| 65 | Deduplicate NetFlow listener code (generic function) | crates/zentra-netflow/src/ | Low | Medium |

---

## PHASE 7: DEPLOYMENT HARDENING (Weeks 10-11)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 66 | Enable SSH StrictHostKeyChecking in deploy scripts | All deploy/*.sh | Medium | Low |
| 67 | Run services as non-root user (zentra service account) | deploy/setup_server.sh | Medium | Low |
| 68 | Fix K8s image tags — pin to semver, not :latest | deploy/k8s/zentra-deployment.yaml | Medium | Low |
| 69 | Use external secret manager for K8s | K8s manifests | Medium | Medium |
| 70 | Add Prometheus endpoint auth | deploy/nginx.conf | Medium | Low |
| 71 | Fix CSP header — remove unsafe-inline and unsafe-eval | deploy/nginx.conf | Medium | Medium |
| 72 | Tighten login rate limit (5r/m → 3r/m) | deploy/nginx.conf | Low | Low |
| 73 | Add rollback capability to deploy.sh | deploy/deploy.sh | High | Medium |
| 74 | Add database snapshot before migration | deploy/deploy.sh | High | Low |
| 75 | Improve health check (10s wait, validate response body) | deploy/deploy.sh | Medium | Low |
| 76 | Standardize ports — single source of truth | All deploy scripts | Medium | Low |
| 77 | Add post-deploy smoke test | deploy/smoke-test.sh | Medium | Low |
| 78 | Increase K8s resource limits | deploy/k8s/zentra-deployment.yaml | Medium | Low |

---

## PHASE 8: UX & ACCESSIBILITY (Week 12)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 79 | Build Health Check Matrix page | Frontend (new page) | Medium | Medium |
| 80 | Build Maintenance Windows page | Frontend (new page) | Medium | Medium |
| 81 | Build SLA Reports dashboard | Frontend (new page) | Medium | Medium |
| 82 | Build IP Blacklist management UI | Frontend (new page) | Medium | Low |
| 83 | Build IOC List management UI | Frontend (new page) | Medium | Low |
| 84 | Complete Runbooks page | Frontend (existing page) | Low | Medium |
| 85 | Standardize loading states (use LoadingSkeleton) | All frontend pages | Medium | Medium |
| 86 | Standardize empty states | All frontend pages | Low | Low |
| 87 | Standardize date/number formatting | All frontend pages | Low | Low |
| 88 | Filter Command Palette by permissions | frontend/src/components/CommandPalette.tsx | Low | Low |
| 89 | Add responsive design for large pages | Frontend pages | Low | Medium |
| 90 | Add ARIA labels to navigation, sidebars, data tables | Layout, DataTable components | Medium | Medium |
| 91 | Add accessible chart descriptions | Chart components | Low | Low |
| 92 | Add focus traps to modals | Modal components | Low | Low |
| 93 | Add skip-to-content link | frontend/src/components/Layout.tsx | Low | Low |
| 94 | Add text labels to color-only status indicators | Badge components | Low | Low |

---

## PHASE 9: ARCHITECTURE & LONG-TERM (Quarter 2)

| # | Task | File(s) | Severity | Effort |
|---|------|---------|----------|--------|
| 95 | Define Plugin trait interface | crates/zentra-core/src/ | Medium | High |
| 96 | Replace SSH/sqlplus with rust-oracle driver | crates/zentra-api/src/handlers/oracle.rs, plugins | Medium | High |
| 97 | Add API versioning (/api/v1/) | crates/zentra-api/src/ | Medium | Medium |
| 98 | Implement event sourcing for audit trail | crates/zentra-stream/, crates/zentra-db/ | Medium | High |
| 99 | Add OpenTelemetry distributed tracing | All backend crates | Medium | High |
| 100 | Add read replica routing | crates/zentra-db/src/ | Medium | High |
| 101 | Migrate lazy_static to std::sync::OnceLock | All backend crates | Low | Low |
| 102 | Write architecture decision records (ADRs) | docs/adr/ | Medium | Low |
| 103 | Write incident response runbook | docs/runbooks/ | High | Low |
| 104 | Write database recovery runbook | docs/runbooks/ | High | Low |
| 105 | Write security hardening checklist | docs/security.md | High | Low |
| 106 | Write plugin development guide | docs/plugins.md | Low | Low |
| 107 | Add CHANGELOG | CHANGELOG.md | Low | Low |
| 108 | Remove dead dependency (Zustand) | frontend/package.json | Low | Low |

---

## SUMMARY

| Priority | Count | Scope |
|----------|-------|-------|
| Critical | 9 | Security vulnerabilities, credential exposure |
| High | 30 | Validation, testing, CI/CD, memory safety, backups |
| Medium | 47 | Performance, UX, deployment, architecture |
| Low | 22 | Accessibility, documentation, cleanup |
| **Total** | **108** | |

## ESTIMATED TIMELINE

| Phase | Duration | Items |
|-------|----------|-------|
| Phase 1: Critical Security | Week 1 | #1–9 |
| Phase 2: Validation & Errors | Week 2 | #10–19 |
| Phase 3: Testing & CI/CD | Weeks 3-4 | #20–33 |
| Phase 4: Memory Safety | Weeks 5-6 | #34–44 |
| Phase 5: Database Fixes | Week 7 | #45–53 |
| Phase 6: Performance | Weeks 8-9 | #54–65 |
| Phase 7: Deployment Hardening | Weeks 10-11 | #66–78 |
| Phase 8: UX & Accessibility | Week 12 | #79–94 |
| Phase 9: Architecture | Quarter 2 | #95–108 |
