# Zentra Build & Run — DEV vs PROD

**Document date:** 2026-05-27
**Audience:** developers, release engineers, ops.
**Related docs:** [LICENSING_HOW_IT_WORKS.md](LICENSING_HOW_IT_WORKS.md), [LICENSING_PROMPTS.md](LICENSING_PROMPTS.md), [LICENSING_DIAGRAMS.md](LICENSING_DIAGRAMS.md)

---

## 1. Where builds happen

**All builds run on the build server, not on developer laptops.**

- Developers write code locally and push to the repo.
- The build server is the single source of compiled binaries — both DEV and PROD artifacts.
- Developers do not produce releasable binaries. Local `cargo build` is for tests, debugging, and quick iteration only — never for distribution.
- This keeps the toolchain pinned, the cargo cache shared, and (post-T01) the `prod-license` feature flag managed from one place where the signing public key lives.

Why this matters: the `prod-license` cargo feature embeds the Ed25519 public key (after T05) into the binary. Letting that compilation happen on uncontrolled machines defeats the point. Centralising builds on the server is a precondition for the licensing system being trustworthy.

---

## 2. The DEV / PROD switch

| Mode | Today (current code) | After T01 (post-2026-05-29) |
|---|---|---|
| **DEV** | `cargo build` with `APP_ENV` unset | `cargo build` (no feature flags) |
| **PROD** | `APP_ENV=production` at runtime **or** undeclared `prod-license` feature | `cargo build --features zentra-core/prod-license,zentra-api/prod-license` |

After T01 the env-var path is removed entirely — the cargo feature is the only switch, and it is compile-time so it cannot be flipped after the fact.

---

## 3. DEV builds (server)

### Command

```bash
cargo build -p zentra-api -p zentra-agent
```

### Properties of a DEV binary

- Licensing middleware compiles in but is a no-op at runtime (today: because `APP_ENV` is unset; after T01: because the `#[cfg(feature = "prod-license")]` block is excluded).
- `GET /api/v1/license-status` returns the mock "Development Sandbox" payload (9999 days, all features on).
- No `license.json` is required anywhere on disk.
- `ZENTRA_LICENSE_PATH` is ignored.
- Suitable for: developer testing, CI test runs, integration test environments, demo sandboxes.

### Distribution

DEV binaries should be tagged in the artifact store with a clearly non-production label:
```
zentra-api-dev-<commit-sha>
zentra-agent-dev-<commit-sha>
```
They must never be promoted to a production channel.

---

## 4. PROD builds (server)

### Command (today — placeholder, not yet wired)

```bash
APP_ENV=production cargo build --release -p zentra-api -p zentra-agent
```

This is fragile (env-var bypassable at runtime). Use only until T01 lands.

### Command (after T01)

```bash
cargo build --release \
    --features zentra-core/prod-license,zentra-api/prod-license \
    -p zentra-api -p zentra-agent
```

### Properties of a PROD binary

- Licensing middleware actively reads `/etc/zentra/license.json` (Linux) or `C:\ProgramData\Zentra\license.json` (Windows).
- Every request goes through signature verification (today HMAC, after T05 Ed25519) and feature-gate checks.
- Missing/expired/tampered license → 403.
- `GET /api/v1/license-status` returns the real entitlement state.
- After T09: `ZENTRA_LICENSE_PATH` env override is locked out.
- Suitable for: customer deployments only.

### Distribution

PROD binaries are tagged:
```
zentra-api-prod-<semver>-<commit-sha>
zentra-agent-prod-<semver>-<commit-sha>
```

Each PROD release must be signed/checksummed by the build server before being made available for download.

---

## 5. Required build-server setup

| Item | Notes |
|---|---|
| Rust toolchain | Pinned via `rust-toolchain.toml` in repo (add this if not present — currently the repo relies on whatever rustc the builder happens to have) |
| `cargo` cache | Mount a persistent volume; clean only on toolchain bumps |
| PROD signing key access | Path to Ed25519 **private** key for the signer CLI (T20). Stored in vault, fetched at build time, never written to disk on the build host (use `--priv -` to stream). The corresponding **public** key is what's embedded in the PROD binary. |
| Build target matrix | At minimum: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`. Add aarch64 if customers need it. |
| Environment isolation | DEV builds and PROD builds must run in separate jobs / containers. Do not let an `APP_ENV=production` env leak from a parent shell into a DEV build job. |

---

## 6. CI / build pipeline definition

Add (or update) the CI workflow to express the matrix explicitly. Example for a GitHub Actions-style runner:

```yaml
jobs:
  dev-build:
    runs-on: build-server
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --workspace --no-default-features
      - run: cargo test --workspace --no-default-features
      - name: Upload DEV artifacts
        uses: actions/upload-artifact@v4
        with:
          name: zentra-dev-${{ github.sha }}
          path: target/debug/zentra-{api,agent}

  prod-build:
    runs-on: build-server
    needs: dev-build
    if: startsWith(github.ref, 'refs/tags/v')
    steps:
      - uses: actions/checkout@v4
      - name: Fetch signing public key
        run: vault kv get -field=pub zentra/license/prod > /tmp/license.pub
      - run: |
          cargo build --release \
            --features zentra-core/prod-license,zentra-api/prod-license \
            -p zentra-api -p zentra-agent
      - run: cargo test --release --features zentra-core/prod-license
      - name: Sign artifacts
        run: |
          cosign sign-blob target/release/zentra-api > zentra-api.sig
          cosign sign-blob target/release/zentra-agent > zentra-agent.sig
      - name: Upload PROD artifacts
        uses: actions/upload-artifact@v4
        with:
          name: zentra-prod-${{ github.ref_name }}-${{ github.sha }}
          path: |
            target/release/zentra-api
            target/release/zentra-agent
            *.sig
```

Notes:
- The `prod-build` job runs only on tag pushes (`v1.2.3`-style), enforcing that PROD binaries always correspond to a released version.
- Both jobs run on the dedicated `build-server` runner — never on shared/cloud runners that might leak the signing key.

---

## 7. Local developer workflow (no builds for distribution)

Developers may run the code locally for testing only:

```bash
# Bash / zsh
unset APP_ENV
cargo run -p zentra-api

# PowerShell
Remove-Item Env:APP_ENV -ErrorAction SilentlyContinue
cargo run -p zentra-api
```

Confirm DEV mode is active:

```bash
curl http://localhost:8080/api/v1/license-status
# Expect: "customer_name": "Development Sandbox (Local)", "days_remaining": 9999
```

If the response is `"is_licensed": false` or you get a 403, `APP_ENV=production` is set somewhere — check your shell init files, VS Code workspace settings (`.vscode/settings.json` → `terminal.integrated.env.*`), and `.env*` files in the project.

**Local binaries must not be distributed.** If you need to share a build with a teammate or QA, push your branch and let the build server produce a DEV artifact instead.

---

## 8. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Every API call returns 403 in dev | `APP_ENV=production` is set | `unset APP_ENV` and restart |
| `license-status` says "Unlicensed Installation" | Same as above, plus no `license.json` present | Same fix as above (or, if you intended PROD, drop a signed license at the platform path) |
| PROD build runs but immediately 403s in production | Missing license file, wrong path, or tampered file | Check `/etc/zentra/license.json` exists, owned by the service user, and was issued by the signer CLI (T20) with the matching key |
| PROD build behaves like DEV (no enforcement) | Pre-T01: `APP_ENV` not set on the host. Post-T01: built without `--features prod-license` | Verify env (pre-T01) or rebuild with the feature flag (post-T01) |
| DEV binary in production by mistake | Promoted wrong artifact | Rebuild from the tag using the `prod-build` job; never promote a DEV artifact |

---

## 9. Migration checklist (T01 cut-over)

When T01 lands and the env-var path is removed, do this in order:

- [ ] Land T01 PR — declares `prod-license` feature, rewrites `check_license_active()`, removes `APP_ENV` branch.
- [ ] Update the build server's `prod-build` job to pass `--features zentra-core/prod-license,zentra-api/prod-license`.
- [ ] Update `dev-build` job to pass `--no-default-features` explicitly (defensive — catches accidental default-feature additions).
- [ ] Remove `APP_ENV=production` from any deployment configs, systemd units, container env files, k8s manifests. Search: `grep -rE "APP_ENV.*production" deploy/ docker/`.
- [ ] Cut a new PROD release; confirm an installation built without the feature flag refuses to enforce licensing (sanity check — there should be no way to be "stuck" in the old behaviour).
- [ ] Communicate the change to ops.

---

## 10. Summary

> **The build server is the only place builds happen.** DEV and PROD differ only in whether the `prod-license` cargo feature is enabled (today: also whether `APP_ENV=production` is set — going away in T01). Developers run code locally for testing but never produce distributable binaries. PROD builds run in a separate, isolated CI job triggered by version tags, with the signing public key fetched from a vault.
