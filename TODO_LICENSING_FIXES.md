# Zentra Licensing — TODO Prompts

Actionable prompts for fixing the issues found in the runtime entitlement / product license implementation. Each prompt is self-contained and can be handed to an agent or used as a work item. Prompts are ordered by priority (P0 = security-critical, P3 = nice-to-have).

Scope note: these target the **runtime entitlement** half of the licensing story (the signed file that gates Zentra itself). The asset-management half (License CRUD / seat tracking / compliance plugin) is in good shape and is not covered here.

---

## Global constraint — PROD toggle gates ALL licensing logic

**Hard rule for every prompt below.** None of this work may run, load files, hit disk, parse signatures, gate routes, emit audit events, or otherwise activate in the **dev environment**. Licensing must be a complete no-op unless a single explicit PROD toggle is enabled. Specifically:

- **The toggle is the `prod-license` cargo feature, declared in `crates/zentra-core/Cargo.toml`.** It is NOT a default feature. Dev builds (`cargo build`, `cargo run`, `cargo test`) must compile and run with no license file, no key material, no network calls, and no 403s.
- **No runtime env-var bypass.** The `APP_ENV` check in `license_verifier.rs:13` is removed entirely. The only switch is the compile-time feature.
- **Every code path added by these prompts must be guarded by `#[cfg(feature = "prod-license")]`** at the highest reasonable level — the middleware layer, the cache, the per-route `RequireFeature` extractor, the audit emitters. When the feature is off, these symbols either don't exist or compile to a trivial pass-through.
- **`check_license_active()` becomes a `const fn` that returns `cfg!(feature = "prod-license")`.** No env reads, no file checks.
- **`get_license_status` handler** (`crates/zentra-api/src/handlers/license_status.rs`) keeps its existing dev-mode behaviour of returning a mock "unlimited" payload when the feature is off. That dev fallback is the ONE intentional exception so the frontend can render normally during development.
- **Production builds** must pass `--features zentra-core/prod-license` (and any transitive feature flags needed in `zentra-api`). Add a `release-prod` target to the build pipeline so this is not forgotten.
- **CI matrix** must build and test both `--no-default-features` (dev) and `--features prod-license` (prod), so neither path bit-rots.

Every prompt below assumes this constraint. If a prompt mentions enforcement, caching, gating, etc., understand it as "...when `prod-license` is enabled; otherwise it does not exist."

---

## P0-1 — Replace symmetric HMAC with real asymmetric signature verification

**Problem.** `crates/zentra-core/src/crypto/license_verifier.rs:67-83` calls the signature check "public key verification" but actually runs `Hmac<Sha256>` keyed by a hardcoded 32-byte constant `EMBEDDED_PUBLIC_KEY_HEX = "0102…1f20"`. HMAC is symmetric, so the value embedded in the binary IS the signing secret — anyone with the binary can forge any license payload. The constant is also a sequential test pattern, clearly placeholder.

**Prompt.**
> Replace the HMAC-SHA256 signature scheme in `crates/zentra-core/src/crypto/license_verifier.rs` with Ed25519 asymmetric verification.
>
> Requirements:
> - Add `ed25519-dalek` to the workspace `Cargo.toml` and to `zentra-core/Cargo.toml`.
> - Embed a real Ed25519 **public** key (32 bytes) as a `const` in `license_verifier.rs`. Add a `// TODO: replace with production public key before release` marker and a placeholder generated with `ed25519_dalek::SigningKey::generate(&mut OsRng)` for now. Document in a comment that the matching private key lives outside this repo.
> - Change `LicenseFile` (in `crates/zentra-core/src/models/license_verification.rs`) so the signature field is clearly an Ed25519 signature (64 bytes hex-encoded). Keep the field name `signature_hex` to minimise churn but update the doc comment.
> - In `verify_license_string`, sign over the canonical UTF-8 bytes of the payload (see P0-2 for canonicalisation — for now, sign over the exact bytes that appear inside the file rather than re-serialising).
> - Remove the `hmac` and `sha2` dependencies from this code path if no longer needed.
> - Update or add unit tests in `license_verifier.rs` that generate a keypair, sign a payload, verify it succeeds, and verify that flipping any payload byte causes verification to fail.

---

## P0-2 — Sign canonical bytes, not re-serialised JSON

**Problem.** `license_verifier.rs:63` computes the MAC over `serde_json::to_vec(&license_file.payload)` — i.e. it re-serialises the parsed payload and signs that. Serde JSON output is not canonical: key ordering, whitespace, and numeric formatting can differ between signer and verifier, breaking verification non-deterministically.

**Prompt.**
> Change `verify_license_string` in `crates/zentra-core/src/crypto/license_verifier.rs` so it verifies the signature over the *original* payload bytes as they appear in the license file, not a re-serialisation.
>
> Approach:
> - Change `LicenseFile` to deserialise `payload` as a `serde_json::value::RawValue` (or store the raw payload string alongside the parsed struct). This preserves the exact bytes the signer used.
> - The signer (whatever tool generates `license.json`) must then sign those exact bytes. Document the signing contract in a `// SIGNING CONTRACT:` comment block at the top of `license_verifier.rs`.
> - Add a test that round-trips a license file with non-default key ordering and unusual whitespace to confirm verification is byte-exact, not structure-equivalent.
>
> If `RawValue` introduces ergonomic issues downstream, the fallback is to define a deterministic canonical serialisation (sorted keys, no whitespace, ASCII-escaped) and document it as the signing contract — but `RawValue` is simpler.

---

## P0-3 — Make `prod-license` the single, compile-time licensing toggle

**Problem.** `check_license_active()` at `license_verifier.rs:13-26` enables enforcement when (a) the `prod-license` cargo feature is compiled in, OR (b) `APP_ENV=production`. Anyone running the binary can unset `APP_ENV` and disable all licensing. Worse, grepping the repo shows `prod-license` is not declared in any `Cargo.toml`, so the cfg branch is dead code and only the (bypassable) env-var path is active. We want the inverse: dev = no licensing at all, prod = enforced — controlled by ONE compile-time switch.

**Prompt.**
> Make the `prod-license` cargo feature the single, non-bypassable switch that activates ALL licensing logic. Dev builds must contain none of it at runtime.
>
> Required changes:
> 1. Declare the feature in `crates/zentra-core/Cargo.toml`:
>    ```toml
>    [features]
>    default = []
>    prod-license = []
>    ```
>    Add a matching pass-through feature in `crates/zentra-api/Cargo.toml`:
>    ```toml
>    [features]
>    default = []
>    prod-license = ["zentra-core/prod-license"]
>    ```
> 2. Rewrite `check_license_active()` in `crates/zentra-core/src/crypto/license_verifier.rs` to be a one-liner:
>    ```rust
>    pub const fn check_license_active() -> bool {
>        cfg!(feature = "prod-license")
>    }
>    ```
>    Delete the `APP_ENV` branch entirely.
> 3. Gate the entire module (or at minimum every public function that touches license files, keys, or verification) behind `#[cfg(feature = "prod-license")]`. Provide stub no-op equivalents under `#[cfg(not(feature = "prod-license"))]` so call sites compile unchanged but do nothing.
> 4. In `crates/zentra-api/src/license_middleware.rs`, gate `enforce_license_middleware` similarly: when the feature is off, the function body is just `Ok(next.run(request).await)` — no file reads, no parsing, no logging about licensing.
> 5. In `crates/zentra-api/src/routes/mod.rs:79`, only attach the middleware layer when `#[cfg(feature = "prod-license")]`. Use `cfg_if!` or two separate `Router` build paths.
> 6. Keep `get_license_status` returning the existing mock "unlimited" payload when the feature is off (this is the documented dev-mode exception so the frontend renders normally).
> 7. Build / CI:
>    - Add a `release-prod` target to the build pipeline (Makefile / justfile / CI workflow) that passes `--features zentra-core/prod-license,zentra-api/prod-license`.
>    - Add a CI job matrix entry that runs `cargo build --no-default-features` AND `cargo build --features prod-license` to prevent either path from rotting.
>    - Add `cargo test --features prod-license` to the matrix as well.
> 8. Tests:
>    - Under `#[cfg(not(feature = "prod-license"))]`, add a test confirming the middleware passes through with no license file present anywhere.
>    - Under `#[cfg(feature = "prod-license")]`, add a test confirming a missing license file produces a 403.
>    - Add a doc comment on `check_license_active()` stating "this is the single source of truth for whether licensing is active — do not add other toggles."

---

## P1-1 — Cache verified license; do not re-read on every request

**Problem.** The Axum middleware (`crates/zentra-api/src/license_middleware.rs:17`) calls `load_and_verify_license()` per request. That's a disk read + JSON parse + signature verify on every HTTP request, with no caching.

**Prompt.**
> Add an in-memory cache for the verified license payload. The entire cache module must be gated on `#[cfg(feature = "prod-license")]` — in dev builds it does not exist.
>
> Design:
> - Create a `LicenseCache` struct in `crates/zentra-api/src/license_middleware.rs` (or a new `license_cache.rs`) holding `Arc<ArcSwap<Result<LicensePayload, String>>>` plus a `last_loaded: Instant`.
> - On the first request after process start, load and verify; cache the result.
> - Re-verify on a TTL (e.g. 5 minutes — make it configurable via `ZENTRA_LICENSE_REFRESH_SECS`, default 300). Re-verification can be lazy (checked on request) or background (tokio task).
> - Inject the cache into `AppState` and have `enforce_license_middleware` read from it.
> - Also re-load if the file's mtime changes, so operators can drop in a new license without restarting. Use `std::fs::metadata().modified()`.
> - Keep the existing fail-closed behaviour: if verification fails, return 403.
> - Add a test that mutates a temp license file and confirms the cache picks up the new content within the TTL.

---

## P1-2 — Decouple per-route feature gates from hardcoded path prefixes

**Problem.** `license_middleware.rs:28-52` hardcodes URL prefixes (`/api/v1/oracle`, `/api/v1/cilium`, `/api/v1/ai-observability`, `/api/v1/{traces,metrics,logs,otel}`) and maps them to feature flags. Adding a new gated route requires editing the middleware, and the mapping is invisible from the route definitions themselves.

**Prompt.**
> Replace the hardcoded path-prefix matching in `crates/zentra-api/src/license_middleware.rs` with a declarative per-route feature requirement. The `RequireFeature` extractor/layer must be a no-op when the `prod-license` feature is disabled (dev builds): provide it under both cfg branches with identical signatures but the dev variant immediately returns success.
>
> Approach:
> - Define an enum `RequiredFeature { Oracle, EbpfNetwork, AiObservability, Otel }` in `zentra-core` next to `LicenseFeatures`.
> - Create an Axum extractor or tower layer `RequireFeature(RequiredFeature)` that, when applied to a route, checks the cached license payload's feature flags and returns 403 if missing.
> - In `crates/zentra-api/src/routes/mod.rs` and the per-module routers, attach `RequireFeature(...)` to the relevant route groups (e.g. `oracle::router().layer(RequireFeature(RequiredFeature::Oracle))`).
> - The global `enforce_license_middleware` keeps responsibility only for "license file exists, signature valid, not expired" — feature gating moves to per-route layers.
> - Delete the prefix checks in the middleware.
> - Add an integration test asserting that a route layered with `RequireFeature(Oracle)` returns 403 when the cached license has `enable_oracle: false`.

---

## P1-3 — Add a grace period / read-only mode for expired licenses

**Problem.** Expiry, missing file, parse error, and signature failure all return the same `403 FORBIDDEN`. There's no soft-expiry window, no read-only fallback, and no actionable signal to the operator. Customers whose license lapsed by one day get a hard outage with no warning.

**Prompt.**
> Introduce graceful expiry handling in the runtime license enforcement. All grace-period logic lives behind `#[cfg(feature = "prod-license")]` — dev builds have no concept of expiry.
>
> Behaviour:
> - Add a `grace_period_days: i64` field to `LicensePayload` (default 14 if absent — handle absence via `serde(default)`).
> - In `verify_license_string` (`crates/zentra-core/src/crypto/license_verifier.rs:86-96`), distinguish three states: `Valid`, `InGracePeriod { days_overdue }`, and `Expired`. Return an enum, not just `Result<Payload, String>`.
> - In `enforce_license_middleware`, when in grace period: allow the request, attach a response header `X-Zentra-License-Status: grace-period` and `X-Zentra-License-Days-Overdue: N`, and log a warning (rate-limited).
> - When fully expired, allow only GET requests to a small allowlist of read-only paths (`/api/v1/license-status`, `/api/v1/dashboard`, health endpoints). Everything else returns 403 with a JSON body explaining the state.
> - Update `get_license_status` in `crates/zentra-api/src/handlers/license_status.rs` to return the new state enum and `days_overdue` so the UI can show a banner.
> - Add tests for: valid, grace, fully expired (read allowed), fully expired (write blocked).

---

## P2-1 — Stop signing/verifying with `signature_hex`; add a versioned envelope

**Problem.** The license file has no format version field, no algorithm identifier, and no key ID. Rotating the signing key or changing the algorithm later (e.g. P0-1 → Ed25519) will be a hard cutover with no way for a single binary to support both old and new licenses during a transition.

**Prompt.**
> Add a versioned, algorithm-tagged envelope to the license file format.
>
> Schema change in `crates/zentra-core/src/models/license_verification.rs`:
> ```rust
> pub struct LicenseFile {
>     pub format_version: u32,        // currently 1
>     pub algorithm: String,          // "ed25519" after P0-1
>     pub key_id: String,             // e.g. "zentra-prod-2026-01"
>     pub payload: Box<RawValue>,     // canonical bytes (P0-2)
>     pub signature: String,          // hex
> }
> ```
> In `license_verifier.rs`, maintain a `HashMap<&'static str, [u8; 32]>` of known key IDs to public keys. Verification looks up the public key by `key_id` and rejects unknown IDs. This lets multiple keys coexist for staged rotation.
>
> Bump `format_version` to 1. Document the upgrade path in a comment.
>
> Update the signing tool (wherever it lives — find it via `grep -r "signature_hex"` or create one under `crates/zentra-cli/`) to emit the new format.

---

## P2-2 — Align UI menu-item allowlist with server-side enforcement

**Problem.** `license_status.rs` returns `allowed_menu_items` for UI gating, while the middleware enforces a separate prefix list. The two lists are maintained independently and can drift — a user could see a menu item the server then 403s on, or (worse) hit a route that should be gated but isn't in the prefix list.

**Prompt.**
> Make `allowed_menu_items` and server-side feature enforcement derive from a single source of truth.
>
> After P1-2 lands (declarative `RequireFeature` per route), do this:
> - Define a `FeatureCatalog` in `zentra-core` that maps each `RequiredFeature` to (a) its license flag and (b) the set of UI menu item IDs it unlocks.
> - Remove `allowed_menu_items` from the persisted `LicensePayload`. Instead, derive it in `get_license_status` from the catalog given the payload's feature flags.
> - This means a single edit to the catalog updates both UI visibility and route gating, eliminating drift.
> - Update the frontend (under `frontend/`) to consume the derived list — should require no change if it already reads `features.allowed_menu_items`.
> - Bump `format_version` and add a back-compat path that ignores any `allowed_menu_items` field present in older files.

---

## P3-1 — Provide an offline license-signing CLI

**Problem.** There's no tooling visible in-tree for generating signed license files. Whoever issues licenses today is presumably running an ad-hoc script. Without a tracked tool, the signing process is undocumented and error-prone (especially given P0-2's canonicalisation requirement).

**Prompt.**
> Create a small CLI at `crates/zentra-license-signer/` that issues signed license files.
>
> Commands:
> - `zentra-license-signer keygen --out-priv prod.key --out-pub prod.pub` — generates an Ed25519 keypair.
> - `zentra-license-signer sign --priv prod.key --key-id zentra-prod-2026-01 --customer "Acme Corp" --org-id <uuid> --trial-days 365 --max-targets 500 --features oracle,otel --grace-days 14 --out license.json` — emits a verified file matching the P2-1 envelope.
> - `zentra-license-signer verify --pub prod.pub license.json` — verifies a file (useful for support diagnostics).
>
> The signer must use the same canonicalisation contract as the verifier (P0-2). Add an integration test that signs a file with the CLI and verifies it with `license_verifier::verify_license_string`.
>
> Document operator workflow (key storage in vault, key rotation cadence) in `docs/licensing/SIGNING.md`.

---

## P3-2 — Audit log for license verification outcomes

**Problem.** When verification fails (tampering attempt, expired file, missing file), the only signal is a `tracing::error!` and a 403. There's no persistent record for incident review.

**Prompt.**
> Wire license verification outcomes into the existing audit log infrastructure.
>
> - `license_audit_log` already exists in `migrations/20250101000003_license_management.sql` for the asset-management half. Either reuse it with a new `action` value (`runtime_license_verified`, `runtime_license_failed`) and `license_id = nil`, or create a parallel `runtime_license_events` table — pick whichever is cleaner.
> - Emit an event on: process startup verification, cache refresh, verification failure, expiry transition, grace-period entry.
> - Include `customer_name`, `key_id` (P2-1), error reason, and source path.
> - Add a handler `GET /api/v1/license-status/events?limit=100` returning recent events for the ops UI.

---

## Suggested execution order

1. **Do P0-3 first.** It establishes the `prod-license` feature flag and the cfg-gating skeleton that every other prompt assumes. After P0-3, dev builds are guaranteed to have zero licensing code paths active, so subsequent prompts can be developed and tested without breaking the dev workflow.
2. **P0-1, P0-2 together** as a single PR — they're entangled (you'll touch `license_verifier.rs` for both) and together close the cryptographic holes. Both must remain inside the `#[cfg(feature = "prod-license")]` gate created in P0-3.
2. **P1-1** — quick win, removes per-request I/O.
3. **P1-2** — refactor; do this before P2-1 because P2-1 builds on per-route gating.
4. **P1-3** — operator-facing improvement; can ship independently of P2-x.
5. **P2-1**, then **P2-2** — schema evolution; do once and bump the format version.
6. **P3-1**, **P3-2** — tooling polish.

Before starting P0-1, verify the public key embedded in the binary won't break any existing in-the-wild license files. If signed licenses are already deployed to customers, plan a transition window where the verifier accepts both old (HMAC) and new (Ed25519) formats keyed by `format_version`, then sunset HMAC after a deprecation period.
