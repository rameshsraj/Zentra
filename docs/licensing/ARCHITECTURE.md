# Zentra Licensing Architecture — Decision Document (T04)

## 1. Decision
We adopt `crates/zentra-core/src/models/license_policy.rs` as the canonical policy and entitlement engine of Zentra, consolidating all licensing check paths and removing all ad-hoc duplication across the repository.

## 2. Rationale
The `license_policy.rs` module is a robust and cleanly designed policy engine that implements structured strategies for license expiry (`ExpiryStrategy`), renewal (`RenewalStrategy`), overages, tiers, and capabilities. Maintaining separate, independent verifier loops in `license_verifier.rs` and warnings in `license-plugin` leads to severe code drift and duplicate signature/validation code. Adopting this engine ensures we have a single, canonical, highly robust implementation of entitlement checking throughout the entire platform.

## 3. Inventory of duplicates & consumers to change

- **Verification implementation**:
  - `crates/zentra-core/src/crypto/license_verifier.rs:59-99` (Ad-hoc signature verifier using HMAC and inline timestamp comparisons; needs migration to Ed25519 in T05 and integration with `license_policy.rs` types).
  - `crates/zentra-core/src/models/license_policy.rs:815-900` (Dead HMAC generation and validation pipeline; must align with Ed25519 verification).

- **Expiry bands and warnings**:
  - `plugins/license-plugin/src/lib.rs:161-169` (Ad-hoc warning threshold day calculations; must be replaced with `ExpiryStrategy::is_usable` from `license_policy.rs` in T11).

## 4. Migration sequence

1. **Establish Foundation (T01-T03) [Completed]**: Feature-gate licensing with `prod-license` cargo feature and clean up env var checks.
2. **Asymmetric Cryptography (T05-T09)**: Replace HMAC with Ed25519 verification inside the `prod-license` block, canonicalizing byte signatures.
3. **Consolidation Spike (T10)**: Align the policy module structure with the new verifier envelope; map `LicenseFeatures` to a unified catalog and eliminate the redundant HMAC validation in `license_policy.rs` to delete ≥300 lines of dead duplicate code.
4. **Plugin Migration (T11)**: Update `license-plugin` to call directly into `ExpiryStrategy::is_usable` from `license_policy.rs`.
5. **Route & UI integration (T13-T19)**: Inject the compiled features and route gates dynamically.
