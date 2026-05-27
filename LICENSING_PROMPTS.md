# Zentra Licensing — Unified Prompts, Gap Analysis & Dated Plan

**Document date:** 2026-05-27
**Target completion:** 2026-07-22 (8-week plan + 1 buffer week)
**Owner:** TBD
**Supersedes:** `TODO_LICENSING_FIXES.md`, `LICENSING_GAP_ANALYSIS_AND_PLAN.md` (both can be deleted)
**Scope:** Runtime entitlement / product licensing only. Asset-management licensing (License CRUD, seats, audit) is out of scope — it is in good shape.

---

## 1. Executive summary

Zentra's runtime licensing has three headline problems:

- **Security: critical.** Signature scheme is symmetric HMAC keyed by a hardcoded sequential constant `EMBEDDED_PUBLIC_KEY_HEX = "0102…1f20"`. Anyone with the binary can forge a license. Enforcement is also bypassable by unsetting `APP_ENV`.
- **Architecture: significant duplication.** A 908-line `license_policy.rs` module implements expiry strategies, machine fingerprinting, heartbeat, tiers, and a `LicenseKey` HMAC pipeline — wired into nothing. The active verifier and the license-plugin scheduler reimplement subsets independently.
- **Operations & UX: incomplete.** No caching (per-request disk I/O), no grace period, no key rotation story, no frontend integration of the menu-item allowlist, no signing tool in-tree.

---

## 2. Global constraint — PROD toggle gates ALL licensing logic

**Hard rule for every task below.** None of this work may run, load files, hit disk, parse signatures, gate routes, emit audit events, or otherwise activate in the **dev environment**. Licensing must be a complete no-op unless a single explicit PROD toggle is enabled:

- **The toggle is the `prod-license` cargo feature, declared in `crates/zentra-core/Cargo.toml`.** Not a default feature. Dev builds (`cargo build`, `cargo run`, `cargo test`) must compile and run with no license file, no key material, no network calls, no 403s.
- **No runtime env-var bypass.** The `APP_ENV` check at `license_verifier.rs:13` is removed entirely. The only switch is the compile-time feature.
- **Every code path added must be guarded by `#[cfg(feature = "prod-license")]`** at the highest reasonable level — middleware, cache, route extractors, audit emitters. When the feature is off, these symbols either don't exist or compile to a trivial pass-through.
- **`check_license_active()` becomes a `const fn` returning `cfg!(feature = "prod-license")`.** No env reads, no file checks.
- **`get_license_status` handler keeps its existing dev-mode behaviour** of returning a mock "unlimited" payload when the feature is off — the ONE intentional exception so the frontend renders during development.
- **Production builds** must pass `--features zentra-core/prod-license` (and transitive flags in `zentra-api`).
- **CI matrix** must build and test both `--no-default-features` (dev) and `--features prod-license` (prod) so neither path bit-rots.

---

## 3. Gap analysis

Legend: 🔴 critical · 🟠 high · 🟡 medium · 🟢 low

| # | Area | Current state | Gap | Severity | Closed by |
|---|---|---|---|---|---|
| G1 | Signature crypto | HMAC-SHA256 with constant `EMBEDDED_PUBLIC_KEY_HEX = "0102…1f20"` ([license_verifier.rs:10](crates/zentra-core/src/crypto/license_verifier.rs:10)) | Symmetric secret embedded in binary — trivially forgeable | 🔴 | T05 |
| G2 | Signed-payload canonicalisation | Re-serialises payload via `serde_json::to_vec` before MAC ([license_verifier.rs:63](crates/zentra-core/src/crypto/license_verifier.rs:63)) | Non-canonical — key order / whitespace can break verification | 🔴 | T06 |
| G3 | Enforcement toggle | `prod-license` feature OR `APP_ENV=production`; feature undeclared anywhere; env path is bypassable | Trivial bypass; dead cfg branch | 🔴 | T01 |
| G4 | Verification caching | `load_and_verify_license()` per request ([license_middleware.rs:17](crates/zentra-api/src/license_middleware.rs:17)) | Disk + parse + MAC every request | 🟠 | T13 |
| G5 | Per-route feature gating | Hardcoded URL prefix list ([license_middleware.rs:28-52](crates/zentra-api/src/license_middleware.rs:28)) | Editing middleware required to add gated routes | 🟠 | T14 |
| G6 | Expired-license behaviour | All failures → 403 | No grace window, no read-only fallback | 🟠 | T15 |
| G7 | **Dead policy engine** | `license_policy.rs` (908 lines) defines `ExpiryStrategy::GracePeriod`, `RenewalStrategy`, `TransferPolicy`, `OveragePolicy`, `MachineFingerprint`, `HeartbeatConfig`, `LicenseTier`, `LicenseKey::generate/validate` — used by nothing | Massive duplication; two HMAC implementations; tier/entitlement system unused | 🟠 | T04, T10, T11 |
| G8 | Frontend integration | Backend exposes `allowed_menu_items`; frontend has no consumer | UI shows everything, then routes 403 | 🟠 | T18, T19 |
| G9 | License file format | `LicenseFile { payload, signature_hex }` — no `format_version`, `algorithm`, `key_id` | Cannot rotate keys without flag-day cutover | 🟡 | T08 |
| G10 | Signing tool | None in repo | Operator workflow undocumented | 🟡 | T20 |
| G11 | Audit / observability | `tracing::error!` on failure only | No persistent incident trail | 🟡 | T16 |
| G12 | License-plugin scheduler tie-in | `warning_threshold_days`/`critical_threshold_days` independent of `ExpiryStrategy` | Two definitions of "expiring soon" can drift | 🟡 | T11 |
| G13 | Test coverage | No negative tests on crypto path | Security-critical path has weakest tests | 🟡 | T07 |
| G14 | CI matrix | `prod-license` build not exercised in CI | Feature path rots silently | 🟡 | T02 |
| G15 | Repo clutter | `*_check.txt`, `*_errors.txt`, `server_*.log` committed to root | Working noise | 🟢 | T22 |
| G16 | `ZENTRA_LICENSE_PATH` override | Env var honoured ([license_verifier.rs:30](crates/zentra-core/src/crypto/license_verifier.rs:30)) | Lets attacker point at self-issued file | 🟡 | T09 |

---

## 4. Phased timeline

Today is 2026-05-27. Working assumptions: one engineer at ~80% capacity, 5-day weeks. Dates = *target completion* (end-of-day).

| Phase | Window | Theme | Tasks | Exit criteria |
|---|---|---|---|---|
| **0 — Foundation** | 2026-05-28 → 2026-06-03 | cfg gating + consolidation decision | T01-T04 | Dev has zero licensing artefacts; CI builds both paths; written policy-engine decision |
| **1 — Crypto correctness** | 2026-06-04 → 2026-06-17 | Real signatures, canonical bytes, versioned envelope | T05-T09 | Binary cannot accept a forged license; verification is byte-deterministic; new keys can be added without a flag day |
| **2 — Architecture consolidation** | 2026-06-18 → 2026-06-24 | Single engine, kill duplicates | T10-T12 | Single source of truth for expiry, signatures, tiers; ≥300 LOC removed |
| **3 — Performance & route gating** | 2026-06-25 → 2026-07-01 | Cache + declarative gates | T13-T14 | ≤1ms p99 license-check overhead; route gating grep-able from route defs |
| **4 — Grace, audit, metrics** | 2026-07-02 → 2026-07-08 | Operator-friendly degraded mode | T15-T17 | Lapsed customer sees warning + read-only, not outage; ops dashboards licensing |
| **5 — UI alignment** | 2026-07-09 → 2026-07-15 | Single feature catalog + frontend | T18-T19 | UI and server cannot drift; users see entitlement state |
| **6 — Tooling & docs** | 2026-07-16 → 2026-07-22 | Signer CLI + runbook | T20-T22 | Ops can issue/rotate licenses from in-repo docs alone |
| **Buffer** | 2026-07-23 → 2026-07-29 | Contingency | — | Reserve for slippage, security review, rotation rehearsal |

---

## 5. Detailed prompts (T01–T22)

Each prompt is self-contained — handoff-ready for an agent or engineer cold-starting on the task. Every prompt assumes the **Global constraint** in §2.

---

### T01 — Establish `prod-license` cargo feature; gate everything; remove `APP_ENV`

**Target:** 2026-05-29 (Fri) · **Closes:** G3 · **Severity:** 🔴

**Prompt.**
> Make the `prod-license` cargo feature the single, non-bypassable switch that activates ALL licensing logic. Dev builds must contain none of it at runtime.
>
> 1. Declare the feature in `crates/zentra-core/Cargo.toml`:
>    ```toml
>    [features]
>    default = []
>    prod-license = []
>    ```
>    Add a pass-through in `crates/zentra-api/Cargo.toml`:
>    ```toml
>    [features]
>    default = []
>    prod-license = ["zentra-core/prod-license"]
>    ```
> 2. Rewrite `check_license_active()` in `crates/zentra-core/src/crypto/license_verifier.rs` to:
>    ```rust
>    pub const fn check_license_active() -> bool {
>        cfg!(feature = "prod-license")
>    }
>    ```
>    Delete the `APP_ENV` branch entirely.
> 3. Gate the entire `license_verifier` module (or every public function that touches license files, keys, or verification) behind `#[cfg(feature = "prod-license")]`. Provide stub no-op equivalents under `#[cfg(not(feature = "prod-license"))]` so call sites compile unchanged.
> 4. In `crates/zentra-api/src/license_middleware.rs`, gate `enforce_license_middleware` similarly: feature-off body is `Ok(next.run(request).await)`, no file reads, no parsing, no logging.
> 5. In `crates/zentra-api/src/routes/mod.rs:79`, only attach the middleware layer when the feature is on (use `cfg_if!` or two `Router` build paths).
> 6. Keep `get_license_status` returning the mock "unlimited" payload when the feature is off — documented dev exception so the frontend renders.
> 7. Add a doc comment on `check_license_active()`: "single source of truth for whether licensing is active — do not add other toggles."

**Tests.**
> - Under `#[cfg(not(feature = "prod-license"))]`: middleware passes through with no license file present anywhere.
> - Under `#[cfg(feature = "prod-license")]`: missing license file produces 403.

---

### T02 — CI matrix for dev and prod feature builds

**Target:** 2026-06-01 (Mon) · **Closes:** G14 · **Severity:** 🟡

**Prompt.**
> Add CI matrix entries that run BOTH `cargo build --no-default-features` AND `cargo build --features prod-license` for the workspace; same for `cargo test`. The licensing module must compile and pass tests under each. Wire into whichever CI system the repo uses (look under `.github/workflows/`, `.gitlab-ci.yml`, etc.). If no CI exists yet, document the two required invocations in a new `docs/licensing/CI.md`.

---

### T03 — `release-prod` build target

**Target:** 2026-06-02 (Tue) · **Closes:** G3 · **Severity:** 🟡

**Prompt.**
> Add a `release-prod` target to whichever build orchestrator the project uses (`Makefile`, `justfile`, top-level shell script, or CI release workflow) that invokes:
> ```
> cargo build --release --features zentra-core/prod-license,zentra-api/prod-license
> ```
> Document in `README.md` that production binaries MUST be built via this target, and that a binary built without it will refuse to enforce licensing.

---

### T04 — Decision spike: adopt or delete `license_policy.rs`

**Target:** 2026-06-03 (Wed) · **Closes:** G7, enables E1 · **Severity:** 🟠

**Prompt.**
> `crates/zentra-core/src/models/license_policy.rs` is a 908-line module defining `ExpiryStrategy` (with `GracePeriod` variant), `RenewalStrategy`, `TransferPolicy`, `OveragePolicy`, `MachineFingerprint`, `HeartbeatConfig`, `LicenseTier`, and a complete `LicenseKey::generate/validate` HMAC pipeline. A `grep` confirms it is used by no consumer outside `mod.rs`. Meanwhile `crypto/license_verifier.rs` reimplements signature verification and `plugins/license-plugin/src/lib.rs` reimplements expiry severity bands.
>
> Decide and document: **adopt `license_policy.rs` as canonical, or delete it.**
>
> Deliverable: a new file `docs/licensing/ARCHITECTURE.md` containing:
> 1. Decision (one sentence).
> 2. Rationale (≤200 words).
> 3. Inventory of consumers/duplicates that must change (file:line for each).
> 4. Migration sequence (which call sites move when).
>
> Default recommendation: **adopt.** The policy module is better-designed than the verifier (proper strategy enums, no magic constants). Subsequent tasks (T10-T12) assume this outcome; rewrite them if the decision goes the other way.

---

### T05 — Replace HMAC with Ed25519 verification

**Target:** 2026-06-09 (Tue) · **Closes:** G1 · **Severity:** 🔴

**Prompt.**
> Replace the HMAC-SHA256 signature scheme in `crates/zentra-core/src/crypto/license_verifier.rs` with Ed25519 asymmetric verification. All work stays inside the `#[cfg(feature = "prod-license")]` gate established by T01.
>
> Requirements:
> - Add `ed25519-dalek` to the workspace `Cargo.toml` and to `zentra-core/Cargo.toml`.
> - Embed a real Ed25519 **public** key (32 bytes) as a `const` in `license_verifier.rs`. Add a `// TODO(T05): replace with production public key before release` marker and a placeholder generated with `ed25519_dalek::SigningKey::generate(&mut OsRng)`. Document in a comment that the matching private key lives outside this repo (key management addressed by T20/T21).
> - Update `LicenseFile` in `crates/zentra-core/src/models/license_verification.rs`: signature is now an Ed25519 signature (64 bytes hex-encoded). Keep the field name `signature_hex` to minimise churn but update the doc comment.
> - In `verify_license_string`, verify the signature using the embedded public key.
> - Remove `hmac` and `sha2` from this code path if no longer needed.

**Tests.**
> Add unit tests that generate a keypair, sign a payload, verify it succeeds, and verify that flipping any single payload byte causes verification to fail.

---

### T06 — Sign canonical bytes, not re-serialised JSON

**Target:** 2026-06-11 (Thu) · **Closes:** G2 · **Severity:** 🔴

**Prompt.**
> Currently `verify_license_string` in `crates/zentra-core/src/crypto/license_verifier.rs:63` computes the MAC over `serde_json::to_vec(&license_file.payload)` — i.e. it re-serialises the parsed payload and signs that. Serde JSON output is not canonical: key ordering, whitespace, and numeric formatting can differ between signer and verifier, breaking verification non-deterministically.
>
> Change verification to use the *original* payload bytes as they appear on disk:
> - Change `LicenseFile` so `payload` deserialises as `Box<serde_json::value::RawValue>` (or store the raw payload slice alongside the parsed struct). This preserves the exact bytes the signer used.
> - The signer (T20) must sign those exact bytes. Document the signing contract in a `// SIGNING CONTRACT:` comment block at the top of `license_verifier.rs`.
>
> Fallback if `RawValue` is awkward downstream: define a deterministic canonical serialisation (sorted keys, no whitespace, ASCII-escaped) and document it as the signing contract. `RawValue` is simpler.

**Tests.**
> Round-trip a license file with non-default key ordering and unusual whitespace; verification must be byte-exact, not structure-equivalent.

---

### T07 — Crypto negative tests

**Target:** 2026-06-12 (Fri) · **Closes:** G13 · **Severity:** 🟡

**Prompt.**
> Add unit tests in `crates/zentra-core/src/crypto/license_verifier.rs` (or a sibling test module) covering:
> 1. Tampered signature (last byte flipped) → rejected.
> 2. Tampered payload (single byte flipped after signing) → rejected.
> 3. Expired payload (`onboarded_at + trial_duration_days < now`) → rejected with expiry error.
> 4. Malformed JSON → rejected with parse error.
> 5. Empty file → rejected.
> 6. Wrong algorithm tag after T08 lands → rejected.
>
> Each must assert the specific error variant, not just any `Err`.

---

### T08 — Versioned signing envelope

**Target:** 2026-06-16 (Tue) · **Closes:** G9 · **Severity:** 🟡

**Prompt.**
> Add a versioned, algorithm-tagged envelope to the license file format so keys/algorithms can rotate without flag days.
>
> Schema change in `crates/zentra-core/src/models/license_verification.rs`:
> ```rust
> pub struct LicenseFile {
>     pub format_version: u32,           // currently 1
>     pub algorithm: String,             // "ed25519"
>     pub key_id: String,                // e.g. "zentra-prod-2026-01"
>     pub payload: Box<RawValue>,        // canonical bytes (from T06)
>     pub signature: String,             // hex
> }
> ```
> In `license_verifier.rs`, maintain a `HashMap<&'static str, [u8; 32]>` of known `key_id → public_key`. Verification looks up the public key by `key_id` and rejects unknown IDs. This lets multiple keys coexist during staged rotation.
>
> Bump `format_version` to 1. Document the upgrade path in a comment.

---

### T09 — Lock `ZENTRA_LICENSE_PATH` override in prod builds

**Target:** 2026-06-17 (Wed) · **Closes:** G16 · **Severity:** 🟡

**Prompt.**
> In `crates/zentra-core/src/crypto/license_verifier.rs:30`, `get_license_filepath()` honours `ZENTRA_LICENSE_PATH`. Useful for tests, but in prod it lets a user point at a self-issued file.
>
> Under `#[cfg(feature = "prod-license")]`, ignore the env var and always return the platform-fixed path (`/etc/zentra/license.json` or `C:\ProgramData\Zentra\license.json`). Under `#[cfg(not(feature = "prod-license"))]` (dev/test only), keep the override available.
>
> Add a test under prod-license cfg confirming the env var is ignored.

---

### T10 — Consolidate signature logic onto `license_policy.rs`

**Target:** 2026-06-22 (Mon) · **Closes:** G7 · **Severity:** 🟠 · **Depends on:** T04 outcome

**Prompt.**
> (Assumes T04 decided "adopt `license_policy.rs`.")
>
> Today there are two HMAC implementations: `crypto/license_verifier.rs::verify_license_string` and `models/license_policy.rs::LicenseKey::{generate,validate}`. After T05 the verifier moves to Ed25519. Now align the two:
> - Either move `LicenseKey` in `license_policy.rs` to also use Ed25519 (with a separate keypair for the license-key format), OR delete `LicenseKey` if the file-based `LicensePayload` envelope makes it redundant — review with T04's decision document.
> - The verifier should call into `license_policy.rs` types where appropriate (e.g. use `LicenseTier` instead of the ad-hoc `LicenseFeatures` struct, after appropriate field mapping in T18).
> - Goal: one canonical signature path, one canonical entitlement type. Measure LOC deleted; target ≥300.

---

### T11 — Plugin uses `ExpiryStrategy`

**Target:** 2026-06-23 (Tue) · **Closes:** G12 · **Severity:** 🟡

**Prompt.**
> `plugins/license-plugin/src/lib.rs:161-169` has ad-hoc expiry severity bands (Ok/Warning/Critical/Expired) defined inside the plugin. `license_policy.rs::ExpiryStrategy::is_usable` produces the same kind of decision with explicit strategy variants.
>
> Replace the inline match in the plugin with a call to `ExpiryStrategy::is_usable`. Map `ExpiryStatus` variants to the existing `ExpirySeverity` (or unify the two enums if the plugin is the only `ExpirySeverity` consumer). Update plugin tests to assert against the unified enum.

---

### T12 — Cross-module consistency tests

**Target:** 2026-06-24 (Wed) · **Closes:** G7, G13 · **Severity:** 🟡

**Prompt.**
> Add integration tests that feed a single payload through verifier → policy → plugin and assert all three agree on its expiry/usability state. Place under `crates/zentra-core/tests/license_consistency.rs`. At least one test per state: Valid, ExpiringSoon, InGracePeriod, Expired.

---

### T13 — License verification cache

**Target:** 2026-06-29 (Mon) · **Closes:** G4 · **Severity:** 🟠

**Prompt.**
> The middleware calls `load_and_verify_license()` per request (`crates/zentra-api/src/license_middleware.rs:17`) — disk read + JSON parse + signature verify every time. Add an in-memory cache, gated entirely behind `#[cfg(feature = "prod-license")]`.
>
> Design:
> - `LicenseCache` holding `Arc<ArcSwap<Result<LicensePayload, String>>>` plus `last_loaded: Instant` and `last_mtime: SystemTime`.
> - First request after process start: load + verify; cache result.
> - Re-verify on TTL expiry (default 300s, configurable via `ZENTRA_LICENSE_REFRESH_SECS`).
> - Re-verify if file's `mtime` has changed since `last_mtime`, so operators can drop in a new license without restarting.
> - Inject into `AppState`; `enforce_license_middleware` reads from the cache.
> - Keep fail-closed: verification failure → 403.

**Tests.**
> Mutate a temp license file mid-test; cache picks up the new content within TTL.

---

### T14 — Declarative per-route feature gates

**Target:** 2026-07-01 (Wed) · **Closes:** G5 · **Severity:** 🟠

**Prompt.**
> Replace hardcoded path-prefix matching in `crates/zentra-api/src/license_middleware.rs:28-52` with a declarative per-route feature requirement. The extractor/layer must compile to a no-op when `prod-license` is disabled.
>
> Approach:
> - Define `enum RequiredFeature { Oracle, EbpfNetwork, AiObservability, Otel }` in `zentra-core` next to `LicenseFeatures` (or, after T10, on `LicenseTier`).
> - Create an Axum tower layer `RequireFeature(RequiredFeature)` that checks the cached license's flags and returns 403 if missing.
> - In each per-module router (`oracle::router()`, `cilium::router()`, etc.), attach `.layer(RequireFeature(RequiredFeature::Oracle))`.
> - The global `enforce_license_middleware` keeps responsibility only for "license file exists, signature valid, not expired"; feature gating moves to per-route layers.
> - Delete the prefix checks from the middleware.

**Tests.**
> Integration test: route layered with `RequireFeature(Oracle)` returns 403 when cached license has `enable_oracle: false`.

---

### T15 — Grace period & read-only fallback

**Target:** 2026-07-06 (Mon) · **Closes:** G6 · **Severity:** 🟠

**Prompt.**
> Currently every verification failure (expired, missing, parse error, signature failure) returns 403. Introduce graceful expiry handling, all behind `#[cfg(feature = "prod-license")]`.
>
> Behaviour:
> - Use `ExpiryStrategy::GracePeriod { grace_days }` from `license_policy.rs` (after T11). Add a `grace_period_days: u32` field to `LicensePayload` with `#[serde(default = "default_grace")]` returning 14.
> - In `verify_license_string`, return an enum `LicenseState { Valid, InGracePeriod { days_overdue }, Expired }` instead of `Result<Payload, String>`.
> - In the middleware (after T14):
>   - **Valid** → pass through.
>   - **InGracePeriod** → allow the request, attach response headers `X-Zentra-License-Status: grace-period` and `X-Zentra-License-Days-Overdue: N`, log a rate-limited warning.
>   - **Expired** → allow GETs to a read-only path allowlist (`/api/v1/license-status`, `/api/v1/dashboard`, health endpoints); everything else 403 with a JSON body explaining the state.
> - Update `get_license_status` in `crates/zentra-api/src/handlers/license_status.rs` to return the new state enum and `days_overdue` so the UI (T19) can show a banner.

**Tests.**
> Cover valid, grace (request allowed + headers present), fully expired (read allowed), fully expired (write blocked).

---

### T16 — Persistent audit log for license events

**Target:** 2026-07-07 (Tue) · **Closes:** G11 · **Severity:** 🟡

**Prompt.**
> Today verification failure only produces a `tracing::error!`. Wire outcomes into the existing audit infrastructure for forensic value.
>
> - Reuse the `license_audit_log` table from `migrations/20250101000003_license_management.sql` with `license_id = nil` and new `action` values: `runtime_license_verified`, `runtime_license_failed`, `runtime_license_grace_entered`, `runtime_license_expired`. Alternative: add a parallel `runtime_license_events` table — pick the cleaner option and document the choice.
> - Emit on: process startup verification, cache refresh (T13), verification failure, expiry transition, grace-period entry.
> - Include `customer_name`, `key_id` (T08), error reason, source path.
> - Add `GET /api/v1/license-status/events?limit=100` returning recent events for ops UI.
>
> Gated entirely under `prod-license`.

---

### T17 — License health metrics

**Target:** 2026-07-08 (Wed) · **Closes:** E5 · **Severity:** 🟡

**Prompt.**
> Emit runtime-entitlement metrics through the existing metric pipeline (mirror what `plugins/license-plugin` already does for the asset-management half):
> - `zentra_license_days_remaining` (gauge)
> - `zentra_license_state{state="valid|grace|expired"}` (gauge, 0/1)
> - `zentra_license_verification_failures_total` (counter)
> - `zentra_license_cache_age_seconds` (gauge, from T13)
>
> Push via the same path the license-plugin uses. Add a Grafana panel definition under `deploy/grafana/` if that's the convention.

---

### T18 — Single feature catalog (UI + server)

**Target:** 2026-07-13 (Mon) · **Closes:** G8 (server half) · **Severity:** 🟠

**Prompt.**
> Today `allowed_menu_items` is serialised inside `LicensePayload` and the server enforces a separate prefix list (T14 replaces with `RequireFeature`). These two lists can drift.
>
> Make both derive from one source:
> - Define a `FeatureCatalog` in `zentra-core` mapping each `RequiredFeature` (from T14) to (a) its license flag and (b) the set of UI menu item IDs it unlocks.
> - Remove `allowed_menu_items` from the persisted `LicensePayload`. Derive it in `get_license_status` from the catalog given the payload's feature flags.
> - Single edit to the catalog updates both UI visibility and route gating.
> - Bump `format_version` (T08). Add a back-compat path that ignores any `allowed_menu_items` field present in older files.

---

### T19 — Frontend: consume `/license-status` + grace banner

**Target:** 2026-07-15 (Wed) · **Closes:** G8 (UI half), E7 · **Severity:** 🟠

**Prompt.**
> The frontend (`frontend/`) currently does not consume `/api/v1/license-status` — `grep` finds only a stray reference in `types.ts`. The menu shows everything; gated routes 403 at click time.
>
> Wire it up:
> - Add a query (probably TanStack Query) to fetch `/api/v1/license-status` at app shell load + every 5 minutes.
> - Filter the navigation/menu using the `allowed_menu_items` array (derived server-side in T18).
> - When the response indicates `InGracePeriod` (T15), render a top-of-page banner: "Your Zentra license expired {N} days ago — contact sales to renew." with severity escalating after day 7.
> - When expired (read-only), banner says so and disables mutation controls (form submits, delete buttons) via a context flag.

---

### T20 — Offline license-signer CLI

**Target:** 2026-07-20 (Mon) · **Closes:** G10 · **Severity:** 🟡

**Prompt.**
> Create a small CLI crate at `crates/zentra-license-signer/`. This crate must NOT depend on `zentra-core`'s `prod-license` feature — it produces files that the prod build will verify, so it builds in any context.
>
> Commands:
> - `zentra-license-signer keygen --out-priv prod.key --out-pub prod.pub` → Ed25519 keypair.
> - `zentra-license-signer sign --priv prod.key --key-id zentra-prod-2026-01 --customer "Acme Corp" --org-id <uuid> --trial-days 365 --max-targets 500 --features oracle,otel --grace-days 14 --out license.json` → signed file matching the T08 envelope.
> - `zentra-license-signer verify --pub prod.pub license.json` → diagnostic verification.
>
> Must use the same canonicalisation contract as T06. Add an integration test that signs with the CLI and verifies via `zentra_core::crypto::license_verifier::verify_license_string`.

---

### T21 — Operator documentation

**Target:** 2026-07-22 (Wed) · **Closes:** E8 · **Severity:** 🟡

**Prompt.**
> Write three docs in `docs/licensing/`:
> 1. `ARCHITECTURE.md` — extend the T04 decision doc into a full architecture overview (envelope, verification flow, cache, feature catalog, audit log).
> 2. `SIGNING.md` — operator workflow for issuing a license: key generation, vault storage, signing command, customer hand-off, rotation cadence (recommend annual key_id rotation with a 6-month overlap).
> 3. `OPERATOR_RUNBOOK.md` — what to do when: customer reports 403, license about to expire, mid-rotation key conflict, audit log shows tamper attempt.
>
> Each doc ≤ 2 pages. Include concrete commands where applicable.

---

### T22 — Repo cleanup

**Target:** 2026-07-22 (Wed) · **Closes:** G15 · **Severity:** 🟢

**Prompt.**
> Sweep root-level scratch logs into `.gitignore` and delete from the repo:
> - `agent_check.log`, `agent_errors.txt`, `api_check.log`, `api_check.txt`, `api_check_clean.txt`, `api_errors.txt`
> - `build_diagnostics.txt`, `build_errors.txt`, `check_db_removal.txt`, `check_output.txt`
> - `errors.txt`, `errors_only.txt`, `final_check.txt`, `last_check.txt`, `last_errors.txt`
> - `full_cargo_check*.txt`, `full_check.txt`
> - `server_stderr.log`, `server_stdout.log`
>
> Add to `.gitignore`:
> ```
> /*.log
> /*_check.txt
> /*_errors.txt
> /errors*.txt
> /full_*.txt
> ```
> Confirm no scripts depend on these files before deleting.

---

## 6. Out of scope (tracked for later)

- **E2 — License server / phone-home.** Significant design work. Park.
- **E3 — Machine fingerprinting enforcement.** Struct exists in `license_policy.rs`; binding adds support burden (hardware swaps, VMs). Decide post-rollout.
- **E4 — Per-tenant runtime entitlement.** Park until multi-tenant deployment is a concrete customer ask.

---

## 7. Sequencing rules

- **T01 must land first.** Everything else assumes cfg-gating exists.
- **T05 → T06 → T08:** crypto chain in order.
- **T04 → T10–T12:** consolidation depends on the decision spike.
- **T14 → T18:** declarative gating must exist before catalog derivation makes sense.
- **T08 → T20:** envelope must be locked before the signer can target it.
- **T15 → T19:** banner depends on grace-state API.

---

## 8. Risk register

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Existing customers have HMAC-format licenses in the wild | Unknown — confirm before T05 | High (flag day) | T08 envelope supports `format_version`; keep HMAC verification behind a deprecation flag for one release, then remove |
| `prod-license` wired transitively breaks unrelated builds | Medium | Medium | T02 CI matrix catches early |
| Signing-tool private key mismanaged | High | Critical | T21 prescribes vault storage; T20 never writes private key to disk by default |
| Frontend changes block on backend (T19) | Medium | Low | T19 is the last user-visible step; backend can ship first with no regression (UI is unaware today) |

---

## 9. Tracking

- **Update cadence:** weekly, end-of-Friday. Update Status column below.
- **PR titles:** must reference `T##` for the task they close.
- **Sign-off gates:** Phase 1 exit (crypto) requires security review. Phase 6 exit requires ops review.

### Status board

| Task | Target | Status |
|---|---|---|
| T01 cfg-gate + remove APP_ENV | 2026-05-29 | ☐ |
| T02 CI matrix | 2026-06-01 | ☐ |
| T03 release-prod target | 2026-06-02 | ☐ |
| T04 policy-engine decision spike | 2026-06-03 | ☐ |
| T05 Ed25519 verification | 2026-06-09 | ☐ |
| T06 canonical-byte signing | 2026-06-11 | ☐ |
| T07 crypto negative tests | 2026-06-12 | ☐ |
| T08 versioned envelope | 2026-06-16 | ☐ |
| T09 lock LICENSE_PATH in prod | 2026-06-17 | ☐ |
| T10 consolidate signature path | 2026-06-22 | ☐ |
| T11 plugin uses ExpiryStrategy | 2026-06-23 | ☐ |
| T12 cross-module consistency tests | 2026-06-24 | ☐ |
| T13 verification cache | 2026-06-29 | ☐ |
| T14 declarative route gates | 2026-07-01 | ☐ |
| T15 grace period + read-only | 2026-07-06 | ☐ |
| T16 audit log | 2026-07-07 | ☐ |
| T17 health metrics | 2026-07-08 | ☐ |
| T18 single feature catalog | 2026-07-13 | ☐ |
| T19 frontend integration | 2026-07-15 | ☐ |
| T20 signer CLI | 2026-07-20 | ☐ |
| T21 operator docs | 2026-07-22 | ☐ |
| T22 repo cleanup | 2026-07-22 | ☐ |

Status legend: ☐ not started · ◐ in progress · ✅ done · ⛔ blocked
