-- Phase 1: New tables for SNMP Traps, Alert Dependencies, Escalation, and Deep Server Metrics

-- SNMP Traps received by the trap receiver
CREATE TABLE IF NOT EXISTS snmp_traps (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    received_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_ip       INET NOT NULL,
    source_port     INT,
    version         TEXT NOT NULL DEFAULT 'v2c',
    community       TEXT,
    enterprise_oid  TEXT,
    trap_type       TEXT,
    varbinds        JSONB,
    decoded_varbinds JSONB,
    severity        TEXT NOT NULL DEFAULT 'info',
    acknowledged    BOOLEAN NOT NULL DEFAULT FALSE,
    target_id       UUID REFERENCES targets(target_id) ON DELETE SET NULL
);

SELECT create_hypertable('snmp_traps', 'received_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_snmp_traps_source_ip   ON snmp_traps (source_ip, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_snmp_traps_severity    ON snmp_traps (severity, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_snmp_traps_acked       ON snmp_traps (acknowledged) WHERE acknowledged = FALSE;
CREATE INDEX IF NOT EXISTS idx_snmp_traps_oid         ON snmp_traps USING GIN (enterprise_oid gin_trgm_ops) WHERE enterprise_oid IS NOT NULL;

-- Loaded MIB definitions
CREATE TABLE IF NOT EXISTS loaded_mibs (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT NOT NULL UNIQUE,
    oid_prefix  TEXT,
    file_name   TEXT,
    content     TEXT,
    loaded_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    object_count INT DEFAULT 0
);

-- SNMP Trap processing rules
CREATE TABLE IF NOT EXISTS snmp_trap_rules (
    id                       UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name                     TEXT NOT NULL,
    oid_pattern              TEXT,
    source_ip_pattern        TEXT,
    severity_map             JSONB NOT NULL DEFAULT '{}',
    auto_acknowledge         BOOLEAN NOT NULL DEFAULT FALSE,
    create_alert             BOOLEAN NOT NULL DEFAULT TRUE,
    notification_channels    UUID[] NOT NULL DEFAULT '{}',
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_snmp_trap_rules_name ON snmp_trap_rules (name);

-- Alert rule parent/child suppression dependencies
CREATE TABLE IF NOT EXISTS alert_rule_dependencies (
    id                          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    parent_rule_id              UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    child_rule_id               UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    suppress_duration_minutes   BIGINT NOT NULL DEFAULT 0,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (parent_rule_id, child_rule_id)
);

CREATE INDEX IF NOT EXISTS idx_alert_dep_parent ON alert_rule_dependencies (parent_rule_id);
CREATE INDEX IF NOT EXISTS idx_alert_dep_child  ON alert_rule_dependencies (child_rule_id);

-- Alert escalation chains
CREATE TABLE IF NOT EXISTS alert_escalation_chains (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    alert_rule_id  UUID NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    levels         JSONB NOT NULL DEFAULT '[]',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (alert_rule_id)
);

-- Per-core CPU metrics (high-cardinality; hypertable for compression)
CREATE TABLE IF NOT EXISTS server_cpu_core_metrics (
    time        TIMESTAMPTZ NOT NULL,
    target_id   UUID NOT NULL,
    core_id     INT NOT NULL,
    user_pct    FLOAT8,
    system_pct  FLOAT8,
    iowait_pct  FLOAT8,
    steal_pct   FLOAT8,
    irq_pct     FLOAT8,
    softirq_pct FLOAT8,
    idle_pct    FLOAT8
);

SELECT create_hypertable('server_cpu_core_metrics', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_server_cpu_core_target ON server_cpu_core_metrics (target_id, time DESC);

-- Top-process snapshots
CREATE TABLE IF NOT EXISTS server_process_snapshots (
    time        TIMESTAMPTZ NOT NULL,
    target_id   UUID NOT NULL,
    pid         BIGINT NOT NULL,
    name        TEXT,
    cpu_pct     FLOAT8,
    mem_pct     FLOAT8,
    rss_kb      FLOAT8,
    vsz_kb      FLOAT8,
    threads     INT,
    state       TEXT
);

SELECT create_hypertable('server_process_snapshots', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_server_proc_target ON server_process_snapshots (target_id, time DESC);

-- SMART disk health data
CREATE TABLE IF NOT EXISTS disk_smart_data (
    time              TIMESTAMPTZ NOT NULL,
    target_id         UUID NOT NULL,
    device            TEXT NOT NULL,
    smart_passed      BOOLEAN,
    temperature_c     INT,
    power_on_hours    BIGINT,
    power_cycle_count BIGINT,
    reallocated_sectors BIGINT,
    pending_sectors   BIGINT,
    uncorrectable_errors BIGINT,
    crc_errors        BIGINT,
    wear_level_pct    INT,
    nvme_spare_pct    INT,
    nvme_used_pct     INT,
    nvme_media_errors BIGINT,
    raw_attrs         JSONB
);

SELECT create_hypertable('disk_smart_data', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_disk_smart_target ON disk_smart_data (target_id, time DESC);

-- Redfish/BMC hardware inventory snapshots
CREATE TABLE IF NOT EXISTS server_redfish_inventory (
    time              TIMESTAMPTZ NOT NULL,
    target_id         UUID NOT NULL,
    system_health     FLOAT8,
    processor_count   INT,
    memory_gib        FLOAT8,
    power_state       TEXT,
    thermal_sensors   JSONB,
    fans              JSONB,
    power_supplies    JSONB,
    raw_data          JSONB
);

SELECT create_hypertable('server_redfish_inventory', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_redfish_inv_target ON server_redfish_inventory (target_id, time DESC);

-- Retention policies (compress after 7d, drop after 90d for high-cardinality tables)
SELECT add_compression_policy('server_cpu_core_metrics', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_compression_policy('server_process_snapshots', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_compression_policy('disk_smart_data', INTERVAL '30 days', if_not_exists => TRUE);
SELECT add_compression_policy('snmp_traps', INTERVAL '7 days', if_not_exists => TRUE);

-- pg_trgm for SNMP OID text search (if not already enabled)
CREATE EXTENSION IF NOT EXISTS pg_trgm;
