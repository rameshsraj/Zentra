# How Zentra's Licensing Model Works

## Context

The user asked an explanatory question — "how does the licensing model work" — not a request to change code. This file is therefore a walkthrough of the current implementation, not an implementation plan. No code edits will follow from this document. Companion work items (gap analysis, prompts, dated rollout) already live in `C:\git\Zentra\LICENSING_PROMPTS.md`.

The repo contains **two unrelated subsystems that both use the word "license"**, and most confusion about this code comes from conflating them. The walkthrough below treats them as separate stories.

---

## Subsystem A — Runtime entitlement (does Zentra itself run?)

This is the gate that decides whether the Zentra binary will serve API requests at all, and which premium features it unlocks.

### Data shape — `LicenseFile`

Defined in `crates/zentra-core/src/models/license_verification.rs`:

```rust
LicenseFile {
    payload: LicensePayload {
        org_id: String,
        customer_name: String,
        onboarded_at: DateTime<Utc>,
        trial_duration_days: i64,
        max_targets: usize,
        features: LicenseFeatures {
            enable_oracle: bool,
            enable_ebpf_network: bool,
            enable_ai_observability: bool,
            enable_otel: bool,
            allowed_menu_items: Vec<String>,
        },
    },
    signature_hex: String,
}
```

The file is plain JSON. On disk it lives at:
- Windows: `C:\ProgramData\Zentra\license.json`
- Linux/macOS: `/etc/zentra/license.json`
- Override (any platform): `ZENTRA_LICENSE_PATH` env var.

### Verification — `crypto/license_verifier.rs`

Two entry points:

1. **`check_license_active()`** (`license_verifier.rs:13`) decides whether enforcement is on. Returns true if either:
   - the `prod-license` cargo feature is compiled in (but that feature is undeclared in any `Cargo.toml`, so this branch is dead today), OR
   - `APP_ENV=production` env var is set.

   In dev mode (neither condition), the rest of the licensing system is bypassed.

2. **`load_and_verify_license()`** (`license_verifier.rs:45`) reads the JSON file, parses it, and validates the signature plus expiry. Internally it calls `verify_license_string()`:
   - Re-serialises `payload` via `serde_json::to_vec`.
   - Computes `HMAC-SHA256(EMBEDDED_PUBLIC_KEY_HEX, payload_bytes)`.
   - Compares to `signature_hex` from the file.
   - If they match, checks `onboarded_at + trial_duration_days vs now` for expiry.

   `EMBEDDED_PUBLIC_KEY_HEX` is a 32-byte constant `"0102030405…1f20"` baked into the binary. The variable name says "public key" but HMAC is **symmetric** — that constant is the signing secret. Anyone with the binary can forge any payload. The constant is also a sequential test pattern, clearly a placeholder.

### Enforcement — `license_middleware.rs`

Wired in `crates/zentra-api/src/routes/mod.rs:79` as a global Axum layer:

```rust
.layer(middleware::from_fn(enforce_license_middleware))
```

For every request:
1. Call `check_license_active()`. If false (dev mode), pass through.
2. Otherwise call `load_and_verify_license()`. **This re-reads and re-parses the file every request.** Failure → 403.
3. Check the request path against four hardcoded prefix → feature-flag rules:
   - `/api/v1/oracle` → `enable_oracle`
   - `/api/v1/cilium` → `enable_ebpf_network`
   - `/api/v1/ai-observability` → `enable_ai_observability`
   - `/api/v1/{traces,metrics,logs,otel}` → `enable_otel`

   If a request hits a gated prefix and the corresponding flag is false → 403.

### Status endpoint — `handlers/license_status.rs`

`GET /api/v1/license-status` returns one of three shapes:

- **Dev mode** (`check_license_active() == false`): mock "Development Sandbox" payload with `trial_duration_days: 9999`, all features true, all menu items allowed. This is what keeps the UI working in dev.
- **Licensed**: real payload data + computed `days_remaining`.
- **Unlicensed / verification failed**: zeroed payload with `is_licensed: false` and the error string.

The frontend is *supposed* to read `features.allowed_menu_items` from this endpoint and hide UI elements accordingly. **Grep confirms it doesn't** — only a stray reference in `frontend/src/lib/api/types.ts`. So today the menu shows everything and gated routes 403 at click time.

### End-to-end flow

```
HTTP request
   ↓
enforce_license_middleware
   ↓
check_license_active()──false──→ pass through (dev)
   ↓ true
load_and_verify_license()  [disk read + JSON parse + HMAC every request]
   ↓ fail → 403
prefix match against feature flags
   ↓ feature off → 403
next.run(request)
```

---

## Subsystem B — License asset management (Zentra-the-product feature)

This subsystem lets Zentra customers track *their own* third-party software/hardware licenses. It has nothing to do with whether Zentra itself is allowed to run. Confusion arises because it lives next to subsystem A and shares the word "license."

### Model — `crates/zentra-core/src/models/license.rs`

`License` struct (40+ fields): `license_id`, `org_id`, `vendor`, `product_name`, `seat_count`, `cost_per_seat`, `expiry_date`, `status`, etc. Enums for `LicenseCategory` (software/hardware/cloud/saas/perpetual), `LicenseType` (per-seat/per-device/site/enterprise/concurrent/subscription), `LicenseStatus`, `ComplianceStatus`.

### Schema — `migrations/20250101000003_license_management.sql`

Four tables and two views:
- `licenses` — the records themselves.
- `license_seats` — individual seat assignments.
- `license_assignments` — which targets/users consume which seats.
- `license_audit_log` — append-only audit trail.
- View `license_compliance_summary` — computes assigned/free seats and compliance status in SQL.
- View `licenses_expiring_soon` — active licenses expiring within 90 days.

### API — `crates/zentra-api/src/handlers/licenses.rs`

Standard CRUD plus seat assignment, audit log, and expiring-soon queries. Scoped per-org via an `OrgId` extractor — this subsystem is multi-tenant from the start.

### Background scheduler — `plugins/license-plugin/src/lib.rs`

A `LicensePlugin` engine that:
1. Periodically (default daily) calls a `LicenseProvider` trait to fetch the org's licenses.
2. Runs `check_expiry`, banding each license as Ok / Warning (≤90d) / Critical (≤30d) / Expired.
3. Runs `calculate_compliance` to produce a `ComplianceReport` (seat utilisation %, over-licensed count, under-utilised count, wasted cost).
4. Optionally auto-transitions Active → Expired.
5. Emits ~11 aggregate metrics + per-license utilisation metrics into Zentra's metric pipeline (`license.total_count`, `license.compliance_score`, `license.utilization_percent`, `license.wasted_cost`, etc.).

Eight unit tests cover severity bands, compliance counts, score calculation, metric emission, and wasted-cost math.

---

## Subsystem C — The dead policy engine

`crates/zentra-core/src/models/license_policy.rs` is **908 lines of designed, never-used code**. It contains:

- `ExpiryStrategy` enum with `HardExpire`, `GracePeriod { grace_days }`, `NotifyOnly`, `NoExpiry`.
- `RenewalStrategy`, `RenewalAction`.
- `TransferPolicy`, `TransferResult`.
- `OveragePolicy`, `OverageResult`.
- `MachineFingerprint` (hardware-binding scaffolding).
- `MachineHeartbeat`, `HeartbeatConfig`, `HeartbeatStatus` (dead-machine seat auto-release).
- `Entitlement`, `LicenseTier` (tiered feature flags).
- `LicenseKey::generate/validate` — a second HMAC-SHA256 signature pipeline producing keys of the form `INFRA-{base64_payload}.{base64_sig}`.
- Composite `LicensePolicy` struct combining all of the above.

`grep` for any consumer outside `crates/zentra-core/src/models/mod.rs` returns nothing. The verifier in subsystem A reimplements its own (worse) signature path; the plugin in subsystem B reimplements its own (cruder) expiry banding. None of the tier/fingerprint/heartbeat plumbing is reachable.

This is the largest source of architectural drift in the licensing area.

---

## Summary table — what calls what

| Caller | Calls | Purpose |
|---|---|---|
| Every HTTP request | `enforce_license_middleware` | Verify subsystem A license, check feature flags |
| `enforce_license_middleware` | `check_license_active()` + `load_and_verify_license()` | Decide if enforcement is on, then verify |
| `load_and_verify_license()` | HMAC-SHA256 over re-serialised payload | "Signature" check |
| `GET /api/v1/license-status` | `load_and_verify_license()` (or mock in dev) | Return entitlement state to (nobody — frontend doesn't consume it yet) |
| `licenses` REST API | `repositories::licenses` (subsystem B) | CRUD on customer's tracked third-party licenses |
| `LicensePlugin::run_scheduler` (background tokio task) | `LicenseProvider::fetch_licenses` + `check_expiry` + `calculate_compliance` + `emit_metrics` | Daily compliance scan for subsystem B |
| Nothing | `models/license_policy.rs` | Dead code |

---

## Known issues at a glance

This is the headline list — full detail is in `C:\git\Zentra\LICENSING_PROMPTS.md`:

1. **Forgeable signatures** — symmetric HMAC with embedded secret (subsystem A).
2. **Non-canonical signing** — re-serialised JSON is signed; key order / whitespace can break verification.
3. **Bypassable enforcement** — unset `APP_ENV` and licensing turns off.
4. **No verification cache** — disk + parse + MAC every request.
5. **Hardcoded route prefixes** — gating is not co-located with route definitions.
6. **No grace period** — every failure mode is a hard 403.
7. **908 lines of dead duplicate logic** in `license_policy.rs`.
8. **Frontend doesn't consume `/license-status`** — `allowed_menu_items` feeds nothing.
9. **No versioned envelope** — key/algorithm rotation requires a flag day.
10. **No signing tool in repo** — issuing licenses is presumably ad-hoc.
11. **Two definitions of "expiring soon"** between plugin and policy module.
12. **No persistent audit of license events** in subsystem A.

---

## Verification — how to confirm any of the above yourself

These are all read-only checks that can be run without changing anything:

1. **Confirm HMAC + embedded secret:** open `crates/zentra-core/src/crypto/license_verifier.rs:10` and `:67-83`.
2. **Confirm `APP_ENV` bypass:** `license_verifier.rs:13-26`.
3. **Confirm the `prod-license` feature is undeclared:** `grep -r "prod-license" --include=Cargo.toml` — zero matches.
4. **Confirm per-request verification:** `crates/zentra-api/src/license_middleware.rs:17`, no cache layer in `AppState`.
5. **Confirm dead policy engine:** `grep -rE "license_policy|LicensePolicy|LicenseKey::|ExpiryStrategy" --include='*.rs'` — only `models/mod.rs` references it.
6. **Confirm frontend non-consumption:** `grep -rE "license-status|allowed_menu_items|enable_oracle" frontend/src` — empty.
7. **Confirm hardcoded prefixes:** `license_middleware.rs:28-52`.
8. **Confirm subsystem B is fine:** read `plugins/license-plugin/src/lib.rs` tests (`cargo test -p license-plugin` — read-only, but invokes test runner).

No code changes are needed to validate the description above.
