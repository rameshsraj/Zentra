# Zentra — Comprehensive Project Architecture & GAP Analysis

This document provides a rigorous, module-by-module architectural audit of the Zentra workspace, highlighting system gaps, recommending strategic enhancements, and laying out the future roadmap.

---

## 1. Module-by-Module Architectural Audit

### 1.1 OpenTelemetry Ingestion Subsystem (`crates/zentra-ingestion`, `crates/zentra-api`)
* **Current Design**: Supports distributed tracing, metrics, logs, and profiles over HTTP and gRPC (Port 4317). Uses `prost` to decode Protobuf payloads and translates them into internal JSON formats for ingestion.
* **Database Integration**: Standardizes on TimescaleDB (`span_red_metrics_1m` materialized views) to query distributed tracing metrics efficiently.
* **Gaps**:
  * **gRPC Authentication**: gRPC routes validate headers manually. It checks `x-zentra-org-id` and `Authorization: Bearer <key>`, but does not validate keys securely against a DB connection on every request.
  * **Profile Visualization**: The experimental `ProfilesService` parses data, but there is no flamegraph or profile aggregation query engine yet implemented.

### 1.2 Oracle Database Monitoring (`plugins/database-plugin`)
* **Current Design**: Implements active schedulers and loaders for Oracle RAC, Data Guard, RMAN, Tablespaces, and TXEventQ.
* **Gaps**:
  * **SSH Execution Wrapper**: Telemetry is collected by SSHing into the host and spawning `sqlplus` commands. This is highly inefficient and raises significant security concerns.
  * **OCI Connection Pooling**: Lacks a native, multi-threaded Rust connection pool utilizing Instant Client FFI or pure Rust drivers (e.g., `oracle` crate) to maintain active DB handles.

### 1.3 Cilium Network Monitoring (`plugins/network-plugin`)
* **Current Design**: Hubble schemas, flow models, traffic analyzers, and threat engines are in place.
* **Gaps**:
  * **Active Streaming Hubble Client**: A real-time Hubble gRPC streaming worker is missing. Currently, network flows are simulated or loaded statically rather than being pulled directly from the local Hubble Unix socket/TCP port.

### 1.4 AI/ML Observability (`plugins/ai-plugin`)
* **Current Design**: Provides ML anomaly score calculation, model health, and inference latency tracking.
* **Gaps**:
  * **Compile Mismatch**: `Metric::with_context` was modified in `zentra-core` to require 3 arguments (`org_id`, `target_id`, `plugin_id`), but `ai-plugin` calls it with only 2, causing workspace compilation to fail during `prod-license` compiles.
  * **OTel Semantic Realignment**: Custom metrics and spans are used instead of standardizing on standard OTel semantic conventions (e.g., `gen_ai.system`, `gen_ai.token_usage`).

### 1.5 Background Worker Engine (`crates/zentra-api/src/background_jobs.rs`)
* **Current Design**: Spawns simple tokio tasks for RED metric MV refreshes (every 1m), alert evaluation (every 5m), and API key cache pruning (every 1h).
* **Gaps**:
  * **Concurrency & Safety**: Relies on `CONCURRENTLY` SQL commands, which will fail if a unique index is missing on the materialized view.
  * **Distributed Task Coordination**: No transactional lock gates exist, meaning if multiple Zentra API nodes are active in a High-Availability cluster, they will attempt to refresh views and evaluate alerts concurrently, causing DB lock contention.

### 1.6 Licensing Engine (`crates/zentra-core/src/crypto/`, `plugins/license-plugin`)
* **Current Design**: Two-tier licensing system:
  * **Cryptographic Verifier** (`license_verifier.rs`): Ed25519 asymmetric signature verification using `ring`, gated behind `prod-license` cargo feature. Uses `Box<RawValue>` for canonical byte verification. Versioned envelope with `format_version`, `algorithm`, and `key_id` fields for staged key rotation.
  * **Policy Engine** (`license_policy.rs`): Comprehensive policy engine supporting expiry strategies (hard expire, grace period, notify-only), renewal strategies, transfer policies, overage handling, machine fingerprinting, heartbeat monitoring, and entitlements.
  * **License Plugin** (`plugins/license-plugin`): Background scheduler for expiry checks, seat utilization calculation, compliance scoring, and metric emission. Includes `LicenseProvider` trait abstraction for data source flexibility.
* **Gaps**:
  * **Production Key Rotation**: `EMBEDDED_PUBLIC_KEY_HEX` is still a placeholder (`0102030405...`). A real Ed25519 keypair must be generated and the public key embedded before any production release.
  * **Offline Signer CLI Tool**: No standalone CLI binary exists to generate and sign license files. Operators must manually script the signing process using the documented Python/Node.js examples in `docs/licensing/GENERATION_GUIDE.md`.
  * **Grace-Period UI Warnings**: The `license_policy.rs` engine supports `ExpiryStrategy::GracePeriod`, but the frontend has no banner or modal to warn users when their license is in grace period or expiring soon.
  * **License Plugin ↔ Verifier Integration**: The `license-plugin` uses ad-hoc day-threshold comparisons (`warning_threshold_days`, `critical_threshold_days`) rather than delegating to `ExpiryStrategy::is_usable()` from `license_policy.rs`, creating duplicated expiry logic.
  * **HMAC Dead Code in Policy Engine**: `license_policy.rs` (lines 815–900) still contains legacy HMAC-SHA256 key generation and validation functions that are now superseded by Ed25519 in the verifier. These ~300 lines should be deleted.

### 1.7 Security Plugin (`plugins/security-plugin`)
* **Current Design**: Implements a comprehensive security analytics suite with 9 sub-modules:
  * **IDS Engine** (`ids_engine.rs`): Suricata/Snort-style rule parsing and EVE JSON log event processing. Supports rule matching, alert generation, and Community ID correlation.
  * **Traffic Parser** (`traffic_parser.rs`): Raw packet parsing for security analysis.
  * **DNS Analyzer** (`dns_analyzer.rs`): DNS query/response anomaly detection.
  * **HTTP Analyzer** (`http_analyzer.rs`): HTTP traffic inspection.
  * **SSL Analyzer** (`ssl_analyzer.rs`): SSL/TLS certificate analysis and expiry monitoring.
  * **GeoIP** (`geoip.rs`): IP-to-geolocation mapping.
  * **Firewall** (`firewall.rs`): Firewall rule evaluation and hit tracking.
  * **File Analysis** (`file_analysis.rs`): File-based threat detection.
  * **Community ID** (`community_id.rs`): Standard network flow Community ID generation.
* **Gaps**:
  * **Static Metric Values**: All 5 security metrics (`security_ids_alerts_total`, `security_connections_blocked`, `security_dns_anomalies_detected`, `security_ssl_cert_expiry_days`, `security_firewall_rules_hit`) are emitted with hardcoded zero/constant values. The plugin does not yet connect its sub-module engines to the metric collection pipeline.
  * **No Real-Time Alert Pipeline**: IDS alerts are parsed but not routed to the Zentra alert evaluation engine or notification router (`crates/zentra-stream/src/notification_router.rs`).
  * **Missing Suricata/Snort Integration**: The IDS engine defines data structures but lacks an active EVE JSON log file watcher or Unix socket listener to ingest real-time alerts from a running Suricata instance.

### 1.8 Cloud Plugin (`plugins/cloud-plugin`)
* **Current Design**: Implements agentless monitoring for three major cloud providers:
  * **AWS CloudWatch** (`aws_cloudwatch.rs`): SigV4-signed HTTP requests (no AWS SDK). Collects EC2, RDS, ELB/ALB, Lambda, and ECS/EKS metrics. Credentials read from `target.credentials` JSON.
  * **Azure Monitor** (`azure_monitor.rs`): Azure REST API integration.
  * **GCP Monitoring** (`gcp_monitoring.rs`): GCP Monitoring API integration.
* **Gaps**:
  * **No Frontend Pages**: There are no dedicated UI pages or menu items in the frontend for viewing cloud monitoring data. The plugin collects metrics into the standard pipeline, but users cannot navigate to a "Cloud Monitoring" dashboard.
  * **No RBAC Module Mapping**: The two-layer RBAC system (`user_module_access`) has no `cloud_monitoring` module ID defined, so cloud-specific access control cannot be configured per user.
  * **Credential Rotation**: Cloud provider credentials are stored statically in `target.credentials`. There is no integration with HashiCorp Vault or automated credential rotation.

### 1.9 Storage Plugin (`plugins/storage-plugin`)
* **Current Design**: The most extensive collector plugin in the workspace, supporting 9 storage backends:
  * **NetApp ONTAP** (`netapp_ontap.rs`): REST API v1 — aggregates, volumes, SnapMirror, SVM, shelves, ports.
  * **Pure Storage** (`pure_storage.rs`): FlashArray REST API.
  * **Ceph Deep** (`ceph_deep.rs`): Ceph cluster health, OSD, pool, and PG metrics.
  * **Veeam** (`veeam.rs`): Veeam Backup & Replication collector.
  * **ZFS/TrueNAS** (`zfs_ssh.rs`): SSH-based ZFS pool and dataset collection.
  * **Dell EMC** (`dell_emc.rs`): Unity/PowerStore/VNX REST API.
  * **MinIO** (`minio_deep.rs`): S3-compatible MinIO metrics.
  * **IBM GPFS/Spectrum Scale** (`ibm_gpfs.rs`): GPFS filesystem metrics.
  * **Local Disk** (via `sysinfo`): Automatic local disk capacity metrics.
* **Gaps**:
  * **SSH-Based ZFS Collection**: The `zfs_ssh.rs` collector uses SSH subprocess execution (similar to the Oracle plugin's SSH problem). Should be migrated to ZFS native API or replaced with agent-based collection.
  * **No Storage-Specific Dashboard**: While the Infrastructure module's storage fleet card exists on the main Dashboard, there is no deep-dive storage monitoring page (e.g., per-volume IOPS, latency heatmaps, capacity forecasting).
  * **TLS Certificate Validation**: `NetAppOntapCollector` uses `danger_accept_invalid_certs(true)`, disabling TLS verification. This should be configurable per-target with proper CA certificate handling.

---

## 2. Technical Debt & Strategic Enhancements

1. **Native FFI Database Drivers**: Transition the Oracle plugin to maintain permanent session pools rather than spawning raw subprocess shells.
2. **Standard OTel Semantic Conventions**: Align all custom AI monitoring pipelines with OpenTelemetry spec standards.
3. **Database Schema Standardization**: Standardize the inconsistent use of `spans` and `traces` tables across DB migrations and handlers to use `traces` universally.
4. **HA Background Task Scheduler**: Introduce a transactional distributed locking mechanism (like Postgres advisory locks `pg_advisory_xact_lock`) in the background worker to coordinate job runs safely.
5. **Licensing Production Readiness**: Generate real Ed25519 keypair, build offline signer CLI, delete HMAC dead code from `license_policy.rs`, wire grace-period warnings into frontend.
6. **Security Plugin Activation**: Connect IDS engine sub-modules to the metric collection pipeline with real data. Implement a Suricata EVE JSON file watcher.
7. **Cloud Monitoring Frontend**: Build dedicated cloud monitoring pages for AWS, Azure, and GCP with appropriate RBAC module access entries.
8. **Storage TLS Hardening**: Replace `danger_accept_invalid_certs(true)` in storage collectors with configurable CA certificate stores per target.

---

## 3. High-Priority Workspace Fixes

To restore immediate, clean compilation across the entire workspace, the following change must be made in `plugins/ai-plugin/src/lib.rs` to fix the `with_context` signature mismatch:

```rust
// plugins/ai-plugin/src/lib.rs (Lines 57-82)
// Fix the method arguments:
metrics.push(
    Metric::new("ai_anomaly_score", 0.0, target.name.clone())
        .with_context(target.org_id, target.target_id, "ai-plugin")
        .with_unit("score")
        .with_resource_type("ai")
);
```
