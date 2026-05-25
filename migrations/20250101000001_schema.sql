-- Zentra Database Schema v1.0
-- Target: PostgreSQL 15+ with TimescaleDB extension
-- All tables are org-scoped for multi-tenancy

-- Enable TimescaleDB
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Enable pg_trgm for full-text search
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ============================================================================
-- TARGETS — monitored infrastructure components
-- ============================================================================
CREATE TABLE IF NOT EXISTS targets (
    target_id       UUID PRIMARY KEY,
    org_id          UUID NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    target_type     VARCHAR(50) NOT NULL,       -- network, database, storage, server
    target_subtype  VARCHAR(50) NOT NULL,       -- postgresql, linux, interface, hp3par, etc.
    host            VARCHAR(255) NOT NULL,
    port            INTEGER NOT NULL,
    credentials     JSONB,                       -- encrypted connection credentials
    connection_options JSONB DEFAULT '{}'::jsonb,
    status          VARCHAR(20) DEFAULT 'pending',
    environment     VARCHAR(50) DEFAULT 'production',
    tags            JSONB DEFAULT '{}'::jsonb,
    check_interval_seconds INTEGER DEFAULT 60,
    last_check_at   TIMESTAMPTZ,
    last_error      TEXT,
    consecutive_failures INTEGER DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    is_active       BOOLEAN DEFAULT true
);

-- ============================================================================
-- METRICS — time-series metric data (TimescaleDB hypertable)
-- ============================================================================
CREATE TABLE IF NOT EXISTS metrics (
    timestamp       TIMESTAMPTZ NOT NULL,
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL,
    plugin_id       VARCHAR(100) NOT NULL,
    resource_type   VARCHAR(50) NOT NULL,
    resource_id     VARCHAR(255) NOT NULL,
    metric_name     VARCHAR(100) NOT NULL,
    value           DOUBLE PRECISION NOT NULL,
    unit            VARCHAR(30),
    tags            JSONB DEFAULT '{}'::jsonb,
    metadata        JSONB
);

-- Convert to hypertable (partitioned by time)
SELECT create_hypertable('metrics', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- ============================================================================
-- LOGS — centralized log storage (TimescaleDB hypertable)
-- ============================================================================
CREATE TABLE IF NOT EXISTS logs (
    log_id          UUID NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL,
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL,
    source          VARCHAR(255) NOT NULL,
    component       VARCHAR(255) DEFAULT '',
    component_type  VARCHAR(50) DEFAULT '',
    severity        VARCHAR(20) NOT NULL,
    message         TEXT NOT NULL,
    raw_log         TEXT,
    parsed_fields   JSONB DEFAULT '{}'::jsonb,
    tags            JSONB DEFAULT '{}'::jsonb
);

SELECT create_hypertable('logs', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Full-text search column
ALTER TABLE logs ADD COLUMN IF NOT EXISTS message_tsv tsvector
    GENERATED ALWAYS AS (to_tsvector('english', message)) STORED;

-- ============================================================================
-- ALERTS — active and historical alert instances
-- ============================================================================
CREATE TABLE IF NOT EXISTS alerts (
    alert_id        UUID PRIMARY KEY,
    rule_id         UUID NOT NULL,
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL,
    resource_id     VARCHAR(255) NOT NULL,
    severity        VARCHAR(20) NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'triggered',
    message         TEXT NOT NULL,
    metric_value    DOUBLE PRECISION NOT NULL,
    threshold       DOUBLE PRECISION NOT NULL,
    triggered_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    acknowledged_at TIMESTAMPTZ,
    acknowledged_by VARCHAR(255),
    resolved_at     TIMESTAMPTZ,
    metadata        JSONB
);

-- ============================================================================
-- ALERT_RULES — alert rule definitions
-- ============================================================================
CREATE TABLE IF NOT EXISTS alert_rules (
    rule_id         UUID PRIMARY KEY,
    org_id          UUID NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    metric_name     VARCHAR(100) NOT NULL,
    condition       JSONB NOT NULL,
    window_seconds  INTEGER NOT NULL,
    severity        VARCHAR(20) NOT NULL,
    enabled         BOOLEAN DEFAULT true,
    tags            JSONB DEFAULT '[]'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- ALERT_ROUTING — where to send alert notifications
-- ============================================================================
CREATE TABLE IF NOT EXISTS alert_routing (
    routing_id      UUID PRIMARY KEY,
    rule_id         UUID NOT NULL REFERENCES alert_rules(rule_id) ON DELETE CASCADE,
    org_id          UUID NOT NULL,
    channel         VARCHAR(50) NOT NULL,       -- email, slack, webhook, pagerduty
    config          JSONB NOT NULL,
    enabled         BOOLEAN DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- EVENTS — internal system events (hypertable)
-- ============================================================================
CREATE TABLE IF NOT EXISTS events (
    event_id        UUID NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_type      VARCHAR(50) NOT NULL,
    payload         JSONB NOT NULL,
    source          VARCHAR(255) NOT NULL
);

SELECT create_hypertable('events', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- ============================================================================
-- NETWORK_FLOWS — network traffic flow data (hypertable)
-- ============================================================================
CREATE TABLE IF NOT EXISTS network_flows (
    timestamp       TIMESTAMPTZ NOT NULL,
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL,
    src_ip          INET NOT NULL,
    src_port        INTEGER,
    dst_ip          INET NOT NULL,
    dst_port        INTEGER,
    protocol        VARCHAR(20) NOT NULL,
    service         VARCHAR(100),
    direction       VARCHAR(10) NOT NULL,       -- incoming, outgoing
    bytes_in        BIGINT DEFAULT 0,
    bytes_out       BIGINT DEFAULT 0,
    packets_in      BIGINT DEFAULT 0,
    packets_out     BIGINT DEFAULT 0,
    program         VARCHAR(255),
    country         VARCHAR(3),
    asn             VARCHAR(100),
    is_local        BOOLEAN DEFAULT false,
    tags            JSONB DEFAULT '{}'::jsonb
);

SELECT create_hypertable('network_flows', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- ============================================================================
-- DRILLDOWN_HIERARCHY — defines navigation hierarchy for drill-down engine
-- ============================================================================
CREATE TABLE IF NOT EXISTS drilldown_hierarchy (
    hierarchy_id    UUID PRIMARY KEY,
    org_id          UUID NOT NULL,
    parent_type     VARCHAR(50),
    parent_id       VARCHAR(255),
    child_type      VARCHAR(50) NOT NULL,
    child_id        VARCHAR(255) NOT NULL,
    label           VARCHAR(255) NOT NULL,
    metadata        JSONB DEFAULT '{}'::jsonb,
    sort_order      INTEGER DEFAULT 0,
    UNIQUE(org_id, parent_type, parent_id, child_type, child_id)
);

-- ============================================================================
-- THREATS — detected security threat events (hypertable)
-- ============================================================================
CREATE TABLE IF NOT EXISTS threats (
    threat_id       UUID NOT NULL,
    org_id          UUID NOT NULL,
    target_id       UUID,
    category        VARCHAR(50) NOT NULL,
    severity        VARCHAR(20) NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'active',
    source_ip       INET,
    dest_ip         INET,
    port            INTEGER,
    description     TEXT NOT NULL,
    evidence        JSONB DEFAULT '[]'::jsonb,
    mitre_tactic    VARCHAR(100),
    mitre_technique VARCHAR(20),
    first_seen      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen       TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_count     BIGINT DEFAULT 1,
    metadata        JSONB
);

SELECT create_hypertable('threats', 'first_seen',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- ============================================================================
-- DASHBOARDS — saved dashboard configurations
-- ============================================================================
CREATE TABLE IF NOT EXISTS dashboards (
    dashboard_id    UUID PRIMARY KEY,
    org_id          UUID NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    layout          JSONB NOT NULL,
    widgets         JSONB NOT NULL,
    is_default      BOOLEAN DEFAULT false,
    created_by      VARCHAR(255),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
