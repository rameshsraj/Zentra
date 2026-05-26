# Why OpenTelemetry in InfraScope

> **Audience:** InfraScope team, stakeholders, and customers  
> **Last Updated:** May 2026

---

## The Problem: Two Worlds of Monitoring

InfraScope already monitors **infrastructure** brilliantly:

| What InfraScope Monitors Today | How |
|-------------------------------|-----|
| Servers (CPU, RAM, disk, processes) | Agent-based collection |
| Databases (Oracle, Postgres, MySQL, MongoDB, SQL Server) | Direct DB connections, query metrics |
| Middleware (Nginx, Apache, HAProxy, Tomcat, Kafka, Redis, etc.) | Service-specific scrapers |
| Network (topology, NetFlow, eBPF, DNS, HTTP analytics) | Packet capture, flow analysis |
| Security (IDS/IPS, firewall, SSL/SSH, threat hunting) | Log parsing, rule engines |
| Logs (explorer, sources, Grok metrics, alerts) | Syslog/file/API collection |
| Storage (arrays, volumes, IOPS) | SNMP, API polling |

**But there's a blind spot: the application itself.**

InfraScope can tell you the database server is slow, but it can't tell you *which API call* made the slow query, *which user request* triggered it, or *how the latency propagated* across 5 microservices.

That's the gap OpenTelemetry fills.

---

## What OpenTelemetry Adds (That We Don't Have Today)

### The Simple Answer

```
BEFORE OpenTelemetry:
  "The Postgres server CPU spiked at 2:15 PM"

AFTER OpenTelemetry:
  "The /api/orders endpoint in the order-service made a slow JOIN query
   that took 3.2s, caused by a missing index on orders.customer_id,
   triggered by user checkout flow, affecting 847 requests with 12% error rate"
```

### The Four Signals

| Signal | What It Captures | InfraScope Without OTel | InfraScope With OTel |
|--------|-----------------|------------------------|---------------------|
| **Traces** | Request flow across services | We see server metrics; no request-level visibility | Full distributed trace: Service A → Service B → DB → Cache, with timing for every hop |
| **Metrics** | Application-level counters/gauges | We scrape infra metrics (CPU, disk, connections) | App-specific: request rate, error rate, queue depth, business metrics |
| **Logs** | Structured application logs | We collect syslog/files — text blobs | Structured JSON logs correlated with trace_id — click a log → see the full request trace |
| **Profiles** | CPU/memory usage by code function | We see process-level CPU %; no code detail | Flamegraph: "function X in class Y consumed 40% CPU during this trace" |

---

## How It Complements (Not Replaces) Existing Monitoring

```
┌─────────────────────────────────────────────────────────────────┐
│                    InfraScope Monitoring Stack                    │
│                                                                  │
│  LAYER 4: APPLICATION CODE (NEW — OpenTelemetry)                │
│  ├── Distributed traces across microservices                     │
│  ├── Per-endpoint RED metrics (Rate, Error, Duration)            │
│  ├── Structured logs with trace correlation                      │
│  ├── CPU/memory profiles linked to specific requests             │
│  └── LLM/GenAI call monitoring (tokens, latency, errors)        │
│                                                                  │
│  LAYER 3: MIDDLEWARE & SERVICES (Existing)                       │
│  ├── Nginx, HAProxy, Tomcat, Kafka, Redis, etc.                 │
│  ├── Request counts, connection pools, queue depths              │
│  └── Per-service health dashboards                               │
│                                                                  │
│  LAYER 2: DATABASE (Existing)                                    │
│  ├── Oracle, Postgres, MySQL, MongoDB, SQL Server                │
│  ├── Query performance, tablespace, replication lag              │
│  └── Slow query analysis, lock monitoring                        │
│                                                                  │
│  LAYER 1: INFRASTRUCTURE (Existing)                              │
│  ├── Servers: CPU, RAM, disk, processes                          │
│  ├── Network: topology, flows, eBPF, DNS                        │
│  ├── Storage: arrays, volumes, IOPS                              │
│  └── Security: IDS, firewall, threat intelligence                │
└─────────────────────────────────────────────────────────────────┘
```

**OpenTelemetry is Layer 4** — the application layer that sits on top of everything InfraScope already does. It doesn't replace Layers 1-3; it **connects them to actual user requests**.

---

## Concrete Use Cases

### Use Case 1: "The App Is Slow"

**Without OTel:**
- InfraScope shows: server CPU is 40%, DB connections normal, Nginx response times elevated
- Team conclusion: "Something is slow but we don't know what"
- Action: SSH into servers, grep logs, try to reproduce

**With OTel:**
- InfraScope shows: `order-service` → `inventory-service` → Postgres: the `check_stock()` query takes 2.8s
- Span Metrics (RED): `/api/checkout` p95 latency jumped from 200ms to 3.1s at 2:10 PM
- Trace waterfall: Request → order-service (50ms) → inventory-service (2.8s) → Postgres (2.7s)
- Action: Fix the slow query, verify fix in real-time on the RED dashboard

### Use Case 2: "Error Rate Spike"

**Without OTel:**
- InfraScope alerts: Nginx 5xx errors increased
- Team: Checks each backend service manually — which one is failing?

**With OTel:**
- Signal Alert fires: `payment-service` error_rate > 5% in the last 5 minutes
- Traces show: 80% of errors are `TimeoutException` on calls to external payment gateway
- GenAI page shows: LLM-powered fraud detection calls are also timing out (same gateway)
- Action: Payment gateway is down → enable fallback, notify vendor

### Use Case 3: "New Deployment Broke Something"

**Without OTel:**
- InfraScope: Server metrics look fine after deploy
- Users report: "Checkout is broken"
- Team: Rolls back blindly

**With OTel:**
- Discovered Services: `cart-service` v2.1.0 appeared after deploy (auto-detected from `service.version`)
- RED metrics: `cart-service` error rate jumped from 0.2% to 18% immediately after deploy
- Traces: New version is calling a deprecated API endpoint that returns 404
- Action: Targeted rollback of `cart-service` only, not entire platform

### Use Case 4: "Which Services Talk to Each Other?"

**Without OTel:**
- InfraScope: Shows network connections between IPs
- Team: "IP 10.0.5.23 talks to 10.0.5.47" — but what services are those?

**With OTel:**
- Pipeline Topology: Visual map showing `frontend` → `api-gateway` → `user-service` → Postgres
- Each node shows: span count, last seen, health status
- Discovered Resources: 12 services across 3 environments, using Java/Python/Go SDKs

### Use Case 5: "LLM Costs Are Exploding"

**Without OTel:**
- InfraScope: Can't see LLM API calls — they're just HTTPS requests
- Team: Checks cloud billing dashboard monthly

**With OTel:**
- GenAI Observability: 45,000 LLM calls this week, consuming 12M tokens
- Model breakdown: `gpt-4` averaging 1,200 tokens/call, `gpt-3.5-turbo` averaging 400
- One service making 10x more calls than expected (runaway retry loop)
- Action: Fix retry loop, switch low-priority calls to cheaper model → 60% cost reduction

---

## Why OpenTelemetry Specifically (vs. Building Our Own)

| Approach | Downside |
|----------|----------|
| **Custom InfraScope agent in every app** | Requires code changes per language, maintenance burden, customers resist proprietary agents |
| **APM vendor SDK (Datadog, New Relic)** | Vendor lock-in, expensive per-host licensing, customers already have OTel |
| **OpenTelemetry (what we chose)** | Open standard, CNCF-graduated, zero vendor lock-in, customers may already be using it |

### Why OTel Wins for InfraScope:

1. **Zero friction adoption** — customers add 3 env vars, no code changes (Java auto-agent, Python auto-instrument)
2. **We become the backend** — customers already running OTel can switch to InfraScope by changing one URL
3. **Steal from Datadog/New Relic** — "You're already using OTel? Just point it at InfraScope instead"
4. **Enterprise trust** — CNCF graduated project, same standard used by Google, Microsoft, AWS
5. **No agent to maintain** — OTel SDKs are maintained by the community, not us
6. **Future-proof** — Profiles (4th signal) puts us ahead of most APM vendors

---

## What Each OTel Page Gives the User

| Page | Value to the User |
|------|------------------|
| **Telemetry Health** | "Is my OTel pipeline working? How much data is flowing?" |
| **Traces** | "Show me exactly what happened during this request, across all services" |
| **Discovered Services** | "What applications are instrumented? Which SDKs are they using?" |
| **Collector Setup** | "How do I point my apps at InfraScope?" (answer: just 3 env vars) |
| **SDK Guide** | "How do I instrument my Java/Python/Go/Node app?" (copy-paste instructions) |
| **Pipeline Topology** | "What's the shape of my application architecture?" |
| **Span Metrics (RED)** | "What's my error rate, latency, and throughput — per service, per endpoint?" |
| **Signal Alerts** | "Alert me when error rate exceeds 5% or p99 latency exceeds 2 seconds" |
| **GenAI Observability** | "How many LLM calls are we making? Which models? How many tokens? What's the cost?" |
| **OTLP Security** | "Control who can send data: API keys, rate limits, audit trail" |

---

## The Complete Picture: InfraScope + OpenTelemetry

```
Customer's Environment
━━━━━━━━━━━━━━━━━━━━━

  [Java App]  [Python App]  [Go Service]  [Node.js API]
       │            │             │              │
       └────────────┴─────────────┴──────────────┘
                         │
                    OTel SDK (auto)
                    3 env vars, zero code changes
                         │
                    ┌────▼─────┐
                    │InfraScope│  ← SINGLE PANE OF GLASS
                    │          │
                    │ Layer 4: Application traces, RED metrics, logs, profiles, GenAI
                    │ Layer 3: Middleware (Nginx, Kafka, Redis, Tomcat, ...)
                    │ Layer 2: Databases (Oracle, Postgres, MySQL, MongoDB, ...)
                    │ Layer 1: Infrastructure (servers, network, storage, security)
                    │          │
                    │ AI & ML: Predictive alerts, RCA, capacity planning
                    │ Ops:     DORA, runbooks, deployments, SLA reports
                    └──────────┘
```

**Without OTel:** InfraScope monitors infrastructure (Layers 1-3) — where the problem lives.  
**With OTel:** InfraScope also monitors applications (Layer 4) — where the problem starts.  
**Together:** Full-stack observability from user click to database query, in one tool.

---

## Summary: One-Line Answer

> **OpenTelemetry gives InfraScope application-level visibility — the ability to trace a user request across every service, database, and queue it touches — without requiring customers to install a proprietary agent or change a single line of code.**

This is the missing layer between "the server is slow" and "this specific API call in this specific service is making this specific slow query."

Every other monitoring feature InfraScope has (servers, databases, middleware, network, security, logs) tells you **where** the problem is. OpenTelemetry tells you **why**.
