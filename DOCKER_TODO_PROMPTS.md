# Zentra — Secure Docker Image: TODO Prompts

**Document date:** 2026-05-29
**Owner:** Zentra Team
**Scope:** Build a hardened, license-protected Docker image with embedded configuration, tamper detection, and 5-day expiring license.
**Existing files:** `docker/Dockerfile`, `docker/docker-entrypoint.sh`, `docker/supervisord.conf`, `docker/nginx.conf`, `docker/install.sh`, `docker/install.ps1`, `docker/README.md`

---

## Current State

The `docker/` folder already contains a working Docker image setup:
- **Dockerfile** — Rocky Linux 8 + PG 16 + TimescaleDB + Nginx + Supervisor + Oracle Instant Client
- **docker-entrypoint.sh** — PostgreSQL init, SSH key generation, basic 5-day license check (file-based epoch comparison)
- **supervisord.conf** — Manages postgresql, zentra-server, nginx
- **nginx.conf** — Frontend on :5010, reverse proxy `/api/*` to :5006, rate limiting, security headers
- **install.sh / install.ps1** — Automated setup scripts
- **README.md** — Comprehensive installation guide

**What needs to change:** Harden the image for production distribution — strip source, lock down internals, add tamper protection, improve the license mechanism, restrict all access to frontend-only, and download the final image locally.

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

## D08 — Implement Expiring License Mechanism

**Priority:** 🔴 High · **Depends on:** D07

**Prompt.**
> Build a robust license validation that expires after 5 days and cannot be trivially bypassed.
>
> **Current state:** `docker-entrypoint.sh` already has a basic 5-day check using `/opt/zentra/.install_date` (epoch file). This is easily bypassed by deleting or modifying the file.
>
> **Improved implementation:**
>
> 1. **Replace the simple epoch file with a signed token** — At first boot, generate a license token:
>    ```bash
>    INSTALL_EPOCH=$(date +%s)
>    EXPIRY_EPOCH=$((INSTALL_EPOCH + LICENSE_DAYS * 86400))
>    LICENSE_PAYLOAD="${INSTALL_EPOCH}:${EXPIRY_EPOCH}:$(hostname)"
>    # Sign with a key derived from the binary checksum + build date
>    BUILD_HASH=$(sha256sum /usr/local/bin/zentra-server | cut -d' ' -f1)
>    LICENSE_SIG=$(echo -n "${LICENSE_PAYLOAD}:${BUILD_HASH}" | sha256sum | cut -d' ' -f1)
>    echo "${LICENSE_PAYLOAD}:${LICENSE_SIG}" > /opt/zentra/.license
>    chmod 400 /opt/zentra/.license
>    ```
>
> 2. **On subsequent boots, verify the license token**:
>    ```bash
>    if [ -f /opt/zentra/.license ]; then
>        LICENSE_DATA=$(cat /opt/zentra/.license)
>        STORED_INSTALL=$(echo "$LICENSE_DATA" | cut -d: -f1)
>        STORED_EXPIRY=$(echo "$LICENSE_DATA" | cut -d: -f2)
>        STORED_HOST=$(echo "$LICENSE_DATA" | cut -d: -f3)
>        STORED_SIG=$(echo "$LICENSE_DATA" | cut -d: -f4)
>
>        # Verify signature
>        BUILD_HASH=$(sha256sum /usr/local/bin/zentra-server | cut -d' ' -f1)
>        EXPECTED_SIG=$(echo -n "${STORED_INSTALL}:${STORED_EXPIRY}:${STORED_HOST}:${BUILD_HASH}" | sha256sum | cut -d' ' -f1)
>
>        if [ "${STORED_SIG}" != "${EXPECTED_SIG}" ]; then
>            echo "[entrypoint] ERROR: License file corrupted. Please reinstall."
>            exit 1
>        fi
>
>        CURRENT_EPOCH=$(date +%s)
>        if [ "${CURRENT_EPOCH}" -ge "${STORED_EXPIRY}" ]; then
>            DAYS_EXPIRED=$(( (CURRENT_EPOCH - STORED_EXPIRY) / 86400 ))
>            echo "[entrypoint] ERROR: License expired ${DAYS_EXPIRED} day(s) ago."
>            echo "[entrypoint] Contact Zentra team for a new license."
>            exit 1
>        fi
>
>        DAYS_LEFT=$(( (STORED_EXPIRY - CURRENT_EPOCH) / 86400 ))
>        echo "[entrypoint] License valid — ${DAYS_LEFT} day(s) remaining."
>    else
>        # First boot — generate license
>        # (code from step 1 above)
>    fi
>    ```
>
> 3. **Runtime check in the API server** — Also add a periodic license check inside `zentra-server` (every 1 hour) that reads `/opt/zentra/.license` and shuts down gracefully if expired. This prevents someone from bypassing the entrypoint.
>
> 4. **Store install date in the database too** — On first boot, after migrations, insert:
>    ```sql
>    INSERT INTO system_settings (key, value) VALUES ('install_epoch', '${INSTALL_EPOCH}')
>    ON CONFLICT (key) DO NOTHING;
>    ```
>    The API server can cross-check this against the file.
>
> 5. **Anti-bypass: persist in the Docker volume** — Store `.license` inside the PG data volume (`/var/lib/postgresql/data/.zentra-license`), so it survives container recreation but cannot be bypassed by simply deleting the overlay layer.
>
> **Done when:** Container refuses to start after 5 days. Modifying or deleting the license file triggers "corrupted" error. Clock manipulation alone doesn't bypass (DB cross-check).

---

## D09 — Testing and Validation

**Priority:** 🟡 Medium · **Depends on:** D01–D08

**Prompt.**
> Create a comprehensive validation script `docker/validate.sh` that tests all security and functionality requirements.
>
> ```bash
> #!/bin/bash
> # docker/validate.sh — Post-build validation for Zentra Docker image
> set -e
>
> IMAGE="zentra:latest"
> CONTAINER="zentra-validate"
> PASS=0
> FAIL=0
>
> check() {
>     local desc="$1"; shift
>     if "$@" >/dev/null 2>&1; then
>         echo "  ✓ ${desc}"; ((PASS++))
>     else
>         echo "  ✗ ${desc}"; ((FAIL++))
>     fi
> }
>
> echo "=== Zentra Docker Image Validation ==="
> echo ""
>
> # 1. Image builds
> echo "[1] Image exists"
> check "Image present" docker image inspect ${IMAGE}
>
> # 2. No source code
> echo "[2] No source code in image"
> check "No .rs files"     docker run --rm ${IMAGE} sh -c '! find / -name "*.rs" 2>/dev/null | grep -q .'
> check "No Cargo.toml"    docker run --rm ${IMAGE} sh -c '! test -f /opt/zentra/Cargo.toml'
> check "No .git dir"      docker run --rm ${IMAGE} sh -c '! test -d /opt/zentra/.git'
> check "No node_modules"  docker run --rm ${IMAGE} sh -c '! find / -name node_modules -type d 2>/dev/null | grep -q .'
>
> # 3. Build artifacts present
> echo "[3] Build artifacts present"
> check "zentra-server binary"  docker run --rm ${IMAGE} test -x /usr/local/bin/zentra-server
> check "Frontend dist"         docker run --rm ${IMAGE} test -f /opt/zentra/frontend/dist/index.html
> check "Migrations"            docker run --rm ${IMAGE} test -d /opt/zentra/migrations
> check "Integrity manifest"    docker run --rm ${IMAGE} test -f /opt/zentra/.integrity
>
> # 4. Security
> echo "[4] Security hardening"
> check "Binary is stripped"    docker run --rm ${IMAGE} sh -c 'file /usr/local/bin/zentra-server | grep -q stripped'
> check "No gcc/make"           docker run --rm ${IMAGE} sh -c '! which gcc 2>/dev/null'
> check "Only port 5010 EXPOSE" docker image inspect ${IMAGE} --format='{{.Config.ExposedPorts}}' | grep -qv 5006
>
> # 5. Container starts
> echo "[5] Container lifecycle"
> docker rm -f ${CONTAINER} 2>/dev/null || true
> docker run -d --name ${CONTAINER} -p 15010:5010 ${IMAGE}
> sleep 15
> check "Container running"    docker ps | grep -q ${CONTAINER}
> check "Health endpoint"      curl -sf http://localhost:15010/api/health | grep -q healthy
> check "Frontend loads"       curl -sf http://localhost:15010/ | grep -q '<div id="root">'
>
> # 6. License
> echo "[6] License mechanism"
> check "License file created"  docker exec ${CONTAINER} test -f /opt/zentra/.license
>
> # Cleanup
> docker rm -f ${CONTAINER} 2>/dev/null || true
>
> echo ""
> echo "=== Results: ${PASS} passed, ${FAIL} failed ==="
> [ ${FAIL} -eq 0 ] && echo "ALL CHECKS PASSED" || echo "SOME CHECKS FAILED"
> exit ${FAIL}
> ```
>
> **Done when:** `bash docker/validate.sh` runs all checks and reports pass/fail. All checks pass on the final image.

---

## D10 — Update Deployment Notes & Download Image Locally

**Priority:** 🟡 Medium · **Depends on:** D01–D09

**Prompt.**
> Update operational documentation and download the final Docker image to the local `docker/` folder.
>
> 1. **Build the final image on the server** (`10.88.11.162`):
>    ```bash
>    ssh root@10.88.11.162
>    cd /store/Zentra
>
>    # Build frontend
>    cd frontend && npm ci && npx vite build && cd ..
>
>    # Build backend
>    cargo build --release --bin zentra-server
>
>    # Build Docker image
>    docker build -f docker/Dockerfile -t zentra:latest .
>
>    # Export as tarball
>    docker save zentra:latest | gzip > /store/Zentra/docker/zentra-latest.tar.gz
>    ls -lh /store/Zentra/docker/zentra-latest.tar.gz
>    ```
>
> 2. **Download image to local `docker/` folder**:
>    ```powershell
>    scp root@10.88.11.162:/store/Zentra/docker/zentra-latest.tar.gz C:\git\Zentra\docker\zentra-latest.tar.gz
>    ```
>
> 3. **Add operational guidance to README.md:**
>    - **Rebuilding the image** — step-by-step commands
>    - **Renewing/extending licenses** — how to issue a new image with reset license, or extend via env var
>    - **Security considerations** — list of hardening measures applied
>    - **Troubleshooting:**
>      - "License expired" → rebuild and redeploy
>      - "Integrity verification failed" → image was modified, use official tarball
>      - "Container won't start" → check `docker logs zentra`
>    - **Recovery procedures** — how to backup data, update image, restore
>
> 4. **Add `.gitignore` entry** for the tarball (it's too large for git):
>    ```
>    docker/zentra-latest.tar.gz
>    ```
>
> **Done when:** `zentra-latest.tar.gz` exists in `C:\git\Zentra\docker\`. README has full operational docs. Image loads and runs successfully on a clean Docker host.

---

## Execution Order

```
D02 (embed setup) ──┐
D03 (artifacts only) ├──► D04 (restrict access) ──► D06 (harden) ──► D07 (tamper) ──► D08 (license)
D05 (frontend only) ─┘                                                                     │
                                                                                            ▼
                                                                              D09 (validate) ──► D01 (docs) ──► D10 (deploy + download)
```

**Critical path:** D02 → D04 → D06 → D07 → D08 → D09 → D10
