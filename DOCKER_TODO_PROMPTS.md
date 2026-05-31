# Zentra — Docker Image: Build & Deploy Prompts

**Document date:** 2026-05-31
**Owner:** Zentra Team
**Scope:** Step-by-step prompts for building, verifying, and distributing the Zentra Docker image on the Linux build server (`10.88.11.162`).

---

## Golden Rules

> **1. BUILD ON THE SERVER ONLY — NEVER LOCALLY.**
> The Docker image must be built on the Linux build server (`10.88.11.162`) where Rust, Node.js, and Docker are installed.
> Do NOT attempt to build the image on a Windows/macOS dev machine. The Dockerfile targets Linux (Rocky Linux 8) and requires a Linux ELF binary.

> **2. IMAGE CONTAINS ONLY THE EXECUTABLE — NO SOURCE CODE.**
> The Docker image ships **only** the compiled binary (`zentra-server`), compiled frontend (`dist/`), SQL migrations, and config files.
> NO `.rs` files, NO `.ts` files, NO `Cargo.toml`, NO `Cargo.lock`, NO `.git/`, NO `node_modules/`, NO `frontend/src/`, NO `crates/`.
> The Rust source and TypeScript source are compiled **before** `docker build` and only the output artifacts are copied in.

---

## What Gets Copied Into the Image

| Source (Build Output) | Destination in Image | Purpose |
|-----------------------|----------------------|---------|
| `target/release/zentra-server` | `/usr/local/bin/zentra-server` | Stripped Rust/Axum backend binary |
| `migrations/*.sql` | `/opt/zentra/migrations/` | SQL schema migrations |
| `frontend/dist/` | `/opt/zentra/frontend/dist/` | Compiled React SPA (Vite build output) |
| `docker/supervisord.conf` | `/etc/supervisord.conf` | Process manager config |
| `docker/nginx.conf` | `/etc/nginx/conf.d/zentra.conf` | Reverse proxy config |
| `docker/docker-entrypoint.sh` | `/usr/local/bin/docker-entrypoint.sh` | Container startup script |

**Generated at build time inside the image:**

| File | Purpose |
|------|---------|
| `/opt/zentra/.build-info` | Build date + image version |
| `/opt/zentra/.integrity` | SHA-256 checksums of critical files |
| `/opt/zentra/.manifest-hash` | SHA-256 of the integrity file itself |

---

## Container Architecture — IP & Port Map

### Zentra Main Container (single container, all-in-one)

| Component | Bind Address | Port | Protocol | Exposed to Host? | Description |
|-----------|-------------|------|----------|------------------|-------------|
| **Nginx** | `0.0.0.0` | **5010** | HTTP | **Yes — only exposed port** | React SPA + reverse proxy to API |
| **Zentra API** | `127.0.0.1` | 5006 | HTTP | No | Rust/Axum backend (via Nginx `/api/*`) |
| **PostgreSQL** | `127.0.0.1` | 5434 | TCP | No | PG 16 + TimescaleDB |
| **OTLP gRPC** | `0.0.0.0` | 4317 | gRPC/H2 | No (optional `-p 4317:4317`) | OpenTelemetry gRPC ingest |
| **OTLP HTTP** | `0.0.0.0` | 4318 | HTTP | No (optional `-p 4318:4318`) | OpenTelemetry HTTP ingest |
| **Supervisord** | `127.0.0.1` | — | Unix socket | No | Process manager |

### Required Docker Port Mapping

```
docker run -p 5010:5010 ...                                      # Required — UI + API
docker run -p 5010:5010 -p 4317:4317 -p 4318:4318 ...           # Optional — adds OTLP ingest
```

> **Security:** Ports 5006 and 5434 are internal-only. Nginx proxies `/api/*` to `127.0.0.1:5006`. PostgreSQL `listen_addresses = '127.0.0.1'`. Direct access from outside the container is impossible.

---

## Simulation Lab — Full IP & Port Reference

The `docker/simulation/` lab creates **12 Docker containers** simulating a datacenter.

> **IPs are auto-detected at runtime.** The `setup.sh` (Linux) or `setup.ps1` (Windows) script
> detects the host's default network interface, subnet, and gateway, then assigns IPs
> starting from offset `+201` in the host's subnet. **No hardcoded IPs.**

### IP Assignment Pattern

If the host IP is `X.X.X.Y` on a `/24` or `/23` subnet, containers get `X.X.X.201` through `X.X.X.212`:

| # | Container | IP (auto-assigned) | Ports | Image | Role |
|---|-----------|---------------------|-------|-------|------|
| 1 | `srv-linux-01` | `<subnet>.201` | 22 (SSH), 161 (SNMP) | `rockylinux:8` | Linux Web Server |
| 2 | `srv-linux-02` | `<subnet>.202` | 22 (SSH), 161 (SNMP) | `ubuntu:22.04` | Linux App Server |
| 3 | `srv-win-01` | `<subnet>.203` | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Windows AD/DNS (sim) |
| 4 | `srv-win-02` | `<subnet>.204` | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Windows File Server (sim) |
| 5 | `sw-core-01` | `<subnet>.205` | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Cisco Catalyst 9300 (sim) |
| 6 | `sw-access-01` | `<subnet>.206` | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Arista 7050X (sim) |
| 7 | `fw-01` | `<subnet>.207` | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Palo Alto PA-3260 (sim) |
| 8 | `stor-01` | `<subnet>.208` | 22 (SSH), 161 (SNMP) | `polinux/snmpd:alpine` | Dell PowerStore (sim) |
| 9 | `mw-nginx-01` | `<subnet>.209` | 22, 80, 8080 (status) | `nginx:1.25-alpine` | Reverse Proxy |
| 10 | `mw-apache-01` | `<subnet>.210` | 22, 80, 8080 (status) | `httpd:2.4-alpine` | HTTP Server |
| 11 | `mw-tomcat-01` | `<subnet>.211` | 22, 8080, 9090 (JMX) | `tomcat:9-jdk17` | App Server |
| 12 | `db-oracle-01` | `<subnet>.212` | 22, 1521 (OCI) | `gvenzl/oracle-xe:21-slim` | Oracle 21c XE |

### How IPs Are Detected

- **Linux (`setup.sh`):** Uses macvlan on the host's default interface. Detects via `ip route` + `ip addr`.
- **Windows (`setup.ps1`):** Creates a Docker bridge network. Detects via `Get-NetRoute` + `Get-NetIPAddress`.
- The offset (`200`) is configurable in `setup.sh` → `IP_OFFSET_START=200`.

### After Setup — View Actual IPs

```bash
# Show all running lab containers with their actual IPs
docker ps --format 'table {{.Names}}\t{{.Status}}' | grep -E 'srv-|sw-|fw-|stor-|mw-|db-'
docker network inspect simulation_zentra-dc --format '{{range .Containers}}{{.Name}}\t{{.IPv4Address}}\n{{end}}'
```

### Lab Credentials

| Service | Username | Password / Community | Notes |
|---------|----------|----------------------|-------|
| **SSH (all 12)** | `root` | `zentra123` | Port 22 |
| **SNMP (all)** | — | `public` | v2c, port 161, read-only |
| **Tomcat Manager** | `zentra` | `zentra123` | `http://<ip>:8080/manager` |
| **Nginx Status** | — | — | `http://<ip>:8080/nginx_status` |
| **Apache Status** | — | — | `http://<ip>:8080/server-status` |
| **Oracle SYS** | `SYS` | `zentra123` | SID `XE`, Service `ZENTRADB`, port 1521 |
| **Oracle Monitor** | `zentra_mon` | `zentra123` | Read-only monitoring user |
| **Zentra Web UI** | `admin@zentra.local` | `changeme` | Host port 5010 |

---

## Prompt D01 — Build the Frontend

**Run on:** Linux build server (`10.88.11.162`)

> ```bash
> cd /store/Zentra/frontend
> npm ci
> npx vite build
> ```
>
> **Verify:**
> ```bash
> ls -la /store/Zentra/frontend/dist/index.html
> ls /store/Zentra/frontend/dist/assets/
> ```
>
> **Done when:** `frontend/dist/index.html` exists and `frontend/dist/assets/` contains hashed `.js` + `.css` bundles.

---

## Prompt D02 — Build the Rust Backend Binary

**Run on:** Linux build server (`10.88.11.162`)

> ```bash
> cd /store/Zentra
> cargo build --release --bin zentra-server
> ```
>
> **Verify:**
> ```bash
> file target/release/zentra-server
> # Must show: ELF 64-bit LSB executable, x86-64, dynamically linked
> ls -lh target/release/zentra-server
> # Size: ~30-50 MB before stripping
> ```
>
> **Done when:** `target/release/zentra-server` is a Linux ELF binary.

---

## Prompt D03 — Build the Docker Image

**Run on:** Linux build server (`10.88.11.162`)
**Depends on:** D01 (frontend dist), D02 (backend binary)

> ```bash
> cd /store/Zentra
> docker build -f docker/Dockerfile -t zentra:latest .
> ```
>
> **What the Dockerfile does (already implemented):**
>
> 1. Base: Rocky Linux 8 + PG 16 + TimescaleDB + Nginx + Supervisor + Oracle Instant Client + e2fsprogs
> 2. Removes dev tools (gcc, make, gdb, strace, editors, scripting languages)
> 3. Creates non-root `zentra` user (API server runs as this user via supervisord)
> 4. Copies and strips `target/release/zentra-server`, sets permissions `550 root:zentra`
> 5. Copies `migrations/`, `frontend/dist/`, `supervisord.conf`, `nginx.conf`, `docker-entrypoint.sh`
> 6. Writes `.build-info` with build timestamp
> 7. Sets file permissions: SQL files `400`, configs `600`, psql `750`
> 8. Build-time source-code leak check (fails if `.git`, `crates/`, `frontend/src/` exist)
> 9. Generates `.integrity` manifest (SHA-256 of critical files) + `.manifest-hash`
> 10. `EXPOSE 5010` only, `HEALTHCHECK`, `VOLUME /var/lib/postgresql/data`
>
> **Verify:**
> ```bash
> docker images zentra:latest
> # Should be ~800 MB – 1.2 GB
> ```
>
> **Done when:** `docker images zentra:latest` shows the image.

---

## Prompt D04 — Verify No Source Code in Image

**Run on:** Linux build server
**Depends on:** D03

> ```bash
> # Must return absolutely nothing
> docker run --rm zentra:latest sh -c '
>   find / -name "*.rs" -o -name "Cargo.toml" -o -name "Cargo.lock" \
>          -o -name ".git" -o -name "node_modules" 2>/dev/null | head -20
> '
>
> # Must show build-info
> docker run --rm zentra:latest cat /opt/zentra/.build-info
>
> # Must show the binary
> docker run --rm zentra:latest ls -la /usr/local/bin/zentra-server
>
> # Must show frontend dist
> docker run --rm zentra:latest ls /opt/zentra/frontend/dist/index.html
>
> # Must show integrity manifest
> docker run --rm zentra:latest cat /opt/zentra/.integrity
> ```
>
> **Done when:** No source code found, binary present, frontend present, integrity manifest present.

---

## Prompt D05 — Test Container Startup (End-to-End)

**Run on:** Linux build server
**Depends on:** D03

> ```bash
> # Start with zero configuration
> docker run -d --name zentra-test -p 5010:5010 \
>   -v zentra-test-data:/var/lib/postgresql/data \
>   zentra:latest
>
> # Wait for startup (30-45 seconds)
> sleep 45
>
> # Check health
> curl -sf http://localhost:5010/api/health
> # Expected: {"database":true,"status":"healthy",...}
>
> # Check frontend loads
> curl -sf http://localhost:5010 | head -5
> # Expected: <!doctype html>...
>
> # Check all 3 services running
> docker exec zentra-test supervisorctl status
> # Expected: nginx RUNNING, postgresql RUNNING, zentra-server RUNNING
>
> # Check license activated
> curl -sf http://localhost:5010/api/system/license-status
> # Expected: {"trial_expired":false,"days_remaining":3,...}
>
> # Check internal ports NOT accessible from outside
> curl -sf http://localhost:5006/api/health 2>&1 || echo "GOOD: port 5006 not exposed"
>
> # Cleanup
> docker stop zentra-test && docker rm zentra-test
> docker volume rm zentra-test-data
> ```
>
> **Done when:** Health returns `"healthy"`, frontend loads, 3 services RUNNING, license valid, port 5006 not accessible.

---

## Prompt D06 — Verify Security Hardening

**Run on:** Linux build server
**Depends on:** D05

> ```bash
> docker run -d --name zentra-sec -p 5011:5010 \
>   -v zentra-sec-data:/var/lib/postgresql/data \
>   zentra:latest
>
> sleep 45
>
> # 1. zentra-server runs as non-root 'zentra' user
> docker exec zentra-sec ps aux | grep zentra-server
> # Must show user=zentra, NOT root
>
> # 2. No compilers or debug tools
> docker exec zentra-sec which gcc 2>&1 || echo "GOOD: no gcc"
> docker exec zentra-sec which make 2>&1 || echo "GOOD: no make"
> docker exec zentra-sec which gdb 2>&1 || echo "GOOD: no gdb"
> docker exec zentra-sec which vi 2>&1 || echo "GOOD: no vi"
>
> # 3. Shells removed (after 5s lockdown delay)
> sleep 10
> docker exec zentra-sec /bin/bash -c "echo test" 2>&1 || echo "GOOD: no bash"
> docker exec zentra-sec /bin/sh -c "echo test" 2>&1 || echo "GOOD: no sh"
>
> # 4. No package managers
> docker exec zentra-sec which dnf 2>&1 || echo "GOOD: no dnf"
> docker exec zentra-sec which rpm 2>&1 || echo "GOOD: no rpm"
>
> # 5. chattr removed (immutable bits cannot be reversed)
> docker exec zentra-sec which chattr 2>&1 || echo "GOOD: chattr removed"
> docker exec zentra-sec which lsattr 2>&1 || echo "GOOD: lsattr removed"
>
> # 6. PostgreSQL not accessible from host
> psql -h localhost -p 5434 -U postgres 2>&1 || echo "GOOD: PG not exposed"
>
> # 7. /api/system/ blocked by Nginx from external
> curl -sf http://localhost:5011/api/system/db 2>&1 || echo "GOOD: /api/system/ blocked"
>
> # Cleanup
> docker stop zentra-sec && docker rm zentra-sec
> docker volume rm zentra-sec-data
> ```
>
> **Done when:** All checks pass — non-root user, no dev tools, no shells, no package managers, immutable files, PG not exposed, system endpoints blocked.

---

## Prompt D07 — Verify License & Tamper Protection

**Run on:** Linux build server
**Depends on:** D05

> ```bash
> # 1. Test integrity verification
> docker run -d --name zentra-tamper -p 5012:5010 \
>   -e ZENTRA_SKIP_LOCKDOWN=1 \
>   -v zentra-tamper-data:/var/lib/postgresql/data \
>   zentra:latest
> sleep 30
>
> # Check integrity passed on startup
> docker logs zentra-tamper 2>&1 | grep -i "integrity"
> # Expected: "Integrity verification PASSED"
>
> # 2. Test license spoofing — env var override must NOT work
> docker stop zentra-tamper && docker rm zentra-tamper
> docker run -d --name zentra-spoof -p 5013:5010 \
>   -e ZENTRA_TRIAL_EXPIRED=0 \
>   -e ZENTRA_SKIP_LOCKDOWN=1 \
>   -v zentra-spoof-data:/var/lib/postgresql/data \
>   zentra:latest
> sleep 45
>
> # Binary verifies license independently from disk — env var is ignored
> curl -sf http://localhost:5013/api/system/license-status
> # Should show real license state, not the spoofed env var
>
> # 3. Test graceful degradation
> # After 3 days, the container still starts but in degraded mode:
> #   - Only Dashboard is accessible
> #   - All other features return 402 Payment Required
> #   - Frontend shows amber warning banner
>
> # Cleanup
> docker stop zentra-spoof && docker rm zentra-spoof
> docker volume rm zentra-tamper-data zentra-spoof-data
> ```
>
> **Done when:** Tamper detection works, env var spoofing ignored, license verified from disk.

---

## Prompt D08 — Export Image as Portable Tarball

**Run on:** Linux build server
**Depends on:** D03, D04, D05, D06

> ```bash
> cd /store/Zentra
>
> # Export and compress
> docker save zentra:latest | gzip > docker/zentra-latest.tar.gz
>
> # Verify size
> ls -lh docker/zentra-latest.tar.gz
> # Expected: ~200-300 MB
>
> # Verify it can be loaded on a clean machine
> docker rmi zentra:latest
> docker load -i docker/zentra-latest.tar.gz
> docker images zentra:latest
> # Must show the image again
> ```
>
> **Done when:** `docker/zentra-latest.tar.gz` exists, is loadable, and produces a working image.

---

## Prompt D09 — Deploy to Client Machine (Zentra + Simulation Lab)

**Run on:** The **client/target machine** where Zentra will run. NOT the build server.

> Copy the `docker/` folder (with `zentra-latest.tar.gz`) to the client machine, then run:
>
> **Linux (full deploy — Zentra + 12-container simulation lab):**
> ```bash
> cd /path/to/docker
> sudo bash deploy-all.sh
> ```
>
> **Windows (full deploy — Zentra + 12-container simulation lab):**
> ```powershell
> cd C:\path\to\docker
> powershell -ExecutionPolicy Bypass -File deploy-all.ps1
> ```
>
> **What `deploy-all` does automatically:**
>
> 1. **Phase 1 — Zentra App** (calls `install.sh` / `install.ps1`):
>    - Loads `zentra-latest.tar.gz` into Docker
>    - Creates `.env` with auto-generated JWT secret + PG password
>    - Starts the Zentra container on port `5010`
>    - Waits for health check to pass
>
> 2. **Phase 2 — Simulation Lab** (calls `simulation/setup.sh` / `simulation/setup.ps1`):
>    - **Auto-detects** the client's network interface, subnet, and gateway
>    - Assigns IPs `<subnet>.201` through `<subnet>.212` (macvlan on Linux, bridge on Windows)
>    - Pulls and starts all 12 lab containers
>
> 3. **Phase 3 — Connect Zentra to Lab:**
>    - Connects the Zentra container to the simulation network
>    - Generates SSH keys inside Zentra
>    - Distributes SSH keys to all 12 lab containers (passwordless SSH)
>    - Verifies connectivity
>
> 4. **Phase 4 — Summary Report:**
>    - Prints all container names, auto-detected IPs, ports, status, and credentials
>
> **Zentra-only (no lab):**
> ```bash
> sudo bash install.sh        # Linux
> ```
> ```powershell
> powershell -ExecutionPolicy Bypass -File install.ps1   # Windows
> ```
>
> **Lab-only (Zentra already running):**
> ```bash
> cd simulation && sudo bash setup.sh     # Linux
> ```
> ```powershell
> cd simulation; powershell -ExecutionPolicy Bypass -File setup.ps1   # Windows
> ```
>
> **Done when:** `deploy-all` prints the summary with all IPs, all 12 lab containers UP, and Zentra healthy.

---

## Prompt D10 — Add Lab Targets to Zentra UI

**Run on:** Browser, after D09 completes

> Open `http://<client-ip>:5010`, login with `admin@zentra.local` / `changeme`, then add:
>
> | Target Type | Where in Zentra | What to Enter |
> |-------------|----------------|---------------|
> | Linux/Windows Servers | Infrastructure > Servers > Add | IP from report, port 22, `root` / `zentra123` |
> | Network Switches | Infrastructure > Network > Add | IP from report, SNMP community `public` |
> | Firewall | Infrastructure > Network > Add | IP from report, SNMP community `public` |
> | Storage Array | Infrastructure > Storage > Add | IP from report, SSH `root` / `zentra123` |
> | Nginx | Middleware > Nginx > Add | IP from report, `http://<ip>:8080/nginx_status` |
> | Apache | Middleware > Apache > Add | IP from report, `http://<ip>:8080/server-status` |
> | Tomcat | Middleware > Tomcat > Add | IP:8080, `zentra` / `zentra123` |
> | Oracle | Database > Oracle > Add | IP:1521, SID `XE`, `zentra_mon` / `zentra123` |
>
> **Done when:** All 12 lab components appear as targets in the Zentra dashboard.

---

## Automated Build Script (`docker/build-image.sh`)

For **frequent builds**, use the automated script instead of running prompts manually.

### Full Build (frontend + backend + image + tarball)

```bash
# On the build server (10.88.11.162)
cd /store/Zentra
bash docker/build-image.sh
```

### Incremental Builds (faster)

```bash
# Skip frontend (reuse existing dist/)
bash docker/build-image.sh --skip-fe

# Skip backend (reuse existing binary)
bash docker/build-image.sh --skip-be

# Rebuild Docker image only (reuse both frontend + backend)
bash docker/build-image.sh --image-only
```

### Download Tarball to Local Machine

```powershell
# From your Windows dev machine
scp root@10.88.11.162:/store/Zentra/docker/zentra-latest.tar.gz C:\git\Zentra\docker\zentra-latest.tar.gz
```

```bash
# From a Linux/macOS dev machine
scp root@10.88.11.162:/store/Zentra/docker/zentra-latest.tar.gz ./docker/zentra-latest.tar.gz
```

### One-Liner: Build on Server + Download Locally

```powershell
# Run from Windows dev machine — builds on server, then downloads the tarball
ssh root@10.88.11.162 "cd /store/Zentra && bash docker/build-image.sh" ; scp root@10.88.11.162:/store/Zentra/docker/zentra-latest.tar.gz C:\git\Zentra\docker\zentra-latest.tar.gz
```

### What the Script Does

1. **D01** — `npm ci && npx vite build` (skippable with `--skip-fe`)
2. **D02** — `cargo build --release --bin zentra-server` (skippable with `--skip-be`)
3. **D03** — `docker build -f docker/Dockerfile -t zentra:latest .`
4. **D04** — Verifies no `.rs`, `.toml`, `.git`, `node_modules` in image
5. **D04** — Prints binary size, frontend status, migration count, integrity check
6. **D08** — `docker save | gzip > docker/zentra-latest.tar.gz`

All verification uses `--entrypoint sh` to bypass the container entrypoint.

---

## Existing Docker Files Reference

### Image Build (run on build server)

| File | What It Does |
|------|--------------|
| `docker/Dockerfile` | Rocky Linux 8 base, PG 16, TimescaleDB, Nginx, Supervisor, Oracle IC, hardening, integrity manifest |
| `docker/docker-entrypoint.sh` | Tamper verification, license check (3-day trial), PG init, restricted `zentra_app` user, SSH keys, self-lockdown |
| `docker/supervisord.conf` | Manages postgresql → zentra-server → nginx startup order |
| `docker/nginx.conf` | Frontend on `:5010`, proxy `/api/*` to `:5006`, rate limiting, security headers, CSP |
| `.dockerignore` | Excludes all source code, `.git/`, `node_modules/`, build intermediates |
| `docker/build-image.sh` | **Build server script** — builds image, verifies, exports tarball, cleans up Docker |

### Client Deployment (run on target machine)

| File | OS | What It Does |
|------|----|--------------|
| `docker/deploy-all.sh` | Linux | **Full deploy** — Zentra + simulation lab + SSH keys + network connect + IP report |
| `docker/deploy-all.ps1` | Windows | **Full deploy** — same as above for PowerShell |
| `docker/install.sh` | Linux | **Zentra-only** — load image, create `.env`, start container, health check |
| `docker/install.ps1` | Windows | **Zentra-only** — same as above for PowerShell |
| `docker/simulation/setup.sh` | Linux | **Lab-only** — auto-detect network, start 12 containers (macvlan) |
| `docker/simulation/setup.ps1` | Windows | **Lab-only** — auto-detect network, start 12 containers (bridge) |
| `docker/simulation/traffic-sim.sh` | Linux | Generate simulated traffic between lab containers |
| `docker/simulation/traffic-sim.ps1` | Windows | Same as above for PowerShell |
| `docker/simulation/docker-compose.yml` | — | Template compose file (IPs replaced at runtime by setup scripts) |
| `docker/simulation/configs/` | — | SNMP, SSH, Nginx, Apache, Tomcat, Oracle configs for lab containers |
| `docker/validate.sh` | Linux | Post-build validation script |
| `docker/README.md` | — | Full installation guide with hardened `docker run` commands |
| `docker/.env.example` | — | Template environment variables |

## Security Measures Summary

| # | Measure | Where |
|---|---------|-------|
| 1 | No source code in image | `.dockerignore` + Dockerfile build-time check |
| 2 | Binary stripped | `strip /usr/local/bin/zentra-server` in Dockerfile |
| 3 | Non-root API process | `user=zentra` in `supervisord.conf` |
| 4 | Restricted DB user | `zentra_app` with DML-only grants (not superuser) |
| 5 | Only port 5010 exposed | `EXPOSE 5010` — no 5006, no 5434 |
| 6 | API binds to localhost | `ZENTRA_BIND_ADDR=127.0.0.1` |
| 7 | PG binds to localhost | `listen_addresses = '127.0.0.1'` |
| 8 | SQL files read-only | `chmod 400` in Dockerfile |
| 9 | Configs permission-locked | `chmod 600` for supervisord.conf, nginx.conf |
| 10 | Security headers | CSP, X-Frame-Options, X-Content-Type-Options, etc. in nginx.conf |
| 11 | Login rate limiting | 5 req/min per IP with burst 3 |
| 12 | Internal endpoints blocked | `/api/system/` and `/api/pool-status` — Nginx `deny all` |
| 13 | Sensitive files blocked | `.env`, `.git`, `.rs`, `.sql`, `.sh` — Nginx `deny all` |
| 14 | Dev tools removed | gcc, make, gdb, strace, vi, nano, perl, lua, ruby |
| 15 | Core dumps disabled | `* hard core 0` |
| 16 | SUID/SGID cleared | `find / -perm /6000 ... chmod a-s` |
| 17 | Integrity manifest | SHA-256 checksums of critical files + manifest hash |
| 18 | Tamper detection on startup | Entrypoint verifies `.integrity` + `.manifest-hash` before anything else |
| 19 | 3-day signed trial license | HMAC-signed with binary hash, persisted in PG volume |
| 20 | Graceful degradation | Dashboard-only mode after expiry, 402 for all other APIs |
| 21 | Binary license verification | Rust binary verifies independently — ignores env vars |
| 22 | Immutable files (chattr +i) | License, configs, integrity, binary, migrations, frontend dist |
| 23 | chattr self-destructs | Removed after making files immutable — bits cannot be reversed |
| 24 | Package managers removed | dnf, yum, rpm deleted at runtime |
| 25 | All shells removed | bash, sh, csh, zsh, ksh, fish, dash — removed 5s after supervisord starts |
| 26 | `--read-only` support | Documented with required tmpfs mounts |
| 27 | `--cap-drop ALL` | Only CHOWN, SETUID, SETGID, DAC_OVERRIDE, FOWNER, NET_BIND_SERVICE, LINUX_IMMUTABLE |
| 28 | `--security-opt=no-new-privileges` | Prevents privilege escalation |
