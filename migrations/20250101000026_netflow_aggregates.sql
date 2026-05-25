-- Migration 026: NetFlow Continuous Aggregates
-- Adds 1-minute and 1-hour continuous aggregates over the netflow_records hypertable
-- for fast traffic analysis dashboards.

-- 1. netflow_1m — 1-minute rollup
DO $$ BEGIN
    CREATE MATERIALIZED VIEW netflow_1m
    WITH (timescaledb.continuous) AS
    SELECT
        time_bucket('1 minute', received_at) AS bucket,
        src_ip,
        dst_ip,
        protocol,
        SUM(bytes)   AS total_bytes,
        SUM(packets) AS total_packets,
        COUNT(*)     AS flow_count
    FROM netflow_records
    GROUP BY bucket, src_ip, dst_ip, protocol;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'netflow_1m already exists or error: %', SQLERRM;
END $$;

DO $$ BEGIN
    PERFORM add_continuous_aggregate_policy('netflow_1m',
        start_offset  => INTERVAL '10 minutes',
        end_offset    => INTERVAL '1 minute',
        schedule_interval => INTERVAL '1 minute',
        if_not_exists => TRUE
    );
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'netflow_1m policy skipped: %', SQLERRM;
END $$;

-- 2. netflow_1h — 1-hour rollup
DO $$ BEGIN
    CREATE MATERIALIZED VIEW netflow_1h
    WITH (timescaledb.continuous) AS
    SELECT
        time_bucket('1 hour', received_at) AS bucket,
        src_ip,
        dst_ip,
        protocol,
        SUM(bytes)   AS total_bytes,
        SUM(packets) AS total_packets,
        COUNT(*)     AS flow_count
    FROM netflow_records
    GROUP BY bucket, src_ip, dst_ip, protocol;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'netflow_1h already exists or error: %', SQLERRM;
END $$;

DO $$ BEGIN
    PERFORM add_continuous_aggregate_policy('netflow_1h',
        start_offset  => INTERVAL '4 hours',
        end_offset    => INTERVAL '1 hour',
        schedule_interval => INTERVAL '1 hour',
        if_not_exists => TRUE
    );
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'netflow_1h policy skipped: %', SQLERRM;
END $$;
