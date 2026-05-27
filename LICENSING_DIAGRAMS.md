# Zentra Licensing — Diagrams

Companion visuals to [LICENSING_PROMPTS.md](LICENSING_PROMPTS.md) and the plan file at `~/.claude/plans/how-does-the-licensing-synchronous-blum.md`.

All diagrams use Mermaid — render in GitHub, VS Code (with Mermaid extension), Obsidian, or any Mermaid live viewer.

---

## 1. System overview — three subsystems

```mermaid
graph TB
    subgraph "Subsystem A: Runtime Entitlement"
        A1[license.json<br/>signed file]
        A2[license_verifier.rs<br/>HMAC-SHA256]
        A3[license_middleware.rs<br/>Axum layer]
        A4[license_status.rs<br/>GET /api/v1/license-status]
        A1 --> A2
        A2 --> A3
        A2 --> A4
    end

    subgraph "Subsystem B: License Asset Management"
        B1[(licenses table<br/>+ seats + assignments<br/>+ audit_log)]
        B2[handlers/licenses.rs<br/>CRUD REST API]
        B3[license-plugin<br/>daily scheduler]
        B4[Metrics pipeline]
        B1 <--> B2
        B1 --> B3
        B3 --> B4
    end

    subgraph "Subsystem C: DEAD"
        C1[license_policy.rs<br/>908 lines]
        C1 -.never called.-> X[ ]
        style C1 fill:#fdd,stroke:#c00,stroke-dasharray: 5 5
        style X fill:none,stroke:none
    end

    Client[HTTP Client] --> A3
    A3 --> B2
    Frontend[Frontend UI] -.should consume<br/>but doesn't.-> A4

    style A1 fill:#ffe4b5
    style A2 fill:#ffcccc
    style C1 fill:#fdd
```

🔴 red box = security-critical (HMAC scheme).
🟡 dashed-red box = dead code.
Dotted line = intended but not implemented integration.

---

## 2. Request flow — what happens on every HTTP request today

```mermaid
sequenceDiagram
    autonumber
    actor Client
    participant MW as enforce_license_middleware
    participant Chk as check_license_active()
    participant Ver as load_and_verify_license()
    participant Disk as license.json
    participant Handler as Route Handler

    Client->>MW: HTTP request /api/v1/oracle/...
    MW->>Chk: enforcement on?
    alt Dev mode (APP_ENV unset, no prod-license feature)
        Chk-->>MW: false
        MW->>Handler: pass through
        Handler-->>Client: 200 OK
    else Prod mode
        Chk-->>MW: true
        MW->>Ver: verify()
        Ver->>Disk: read file (EVERY request)
        Disk-->>Ver: JSON bytes
        Ver->>Ver: parse, HMAC-SHA256, check expiry
        alt Verification fails
            Ver-->>MW: Err
            MW-->>Client: 403 FORBIDDEN
        else Verification OK
            Ver-->>MW: payload
            MW->>MW: prefix match on path
            alt Path gated & feature off
                MW-->>Client: 403 FORBIDDEN
            else
                MW->>Handler: pass through
                Handler-->>Client: 200 OK
            end
        end
    end
```

**Pain point highlighted:** step 8 — disk read + parse + MAC on every request, no caching.

---

## 3. License file data model (Subsystem A)

```mermaid
classDiagram
    class LicenseFile {
        +LicensePayload payload
        +String signature_hex
    }
    class LicensePayload {
        +String org_id
        +String customer_name
        +DateTime onboarded_at
        +i64 trial_duration_days
        +usize max_targets
        +LicenseFeatures features
    }
    class LicenseFeatures {
        +bool enable_oracle
        +bool enable_ebpf_network
        +bool enable_ai_observability
        +bool enable_otel
        +Vec~String~ allowed_menu_items
    }

    LicenseFile --> LicensePayload
    LicensePayload --> LicenseFeatures

    note for LicenseFile "signature_hex = HMAC-SHA256(\n  EMBEDDED_PUBLIC_KEY_HEX,\n  serde_json::to_vec(payload)\n)\n\n🔴 EMBEDDED_PUBLIC_KEY_HEX is a\nhardcoded sequential constant —\nforgeable."
```

---

## 4. Subsystem B data model — License asset management

```mermaid
erDiagram
    licenses ||--o{ license_seats : "has many"
    licenses ||--o{ license_assignments : "has many"
    licenses ||--o{ license_audit_log : "logged in"
    license_seats ||--o{ license_assignments : "claimed by"

    licenses {
        UUID license_id PK
        UUID org_id
        VARCHAR vendor
        VARCHAR product_name
        INTEGER seat_count
        NUMERIC cost_per_seat
        DATE expiry_date
        VARCHAR status
        VARCHAR license_type "per_seat | per_device | site | enterprise | concurrent | subscription"
        VARCHAR category "software | hardware | cloud | saas | perpetual"
    }
    license_seats {
        UUID seat_id PK
        UUID license_id FK
        VARCHAR assigned_to
        UUID assigned_user_id
        TIMESTAMPTZ checkout_at
        BOOLEAN is_active
    }
    license_assignments {
        UUID assignment_id PK
        UUID license_id FK
        UUID seat_id FK
        UUID target_id
        VARCHAR assignment_type "user | device | service"
        TIMESTAMPTZ assigned_at
    }
    license_audit_log {
        UUID audit_id PK
        UUID license_id FK
        VARCHAR action "created | updated | seat_assigned | renewed | expired"
        VARCHAR actor
        JSONB changes
    }
```

---

## 5. Subsystem B — daily compliance scheduler

```mermaid
sequenceDiagram
    autonumber
    participant Sched as LicensePlugin::run_scheduler
    participant Provider as LicenseProvider
    participant DB as licenses table
    participant Metrics as Metric pipeline

    loop every check_interval_secs (default 86400s)
        Sched->>Provider: fetch_licenses(org_id)
        Provider->>DB: SELECT * FROM licenses WHERE org_id = ?
        DB-->>Provider: Vec~License~
        Provider-->>Sched: licenses
        Sched->>Sched: check_expiry()<br/>band as Ok/Warning/Critical/Expired
        Sched->>Sched: calculate_compliance()<br/>utilisation %, wasted cost
        opt severity == Expired AND status == Active
            Sched->>Provider: mark_expired(ids)
            Provider->>DB: UPDATE licenses SET status='expired'
        end
        Sched->>Metrics: emit ~11 aggregate metrics<br/>+ per-license utilisation
    end
```

---

## 6. Call graph — what calls what (today)

```mermaid
graph LR
    HTTP[HTTP Request] --> MW[enforce_license_middleware]
    MW --> CHK[check_license_active]
    MW --> LV[load_and_verify_license]
    LV --> VS[verify_license_string]
    VS -->|HMAC-SHA256| EK[EMBEDDED_PUBLIC_KEY_HEX]

    STATUS[GET /license-status] --> LV

    PLUGIN[license-plugin scheduler] --> PROV[LicenseProvider]
    PROV --> REPO[repositories::licenses]
    REPO --> PG[(PostgreSQL)]

    CRUD[REST /api/v1/licenses/*] --> REPO

    DEAD[license_policy.rs<br/>ExpiryStrategy, LicenseTier,<br/>MachineFingerprint, LicenseKey]
    DEAD -.NO CALLERS.-> NOWHERE[ ]

    FE[Frontend menu] -.should call but doesn't.-> STATUS

    style EK fill:#ffcccc,stroke:#c00
    style DEAD fill:#fdd,stroke:#c00,stroke-dasharray: 5 5
    style NOWHERE fill:none,stroke:none
```

---

## 7. Current vs. target architecture (after rollout)

```mermaid
graph TB
    subgraph "TODAY"
        T1[HMAC-SHA256<br/>embedded secret] -->|verifies| T2[per-request<br/>disk read]
        T2 --> T3[hardcoded prefix<br/>gate in middleware]
        T3 --> T4[hard 403 on any failure]
        T5[license_policy.rs<br/>UNUSED]
        T6[Frontend UI<br/>doesn't consume status]
        style T1 fill:#ffcccc
        style T2 fill:#ffd699
        style T3 fill:#ffd699
        style T4 fill:#ffd699
        style T5 fill:#fdd
        style T6 fill:#fdd
    end

    subgraph "TARGET (post-2026-07-22)"
        N1[Ed25519 verification<br/>+ versioned envelope<br/>+ key_id rotation] -->|verifies| N2[LicenseCache<br/>TTL + mtime]
        N2 --> N3[RequireFeature layer<br/>per route]
        N3 --> N4[Valid / Grace / Expired<br/>read-only fallback]
        N5[license_policy.rs<br/>= canonical engine]
        N6[Frontend reads<br/>FeatureCatalog]
        N7[Audit log + metrics]
        N8[zentra-license-signer CLI]
        style N1 fill:#c8e6c9
        style N2 fill:#c8e6c9
        style N3 fill:#c8e6c9
        style N4 fill:#c8e6c9
        style N5 fill:#c8e6c9
        style N6 fill:#c8e6c9
        style N7 fill:#c8e6c9
        style N8 fill:#c8e6c9
    end

    TODAY -.via prod-license cfg gate.-> TARGET
```

---

## 8. cfg-gating model (after T01)

```mermaid
flowchart TB
    Build{cargo build}
    Build -->|--no-default-features<br/>DEV| Dev[Licensing code = no-op stubs<br/>get_license_status returns mock<br/>middleware = pass-through<br/>NO file reads, NO 403s]
    Build -->|--features prod-license<br/>PROD| Prod[Full licensing active:<br/>verifier, cache, RequireFeature,<br/>audit, metrics, grace period]

    Dev -.never executes.-> X[Licensing logic]
    Prod --> X

    style Dev fill:#c8e6c9
    style Prod fill:#fff9c4
    style X fill:#ffcccc
```

The cargo feature is the **only** switch. No env var, no runtime config — it's compile-time, non-bypassable.

---

## 9. Future request flow (after T13 cache + T14 RequireFeature + T15 grace)

```mermaid
sequenceDiagram
    autonumber
    actor Client
    participant MW as enforce_license_middleware
    participant Cache as LicenseCache (Arc+ArcSwap)
    participant Disk as license.json
    participant RF as RequireFeature(Oracle) layer
    participant Handler as Route Handler

    Client->>MW: HTTP request /api/v1/oracle/...
    MW->>Cache: get_payload()
    alt Cache fresh (TTL not expired, mtime unchanged)
        Cache-->>MW: cached payload
    else Stale or mtime changed
        Cache->>Disk: read + Ed25519 verify
        Disk-->>Cache: payload
        Cache-->>MW: refreshed payload
    end

    alt State == Valid
        MW->>RF: continue
    else State == InGracePeriod
        MW->>RF: continue (+ X-Zentra-License-Status header)
    else State == Expired & GET on allowlist
        MW->>RF: continue (read-only mode)
    else State == Expired & anything else
        MW-->>Client: 403 + JSON explanation
    end

    RF->>RF: feature flag check
    alt Feature off
        RF-->>Client: 403
    else Feature on
        RF->>Handler: continue
        Handler-->>Client: 200 OK
    end
```

---

## 10. Rollout timeline (Gantt)

```mermaid
gantt
    title Zentra Licensing Rollout
    dateFormat YYYY-MM-DD
    axisFormat %b %d

    section Phase 0 — Foundation
    T01 cfg-gate + remove APP_ENV    :crit, t01, 2026-05-28, 2d
    T02 CI matrix                    :t02, after t01, 1d
    T03 release-prod target          :t03, after t02, 1d
    T04 policy-engine decision spike :crit, t04, 2026-06-03, 1d

    section Phase 1 — Crypto
    T05 Ed25519 verification         :crit, t05, 2026-06-04, 4d
    T06 canonical-byte signing       :crit, t06, after t05, 2d
    T07 negative tests               :t07, after t06, 1d
    T08 versioned envelope           :t08, 2026-06-15, 2d
    T09 lock LICENSE_PATH in prod    :t09, after t08, 1d

    section Phase 2 — Consolidation
    T10 consolidate signature path   :t10, 2026-06-18, 3d
    T11 plugin uses ExpiryStrategy   :t11, after t10, 1d
    T12 cross-module tests           :t12, after t11, 1d

    section Phase 3 — Runtime
    T13 verification cache           :t13, 2026-06-25, 3d
    T14 declarative route gates      :t14, after t13, 2d

    section Phase 4 — Grace + Obs
    T15 grace period + read-only     :t15, 2026-07-02, 3d
    T16 audit log                    :t16, after t15, 1d
    T17 health metrics               :t17, after t16, 1d

    section Phase 5 — UI
    T18 single feature catalog       :t18, 2026-07-09, 3d
    T19 frontend integration         :t19, after t18, 2d

    section Phase 6 — Tooling
    T20 signer CLI                   :t20, 2026-07-16, 3d
    T21 operator docs                :t21, after t20, 2d
    T22 repo cleanup                 :t22, 2026-07-22, 1d

    section Buffer
    Contingency / sign-off           :buffer, 2026-07-23, 5d
```

---

## 11. Task dependency graph

```mermaid
graph TD
    T01[T01 cfg-gate] --> T02[T02 CI matrix]
    T01 --> T03[T03 release-prod]
    T01 --> T04[T04 decision spike]
    T01 --> T05[T05 Ed25519]
    T05 --> T06[T06 canonical signing]
    T06 --> T07[T07 negative tests]
    T06 --> T08[T08 versioned envelope]
    T08 --> T09[T09 lock LICENSE_PATH]
    T04 --> T10[T10 consolidate signatures]
    T10 --> T11[T11 plugin uses ExpiryStrategy]
    T11 --> T12[T12 consistency tests]
    T08 --> T13[T13 verification cache]
    T13 --> T14[T14 RequireFeature]
    T14 --> T15[T15 grace period]
    T11 --> T15
    T15 --> T16[T16 audit log]
    T16 --> T17[T17 metrics]
    T14 --> T18[T18 feature catalog]
    T15 --> T19[T19 frontend]
    T18 --> T19
    T08 --> T20[T20 signer CLI]
    T20 --> T21[T21 docs]
    T04 --> T21
    T17 --> T22[T22 cleanup]

    classDef crit fill:#ffcccc,stroke:#c00,stroke-width:2px
    class T01,T04,T05,T06 crit
```

🔴 red nodes = critical path. T01 unblocks everything; T04 unblocks consolidation; T05+T06 are the security-critical core.

---

## 12. License state machine (after T15 grace period)

```mermaid
stateDiagram-v2
    [*] --> Unverified: process start

    Unverified --> Valid: verify OK,<br/>before expiry
    Unverified --> InGracePeriod: verify OK,<br/>within grace_days after expiry
    Unverified --> Expired: verify OK,<br/>past grace
    Unverified --> Invalid: signature fail<br/>or parse error

    Valid --> InGracePeriod: time passes,<br/>expiry crossed
    InGracePeriod --> Expired: time passes,<br/>grace_days elapsed

    state Valid {
        [*] --> ServingAll
        ServingAll: All routes allowed
    }
    state InGracePeriod {
        [*] --> ServingWithBanner
        ServingWithBanner: All routes allowed<br/>X-Zentra-License-Status header<br/>UI shows banner
    }
    state Expired {
        [*] --> ReadOnly
        ReadOnly: GET on allowlist OK<br/>Everything else 403
    }
    state Invalid {
        [*] --> AllBlocked
        AllBlocked: Every request 403
    }
```

---

## How to use these diagrams

- **Onboarding a new engineer** → start with diagram 1 (overview), then 2 (request flow), then 6 (call graph).
- **Security review** → diagrams 2, 3, 9 highlight the verification path and where Ed25519 + canonical bytes drop in.
- **Sprint planning** → diagrams 10 (timeline) and 11 (dependencies) drive the schedule.
- **Product / sales conversations** → diagram 12 (state machine) and 7 (current-vs-target) are the most accessible.
