-- Zentra TimescaleDB Enhancements
-- Migration 008: Compression policies, 5-minute continuous aggregate, retention tuning
-- Ensures all hypertables are properly configured for production performance

-- ============================================================================
-- STEP 1: Enable compression on hypertables
-- Metrics: compress chunks older than 2 hours, segment by org_id + metric_name
-- ============================================================================
DO $$ BEGIN
    ALTER TABLE metrics SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'org_id, target_id, metric_name',
        timescaledb.compress_orderby = 'timestamp DESC'
    );
    PERFORM add_compression_policy('metrics', INTERVAL '2 hours', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'metrics compression skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

DO $$ BEGIN
    ALTER TABLE logs SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'org_id, severity, source',
        timescaledb.compress_orderby = 'timestamp DESC'
    );
    PERFORM add_compression_policy('logs', INTERVAL '6 hours', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'logs compression skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

DO $$ BEGIN
    ALTER TABLE network_flows SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'org_id, protocol, direction',
        timescaledb.compress_orderby = 'timestamp DESC'
    );
    PERFORM add_compression_policy('network_flows', INTERVAL '1 hour', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'network_flows compression skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

DO $$ BEGIN
    ALTER TABLE events SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'event_type',
        timescaledb.compress_orderby = 'timestamp DESC'
    );
    PERFORM add_compression_policy('events', INTERVAL '1 day', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'events compression skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

DO $$ BEGIN
    ALTER TABLE threats SET (
        timescaledb.compress,
        timescaledb.compress_segmentby = 'org_id, category, severity',
        timescaledb.compress_orderby = 'first_seen DESC'
    );
    PERFORM add_compression_policy('threats', INTERVAL '1 day', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'threats compression skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- ============================================================================
-- STEP 2: Add 5-minute continuous aggregate (missing from original schema)
-- ============================================================================
DO $$ BEGIN
    EXECUTE $q$
        CREATE MATERIALIZED VIEW IF NOT EXISTS metrics_5m
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('5 minutes', timestamp) AS bucket,
            org_id, target_id, metric_name, resource_type,
            avg(value) AS avg_value, max(value) AS max_value,
            min(value) AS min_value, count(*) AS sample_count
        FROM metrics GROUP BY 1, 2, 3, 4, 5 WITH NO DATA
    $q$;
    PERFORM add_continuous_aggregate_policy('metrics_5m',
        start_offset => INTERVAL '6 hours', end_offset => INTERVAL '5 minutes',
        schedule_interval => INTERVAL '5 minutes', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'metrics_5m aggregate skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- ============================================================================
-- STEP 3: Network flow rollups (1-minute and 1-hour)
-- ============================================================================
DO $$ BEGIN
    EXECUTE $q$
        CREATE MATERIALIZED VIEW IF NOT EXISTS network_flows_1m
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 minute', timestamp) AS bucket,
            org_id, protocol, direction,
            sum(bytes_in) AS total_bytes_in, sum(bytes_out) AS total_bytes_out,
            sum(packets_in) AS total_packets_in, sum(packets_out) AS total_packets_out,
            count(*) AS flow_count
        FROM network_flows GROUP BY 1, 2, 3, 4 WITH NO DATA
    $q$;
    PERFORM add_continuous_aggregate_policy('network_flows_1m',
        start_offset => INTERVAL '2 hours', end_offset => INTERVAL '1 minute',
        schedule_interval => INTERVAL '1 minute', if_not_exists => TRUE);
    EXECUTE $q$
        CREATE MATERIALIZED VIEW IF NOT EXISTS network_flows_1h
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 hour', timestamp) AS bucket,
            org_id, protocol, direction,
            sum(bytes_in) AS total_bytes_in, sum(bytes_out) AS total_bytes_out,
            sum(packets_in) AS total_packets_in, sum(packets_out) AS total_packets_out,
            count(*) AS flow_count
        FROM network_flows GROUP BY 1, 2, 3, 4 WITH NO DATA
    $q$;
    PERFORM add_continuous_aggregate_policy('network_flows_1h',
        start_offset => INTERVAL '2 days', end_offset => INTERVAL '1 hour',
        schedule_interval => INTERVAL '1 hour', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'network_flows aggregates skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- ============================================================================
-- STEP 4: Alert state tracking table for state machine
-- ============================================================================
CREATE TABLE IF NOT EXISTS alert_state_log (
    log_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_id        UUID NOT NULL,
    rule_id         UUID NOT NULL,
    org_id          UUID NOT NULL,
    previous_state  VARCHAR(20),
    new_state       VARCHAR(20) NOT NULL,
    transition_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    metadata        JSONB
);

CREATE INDEX IF NOT EXISTS idx_alert_state_log_alert ON alert_state_log(alert_id, transition_at DESC);
CREATE INDEX IF NOT EXISTS idx_alert_state_log_org ON alert_state_log(org_id, transition_at DESC);

-- ============================================================================
-- STEP 5: Notification routing policies table
-- ============================================================================
CREATE TABLE IF NOT EXISTS notification_policies (
    policy_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            VARCHAR(255) NOT NULL,
    description     TEXT,
    -- Match conditions (all must match for this policy to activate)
    match_severity  VARCHAR(20)[],           -- NULL = match all
    match_tags      JSONB DEFAULT '[]'::jsonb,
    match_target_type VARCHAR(50),            -- NULL = match all
    -- Routing
    channel_type    VARCHAR(50) NOT NULL,     -- email, slack, webhook, pagerduty, etc.
    channel_config  JSONB NOT NULL,           -- channel-specific settings
    -- Behavior
    enabled         BOOLEAN DEFAULT true,
    group_wait      INTEGER DEFAULT 30,       -- seconds to wait before sending first notification
    group_interval  INTEGER DEFAULT 300,      -- seconds between notifications for same group
    repeat_interval INTEGER DEFAULT 3600,     -- seconds before repeating a notification
    -- Mute timing
    mute_start_cron VARCHAR(100),             -- cron expression for mute window start
    mute_end_cron   VARCHAR(100),             -- cron expression for mute window end
    mute_days       INTEGER[] DEFAULT '{}',   -- 0=Sun..6=Sat, empty = no mute
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_notif_policies_org ON notification_policies(org_id, enabled);

-- ============================================================================
-- STEP 6: Notification dispatch history
-- ============================================================================
CREATE TABLE IF NOT EXISTS notification_dispatch_log (
    dispatch_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    alert_id        UUID,
    policy_id       UUID REFERENCES notification_policies(policy_id) ON DELETE SET NULL,
    channel_type    VARCHAR(50) NOT NULL,
    recipient       TEXT,
    success         BOOLEAN NOT NULL,
    response_code   INTEGER,
    error_message   TEXT,
    dispatched_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_dispatch_log_org ON notification_dispatch_log(org_id, dispatched_at DESC);
CREATE INDEX IF NOT EXISTS idx_dispatch_log_alert ON notification_dispatch_log(alert_id, dispatched_at DESC);

-- ============================================================================
-- STEP 7: Enhance alert_rules with pending duration and hysteresis
-- ============================================================================
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS for_duration_seconds INTEGER DEFAULT 0;
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS hysteresis_threshold DOUBLE PRECISION;
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS repeat_interval_seconds INTEGER DEFAULT 3600;
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS notification_policy_id UUID REFERENCES notification_policies(policy_id) ON DELETE SET NULL;

-- ============================================================================
-- STEP 8: Enhance alerts with pending state support
-- ============================================================================
ALTER TABLE alerts ADD COLUMN IF NOT EXISTS pending_since TIMESTAMPTZ;
ALTER TABLE alerts ADD COLUMN IF NOT EXISTS last_notified_at TIMESTAMPTZ;
ALTER TABLE alerts ADD COLUMN IF NOT EXISTS notification_count INTEGER DEFAULT 0;
