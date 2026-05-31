# Zentra — Secure Docker Image: TODO Prompts

**Document date:** 2026-05-29
**Owner:** Zentra Team
**Scope:** Build a hardened, license-protected Docker image with embedded configuration, tamper detection, 3-day expiring trial license with graceful degradation, and runtime self-lockdown.
**Existing files:** `docker/Dockerfile`, `docker/docker-entrypoint.sh`, `docker/supervisord.conf`, `docker/nginx.conf`, `docker/install.sh`, `docker/install.ps1`, `docker/README.md`

---

## Current State

The `docker/` folder already contains a working Docker image setup:
- **Dockerfile** — Rocky Linux 8 + PG 16 + TimescaleDB + Nginx + Supervisor + Oracle Instant Client + e2fsprogs (chattr)
- **docker-entrypoint.sh** — PostgreSQL init, SSH key generation, 3-day signed license with graceful degradation, D09 self-lockdown (immutable files, shell removal)
- **supervisord.conf** — Manages postgresql, zentra-server, nginx
- **nginx.conf** — Frontend on :5010, reverse proxy `/api/*` to :5006, rate limiting, security headers
- **install.sh / install.ps1** — Automated setup scripts
- **README.md** — Comprehensive installation guide with hardened docker run commands

**Status:** All D01–D11 prompts are **implemented**. The image is hardened for production distribution with immutable config/license files, shell removal after startup, binary-verified license enforcement, and graceful degradation after trial expiry.

---

## D01 — Document Docker Image Creation Process

**Priority:** 🟡 Medium · **Depends on:** D02–D06 (write docs after implementation)

**Prompt.**
> Update `docker/README.md` to add a new section **"Building the Docker Image"** covering:
>
> 1. **Prerequisites on the build server** (`10.88.11.162`):
>    - Rust toolchain (`rustc 1.95+`, `cargo`)
>    - Node.js 18+ and npm (for frontend build)
>    - Docker installed and running
>    - Network access to pull base image `rockylinux:8` and RPM repos
>
> 2. **Build steps** (step-by-step commands):
>    ```bash
>    # 1. Build frontend
>    cd /store/Zentra/frontend && npm ci && npx vite build
>
>    # 2. Build backend (release)
>    cd /store/Zentra && cargo build --release --bin zentra-server
>
>    # 3. Build Docker image
>    docker build -f docker/Dockerfile -t zentra:latest .
>
>    # 4. Export image as tarball
>    docker save zentra:latest | gzip > docker/zentra-latest.tar.gz
>    ```
>
> 3. **Validation commands** after build:
>    ```bash
>    docker run --rm zentra:latest cat /opt/zentra/.build-info
>    docker run --rm zentra:latest ls -la /usr/local/bin/zentra-server
>    docker run --rm zentra:latest ls /opt/zentra/frontend/dist/index.html
>    ```
>
> 4. **Environment-specific examples:**
>    - Local dev: `docker run -d -p 5010:5010 zentra:latest`
>    - Production: full command with `--env-file`, named volume, `--restart unless-stopped`
>    - Air-gapped: `docker load -i zentra-latest.tar.gz`
>
> **Done when:** README has a complete "Building" section a new engineer can follow cold.

---

## D02 — Embed Full Application Setup into Docker Image

**Priority:** 🔴 High · **Depends on:** None

**Prompt.**
> ONLY the BUILD should be part of the docker, do not copy the Sourcecode. 

> Ensure the Docker image is fully self-contained and can run independently after `docker run` with zero external dependencies beyond environment variables.
>
> **Current state:** `docker/Dockerfile` already copies binary, migrations, frontend dist, and config files. `docker-entrypoint.sh` initializes PostgreSQL, creates the database, enables TimescaleDB, and runs migrations via the API server at startup.
>
> **Tweaks needed:**
>
> 1. **Add a build-info file** — In the Dockerfile, after copying the binary, write build metadata:
>    ```dockerfile
>    RUN echo "build_date=$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /opt/zentra/.build-info && \
>        echo "image_version=1.0.0" >> /opt/zentra/.build-info
>    ```
>
> 2. **Embed default `.env` values** — The entrypoint already sets defaults for `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `POSTGRES_PORT`, `DATABASE_URL`, `ZENTRA_API_PORT`. Verify all required env vars have sane defaults so the container starts with zero configuration:
>    ```bash
>    docker run -d --name zentra-test -p 5010:5010 zentra:latest
>    # Must start successfully with auto-generated password + JWT secret
>    ```
>
> 3. **Auto-generate JWT secret if not provided** — In `docker-entrypoint.sh`, if `ZENTRA_JWT_SECRET` is empty, generate one:
>    ```bash
>    if [ -z "${ZENTRA_JWT_SECRET}" ]; then
>        export ZENTRA_JWT_SECRET=$(head -c 32 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 48)
>        echo "[entrypoint] ZENTRA_JWT_SECRET not set - auto-generated for this session."
>    fi
>    ```
>
> 4. **Validate startup end-to-end** — After the container starts, confirm:
>    - `curl http://localhost:5010/api/health` returns `{"status":"healthy"}`
>    - `curl http://localhost:5010` returns the React SPA
>    - `docker exec zentra supervisorctl status` shows all 3 services RUNNING
>    - Login with `admin@zentra.local` / `Admin@12345` works
>
> **Done when:** `docker run -d -p 5010:5010 zentra:latest` works end-to-end with zero env vars passed.

---

## D03 — Package Build Artifacts Only (No Source Code)

**Priority:** 🔴 High · **Depends on:** None

**Prompt.**
> Verify and enforce that the Docker image contains ONLY runtime artifacts. No source code, development files, git history, or build tooling.
>
> **Current state:** The Dockerfile already uses selective `COPY` (binary, migrations, frontend dist, config). The `.dockerignore` file exists at project root.
>
> **Tweaks needed:**
>
> 1. **Audit the `.dockerignore`** — Ensure it excludes:
>    ```
>    .git/
>    .github/
>    crates/
>    frontend/src/
>    frontend/node_modules/
>    frontend/package.json
>    frontend/vite.config.*
>    target/debug/
>    target/release/deps/
>    target/release/build/
>    target/release/incremental/
>    target/release/.fingerprint/
>    *.rs
>    *.toml
>    *.lock
>    *.md
>    *.txt
>    *.log
>    docs/
>    tools/
>    plugins/
>    deploy/
>    ```
>
> 2. **Add a verification step in the Dockerfile** — At the end, before the ENTRYPOINT, add:
>    ```dockerfile
>    # Verify no source code leaked into the image
>    RUN test ! -d /opt/zentra/crates && \
>        test ! -d /opt/zentra/.git && \
>        test ! -f /opt/zentra/Cargo.toml && \
>        test ! -d /opt/zentra/frontend/src && \
>        test ! -d /opt/zentra/frontend/node_modules && \
>        echo "Source-code verification passed"
>    ```
>
> 3. **Verify after build** — Run this check against the built image:
>    ```bash
>    docker run --rm zentra:latest sh -c '
>      find / -name "*.rs" -o -name "*.toml" -o -name "Cargo.lock" \
>             -o -name ".git" -o -name "node_modules" 2>/dev/null | head -20
>    '
>    # Must return empty
>    ```
>
> 4. **Strip the binary** — Add to Dockerfile after COPY:
>    ```dockerfile
>    RUN strip /usr/local/bin/zentra-server 2>/dev/null || true
>    ```
>
> **Done when:** `docker run --rm zentra:latest find / -name '*.rs' 2>/dev/null` returns nothing. Binary is stripped. No Cargo.toml, no .git, no node_modules.

---

## D04 — Restrict Access to Sensitive Docker Contents

**Priority:** 🔴 High · **Depends on:** D02

**Prompt.**
> Implement least-privilege access controls inside the container so that even if someone gets shell access, sensitive data is protected.
>
> **Tweaks needed:**
>
> 1. **Database access restriction** — In `docker-entrypoint.sh`, after creating the database, create a restricted application user:
>    ```sql
>    CREATE USER zentra_app WITH PASSWORD '${POSTGRES_PASSWORD}';
>    GRANT CONNECT ON DATABASE zentra TO zentra_app;
>    GRANT USAGE ON SCHEMA public TO zentra_app;
>    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO zentra_app;
>    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO zentra_app;
>    GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO zentra_app;
>    ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE ON SEQUENCES TO zentra_app;
>    ```
>    The `DATABASE_URL` used by `zentra-server` should use `zentra_app`, not `postgres`.
>
> 2. **File permissions** — In the Dockerfile:
>    ```dockerfile
>    RUN chmod 500 /usr/local/bin/zentra-server && \
>        chmod 400 /opt/zentra/migrations/*.sql && \
>        chmod 700 /opt/zentra/logs && \
>        chmod 600 /etc/supervisord.conf && \
>        chmod 600 /etc/nginx/conf.d/zentra.conf
>    ```
>
> 3. **PostgreSQL `pg_hba.conf` hardening** — Already restricts to localhost. Add comment confirming no external access:
>    ```
>    # No external DB access — PostgreSQL is internal-only
>    ```
>
> 4. **Remove psql for non-postgres users** — Don't remove psql entirely (needed for migrations), but restrict:
>    ```dockerfile
>    RUN chmod 750 /usr/pgsql-16/bin/psql
>    ```
>
> **Done when:** `zentra-server` connects as `zentra_app` (not superuser). SQL files are read-only. Sensitive configs are permission-locked.

---

## D05 — Expose Frontend URL Only

**Priority:** 🔴 High · **Depends on:** None

**Prompt.**
> Ensure only port 5010 (Nginx frontend + API proxy) is externally accessible. No internal ports, no database, no admin panels.
>
> **Current state:** Dockerfile has `EXPOSE 5010 5006`. Nginx proxies `/api/*` to :5006 and blocks sensitive file extensions.
>
> **Tweaks needed:**
>
> 1. **Remove port 5006 from EXPOSE** — Only 5010 should be exposed:
>    ```dockerfile
>    EXPOSE 5010
>    ```
>    Port 5006 (backend API) is internal — Nginx proxies to it on 127.0.0.1.
>
> 2. **Bind zentra-server to 127.0.0.1 only** — In `supervisord.conf` or the server binary config, ensure the API binds to `127.0.0.1:5006` not `0.0.0.0:5006`. Check if the binary respects a `ZENTRA_BIND_ADDR` env var; if not, add one. This prevents direct access even if someone maps port 5006.
>
> 3. **Block internal endpoints in Nginx** — Add to `nginx.conf`:
>    ```nginx
>    # Block internal/debug endpoints from external access
>    location /api/system/ {
>        allow 127.0.0.1;
>        deny all;
>    }
>    location /api/pool-status {
>        allow 127.0.0.1;
>        deny all;
>    }
>    ```
>
> 4. **Ensure PostgreSQL binds to 127.0.0.1 only** — Already the case via `pg_hba.conf`, but also add to `postgresql.conf`:
>    ```
>    listen_addresses = '127.0.0.1'
>    ```
>
> 5. **Validate** — After running the container with `-p 5010:5010`:
>    ```bash
>    curl http://localhost:5010          # ✓ works (frontend)
>    curl http://localhost:5010/api/health  # ✓ works (proxied)
>    curl http://localhost:5006/api/health  # ✗ connection refused (not exposed)
>    psql -h localhost -p 5434            # ✗ connection refused (not exposed)
>    ```
>
> **Done when:** Only `-p 5010:5010` is needed. Direct access to :5006 and :5434 is impossible from outside the container.

---

## D06 — Harden Docker Image Security

**Priority:** 🔴 High · **Depends on:** D03, D04, D05

**Prompt.**
> Remove unnecessary packages, disable debug access, minimize attack surface.
>
> **Tweaks needed:**
>
> 1. **Remove unnecessary packages after install** — In the Dockerfile, after all setup, add a cleanup layer:
>    ```dockerfile
>    RUN dnf remove -y gcc make cpp && \
>        dnf clean all && \
>        rm -rf /var/cache/dnf /tmp/* /var/tmp/*
>    ```
>
> 2. **Remove shell access from non-root users** — Create a dedicated `zentra` runtime user:
>    ```dockerfile
>    RUN useradd -r -s /sbin/nologin zentra
>    ```
>    Note: The container still needs root for supervisord to manage PostgreSQL (which runs as `postgres`). But the zentra-server process should run as the `zentra` user. Update `supervisord.conf`:
>    ```ini
>    [program:zentra-server]
>    user=zentra
>    ```
>
> 3. **Disable core dumps**:
>    ```dockerfile
>    RUN echo '* hard core 0' >> /etc/security/limits.conf
>    ```
>
> 4. **Remove unnecessary SUID/SGID binaries**:
>    ```dockerfile
>    RUN find / -perm /6000 -type f -exec chmod a-s {} \; 2>/dev/null || true
>    ```
>
> 5. **Add `--read-only` support** — Document that the container can be run with `--read-only --tmpfs /tmp --tmpfs /run` for extra security (PostgreSQL volume is mounted writable).
>
> 6. **Remove bash history and temp files in final layer**:
>    ```dockerfile
>    RUN rm -rf /root/.bash_history /root/.cache /tmp/* /var/tmp/* && \
>        history -c 2>/dev/null || true
>    ```
>
> **Done when:** `docker run --rm zentra:latest rpm -qa | wc -l` is minimal. No compiler, no debug tools. zentra-server runs as non-root user.

---

## D07 — Implement Docker Tamper Protection

**Priority:** 🔴 High · **Depends on:** D03, D06

**Prompt.**
> Add integrity verification so that if the Docker image or its protected contents are modified, the application refuses to start.
>
> **Implementation:**
>
> 1. **Build-time: generate integrity manifest** — At the END of the Dockerfile (last RUN before ENTRYPOINT), compute checksums of all critical files:
>    ```dockerfile
>    RUN sha256sum \
>        /usr/local/bin/zentra-server \
>        /opt/zentra/frontend/dist/index.html \
>        /etc/nginx/conf.d/zentra.conf \
>        /etc/supervisord.conf \
>        /usr/local/bin/docker-entrypoint.sh \
>        > /opt/zentra/.integrity && \
>        chmod 400 /opt/zentra/.integrity
>    ```
>
> 2. **Runtime: verify on startup** — In `docker-entrypoint.sh`, BEFORE the license check, add:
>    ```bash
>    # Tamper detection
>    INTEGRITY_FILE="/opt/zentra/.integrity"
>    if [ -f "${INTEGRITY_FILE}" ]; then
>        if ! sha256sum -c "${INTEGRITY_FILE}" --quiet 2>/dev/null; then
>            echo "[entrypoint] FATAL: Integrity verification failed. Image may be tampered."
>            echo "[entrypoint] Application cannot start. Please use an official image."
>            exit 1
>        fi
>    else
>        echo "[entrypoint] FATAL: Integrity manifest missing."
>        exit 1
>    fi
>    ```
>
> 3. **Protect the manifest itself** — Embed a secondary checksum of `.integrity` inside the binary or as a Docker LABEL:
>    ```dockerfile
>    RUN MANIFEST_HASH=$(sha256sum /opt/zentra/.integrity | cut -d' ' -f1) && \
>        echo "${MANIFEST_HASH}" > /opt/zentra/.manifest-hash && \
>        chmod 400 /opt/zentra/.manifest-hash
>    ```
>    In the entrypoint, verify the manifest hash too:
>    ```bash
>    EXPECTED_HASH=$(cat /opt/zentra/.manifest-hash)
>    ACTUAL_HASH=$(sha256sum /opt/zentra/.integrity | cut -d' ' -f1)
>    if [ "${EXPECTED_HASH}" != "${ACTUAL_HASH}" ]; then
>        echo "[entrypoint] FATAL: Integrity manifest has been modified."
>        exit 1
>    fi
>    ```
>
> 4. **On failure** — Log the error and `exit 1`. Supervisord won't start. Container goes to "exited" state.
>
> **Done when:** Modifying any protected file (e.g., `docker exec zentra bash -c 'echo x >> /usr/local/bin/zentra-server'`) causes the next startup to fail with an integrity error.

---

## D08 — Implement Expiring License Mechanism (3-Day Trial + Graceful Degradation)

**Priority:** 🔴 High · **Depends on:** D07
**Status:** ✅ IMPLEMENTED

**Prompt.**
> Build a robust license validation with a 3-day trial period and graceful degradation after expiry.
>
> **Implemented (differs from original 5-day hard-block spec):**
>
> 1. **3-day trial** — `LICENSE_DAYS=3` in `docker-entrypoint.sh`. After expiry, the container **still starts** but in degraded mode (Dashboard only, all other features disabled).
>
> 2. **Signed license token** — Same HMAC-signed format: `install_epoch:expiry_epoch:hostname:sha256_hmac`. Signature uses the binary's SHA-256 hash. Tampering triggers `exit 1`.
>
> 3. **Graceful degradation (not hard block)** — After expiry:
>    - Entrypoint sets `ZENTRA_TRIAL_EXPIRED=1` and logs a WARNING (no `exit 1`)
>    - Backend middleware returns `402 Payment Required` for all non-essential API routes
>    - Allowed routes in degraded mode: `/api/health`, `/api/auth/*`, `/api/system/license-status`, `/api/config/mode`, `/api/system/status`, `/api/system/db`, `/metrics`
>    - Frontend hides all nav items except "Command Center" (Dashboard) and shows an amber warning banner
>
> 4. **Binary-verified license (hardened)** — The Rust binary (`license_verifier.rs`) independently reads the license file from disk, computes its own SHA-256 hash, verifies the HMAC, and checks the expiry epoch. It does **NOT** trust the `ZENTRA_TRIAL_EXPIRED` env var in Docker environments. Attack vectors blocked:
>    - Spoofing env var via `docker run -e ZENTRA_TRIAL_EXPIRED=0` → ignored
>    - Deleting license file when DB dir exists → treated as bypass attempt → expired
>    - Modifying expiry epoch → HMAC fails → expired
>    - Modifying binary → changes hash → HMAC fails → expired
>    - Result cached in `OnceLock` (one disk read per process lifetime)
>
> 5. **License persisted in PG volume** — Stored at `/var/lib/postgresql/data/.zentra-license` (survives container recreation). Fallback at `/opt/zentra/.license`.
>
> 6. **License status API** — `GET /api/system/license-status` returns `trial_expired`, `days_remaining`, `features`, etc. Frontend polls this every 5 minutes.
>
> **Files modified:**
> - `docker/docker-entrypoint.sh` — LICENSE_DAYS=3, graceful startup on expiry
> - `crates/zentra-core/src/crypto/license_verifier.rs` — `is_trial_expired()` with disk-based verification
> - `crates/zentra-api/src/license_middleware.rs` — Trial expiry check + degraded mode routing
> - `crates/zentra-api/src/handlers/license_status.rs` — `trial_expired` flag in response
> - `crates/zentra-api/src/routes/mod.rs` — License middleware always applied (not just prod-license)
> - `frontend/src/lib/api/health.ts` — `LicenseStatus` type + `getLicenseStatus()`
> - `frontend/src/components/Layout.tsx` — Nav filtering + warning banner when expired
>
> **Done when:** Container runs in degraded mode after 3 days. Dashboard is viewable. All other features return 402. License cannot be spoofed via env var, file deletion, or file modification.

---

## D09 — Testing and Validation

**Priority:** 🟡 Medium · **Depends on:** D01–D08
**Status:** ✅ IMPLEMENTED

**Prompt.**
> Create a comprehensive validation script `docker/validate.sh` that tests all security and functionality requirements.
>
> **Done:** `docker/validate.sh` exists and tests image presence, no source code, build artifacts, security hardening, container lifecycle, health endpoints, and license mechanism.

---

## D10 — Update Deployment Notes & Download Image Locally

**Priority:** 🟡 Medium · **Depends on:** D01–D09
**Status:** ✅ IMPLEMENTED

> Updated `docker/README.md` with hardened `docker run` commands (`--read-only`, `--cap-drop ALL`, `--security-opt=no-new-privileges`), full security table (20+ measures), and operational docs.

---

## D11 — Container Self-Lockdown (Runtime Hardening)

**Priority:** 🔴 High · **Depends on:** D06, D07, D08
**Status:** ✅ IMPLEMENTED

**Prompt.**
> After the entrypoint completes all initialization (PostgreSQL setup, license check, SSH keys), lock down the container so that no one can modify config files, license files, or get interactive access — even with root.
>
> **Implemented in `docker-entrypoint.sh` (D09 self-lockdown phase):**
>
> 1. **Immutable license files** — Both `/var/lib/postgresql/data/.zentra-license` and `/opt/zentra/.license` are locked with `chattr +i`. Even root cannot modify or delete them.
>
> 2. **Immutable config files** — `supervisord.conf`, `nginx.conf`, `zentra.conf`, `.integrity`, `.manifest-hash`, `.build-info`, `zentra-server` binary, and `docker-entrypoint.sh` itself are all locked with `chattr +i`.
>
> 3. **Immutable SQL migrations** — All `.sql` files under `/opt/zentra/migrations/` locked with `chattr +i`.
>
> 4. **Immutable frontend dist** — All files under `/opt/zentra/frontend/dist/` locked with `chattr +i`.
>
> 5. **Package managers removed** — `dnf`, `dnf-3`, `yum`, `yum-config-manager`, `rpm`, `rpm2cpio`, `rpmkeys` all deleted. `/etc/yum.repos.d` and `/var/cache/dnf` removed. No software can be installed at runtime.
>
> 6. **Editors and scripting languages removed** — `vi`, `vim`, `nano`, `ed`, `perl`, `lua`, `ruby` deleted. Python3 is **kept** (supervisord depends on it).
>
> 7. **Dangerous utilities removed** — `wget`, `ftp`, `telnet`, `gcc`, `cc`, `make`, `gdb`, `strace`, `ltrace`, `dd`, `strings` all deleted.
>
> 8. **`chattr` self-destructs** — After making all files immutable, `chattr` and `lsattr` are deleted. The immutable bits can **never** be reversed.
>
> 9. **All shells removed** — A background task removes `bash`, `sh`, `csh`, `tcsh`, `zsh`, `ksh`, `fish`, `dash` 5 seconds after `exec supervisord`. After this, `docker exec` is useless — there's no shell to exec into.
>
> 10. **Preserved tools** — `sshpass`, `ssh`, `scp` (monitoring targets), `curl` (healthcheck), `python3` (supervisord).
>
> **Bypass protection via `ZENTRA_SKIP_LOCKDOWN=1`** — Disables the lockdown for debugging. Must NEVER be used in production.
>
> **Dockerfile additions:**
> - `e2fsprogs` installed (provides `chattr`)
> - Editors/scripting languages also removed at build time
> - Entrypoint permissions set to `500` (read+execute only, root-owned)
> - Recommended runtime flags documented: `--read-only`, `--cap-drop ALL`, `--security-opt=no-new-privileges`
>
> **Files modified:**
> - `docker/docker-entrypoint.sh` — D09 self-lockdown phase (lines 290–370)
> - `docker/Dockerfile` — e2fsprogs, build-time cleanup, entrypoint permissions, security flag docs
> - `docker/README.md` — Full security table (20+ measures), hardened docker run commands
>
> **Done when:** After startup, `docker exec -it zentra bash` fails (no shell). Config/license files cannot be modified. No package managers or editors exist.

---

## Execution Order

```
D02 (embed setup) ──┐
D03 (artifacts only) ├──► D04 (restrict access) ──► D06 (harden) ──► D07 (tamper) ──► D08 (license + degradation)
D05 (frontend only) ─┘                                                                     │
                                                                                            ▼
                                                                   D11 (self-lockdown) ──► D09 (validate) ──► D01 (docs) ──► D10 (deploy)
```

**Critical path:** D02 → D04 → D06 → D07 → D08 → D11 → D09 → D10

**All items D01–D11 are ✅ IMPLEMENTED.**
