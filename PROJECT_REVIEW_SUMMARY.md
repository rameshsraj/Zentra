# Zentra — Full Project Review & Status Summary
**Date:** May 2026  
**Status:** Unified Rebranding Complete | OTel Module Deployed | Core Infrastructure Robust

## 1. Project Overview
Zentra (formerly InfraScope) is an enterprise-grade observability platform built on Rust (backend) and React (frontend). It specializes in deep Oracle Monitoring, OpenTelemetry native ingestion (OTLP), and eBPF-powered network visibility (Cilium/SkyWalking).

## 2. Module Status Audit

### **OpenTelemetry (OTel)**
- **Status:** 🟢 **Implemented & Deployed**
- **Capabilities:** Supports all 4 signals (Traces, Metrics, Logs, Profiles) via HTTP and gRPC (:4317).
- **Recent Wins:** Protobuf decoding enabled via `prost`; axum-based gRPC server live.
- **Gaps:** gRPC auth validation, background alert evaluation, and profiling visualization (flamegraphs).

### **Oracle Database Monitoring**
- **Status:** 🟡 **Partially Implemented (Frontend Ready / Backend Simulated)**
- **Capabilities:** Multi-tab UI for RAC, Data Guard, RMAN, Tablespaces, and TxEventQ.
- **Gaps:** Most backend collection still uses "SSH + sqlplus" fallback or simulated data. Native OCI/godror-style connection pooling is the primary missing foundation.

### **Network & Cilium (SkyWalking Alignment)**
- **Status:** 🟡 **Foundation Ready / Data Collection Pending**
- **Capabilities:** Cilium monitoring dashboard UI exists; Hubble flow schema defined.
- **Gaps:** Real-time Hubble gRPC flow collection and eBPF profiling workers are not yet active.

### **AI / LLM Observability**
- **Status:** 🟡 **Legacy Active / OTel Alignment Pending**
- **Capabilities:** Dashboard for token usage, latency, and cost tracking.
- **Gaps:** Needs transition from legacy custom endpoints to standard OTel `gen_ai.*` semantic conventions.

---

## 3. Technical Debt & Improvements
1. **Naming Consistency:** While "InfraScope" is gone from user-facing text, some internal crate folders or old migrations may still reference it. (Verified: Grep confirms 0 matches in source code, only leftover in system logs/build artifacts).
2. **Background Workers:** The system relies heavily on API-triggered logic. A unified `zentra-worker` or background task registry in `main.rs` is needed for:
   - Materialized View refreshes.
   - Alert rule evaluation.
   - Credential rotation (Vault).
3. **Database Schema:** `spans` table vs `traces` table terminology is inconsistent in some handlers. Recommend standardizing on `traces` to match OTel nomenclature.

---

## 4. Unified Roadmap (Next 4 Weeks)

### **Phase 1: Security & Automation (High Priority)**
- [ ] **OT-SEC:** Implement DB-backed API key validation for gRPC ingestion.
- [ ] **BKG-JOBS:** Add a background task runner for RED metric MV refresh and alert evaluation.
- [ ] **ORA-FFI:** Implement native Oracle connection pooling (remove SSH dependency).

### **Phase 2: Visualization & Depth (Medium Priority)**
- [ ] **PROF-UI:** Build the Flamegraph explorer for OTel Profiles.
- [ ] **HUBBLE:** Enable the gRPC stream collector for Hubble L7 flows.
- [ ] **AI-OTEL:** Align LLM dashboard with standard OTel GenAI spans.

### **Phase 3: Scale & Performance (Low Priority)**
- [ ] **TSDB:** Enable TimescaleDB compression for historical telemetry.
- [ ] **TraceQL:** Implement advanced filtering for distributed traces.
