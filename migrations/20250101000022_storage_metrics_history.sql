-- ============================================================================
-- Storage Array Metrics History — persists SNMP poll snapshots for trend analysis
-- ============================================================================

CREATE TABLE IF NOT EXISTS storage_array_metrics (
    id              BIGSERIAL,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL,
    array_name      VARCHAR(255) NOT NULL,
    vendor          VARCHAR(100),
    model           VARCHAR(255),
    status          VARCHAR(50),

    -- Capacity
    total_tb        DOUBLE PRECISION NOT NULL DEFAULT 0,
    used_tb         DOUBLE PRECISION NOT NULL DEFAULT 0,
    free_tb         DOUBLE PRECISION NOT NULL DEFAULT 0,
    utilization_pct DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- Performance
    iops            BIGINT NOT NULL DEFAULT 0,
    throughput_mb   DOUBLE PRECISION NOT NULL DEFAULT 0,
    latency_ms      DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- Efficiency
    data_reduction_ratio DOUBLE PRECISION NOT NULL DEFAULT 0,

    -- Inventory
    snapshot_count  INTEGER NOT NULL DEFAULT 0,
    volume_count    INTEGER NOT NULL DEFAULT 0,

    -- Health
    overall_health  VARCHAR(50),
    alert_count     INTEGER NOT NULL DEFAULT 0,

    -- Replication
    replication_enabled BOOLEAN NOT NULL DEFAULT FALSE,

    -- Raw SNMP extends (full JSON for deep drill-down)
    raw_extends     JSONB,

    PRIMARY KEY (id, timestamp)
);

-- Convert to hypertable partitioned by time (1-day chunks)
SELECT create_hypertable('storage_array_metrics', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE,
    migrate_data => TRUE
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_sam_target_time
    ON storage_array_metrics (target_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_sam_org_time
    ON storage_array_metrics (org_id, timestamp DESC);

-- Retention policy: keep 90 days of detailed data
-- (TimescaleDB will automatically drop old chunks)
DO $$ BEGIN
    PERFORM add_retention_policy('storage_array_metrics', INTERVAL '90 days', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Retention policy skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- Continuous aggregate for hourly rollups (dashboard overview)
DO $$ BEGIN
    EXECUTE $DDL$
    CREATE MATERIALIZED VIEW IF NOT EXISTS storage_array_metrics_hourly
    WITH (timescaledb.continuous) AS
    SELECT
        time_bucket('1 hour', timestamp) AS bucket,
        org_id,
        target_id,
        array_name,
        AVG(total_tb)           AS avg_total_tb,
        AVG(used_tb)            AS avg_used_tb,
        AVG(utilization_pct)    AS avg_utilization_pct,
        AVG(iops)               AS avg_iops,
        MAX(iops)               AS max_iops,
        AVG(throughput_mb)      AS avg_throughput_mb,
        MAX(throughput_mb)      AS max_throughput_mb,
        AVG(latency_ms)         AS avg_latency_ms,
        MAX(latency_ms)         AS max_latency_ms,
        AVG(data_reduction_ratio) AS avg_data_reduction,
        MAX(alert_count)        AS max_alerts,
        COUNT(*)                AS sample_count
    FROM storage_array_metrics
    GROUP BY bucket, org_id, target_id, array_name
    WITH NO DATA
    $DDL$;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Continuous aggregate (hourly) skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- Refresh policy: compute hourly rollups, refreshing last 3 hours
DO $$ BEGIN
    PERFORM add_continuous_aggregate_policy('storage_array_metrics_hourly',
        start_offset    => INTERVAL '3 hours',
        end_offset      => INTERVAL '1 hour',
        schedule_interval => INTERVAL '1 hour',
        if_not_exists   => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Continuous aggregate policy skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- Daily rollups for long-term capacity trending
DO $$ BEGIN
    EXECUTE $DDL$
    CREATE MATERIALIZED VIEW IF NOT EXISTS storage_array_metrics_daily
    WITH (timescaledb.continuous) AS
    SELECT
        time_bucket('1 day', timestamp) AS bucket,
        org_id,
        target_id,
        array_name,
        AVG(total_tb)           AS avg_total_tb,
        AVG(used_tb)            AS avg_used_tb,
        MAX(used_tb)            AS max_used_tb,
        AVG(utilization_pct)    AS avg_utilization_pct,
        MAX(utilization_pct)    AS max_utilization_pct,
        AVG(iops)               AS avg_iops,
        MAX(iops)               AS peak_iops,
        AVG(throughput_mb)      AS avg_throughput_mb,
        AVG(latency_ms)         AS avg_latency_ms,
        MAX(latency_ms)         AS peak_latency_ms,
        AVG(data_reduction_ratio) AS avg_data_reduction,
        MAX(snapshot_count)     AS max_snapshots,
        MAX(volume_count)       AS max_volumes,
        MAX(alert_count)        AS max_alerts,
        COUNT(*)                AS sample_count
    FROM storage_array_metrics
    GROUP BY bucket, org_id, target_id, array_name
    WITH NO DATA
    $DDL$;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Continuous aggregate (daily) skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

DO $$ BEGIN
    PERFORM add_continuous_aggregate_policy('storage_array_metrics_daily',
        start_offset    => INTERVAL '3 days',
        end_offset      => INTERVAL '1 day',
        schedule_interval => INTERVAL '1 day',
        if_not_exists   => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Continuous aggregate policy skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;
