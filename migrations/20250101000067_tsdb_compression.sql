-- OT-PERF-01: Enable TimescaleDB compression for historical telemetry
-- Compressed after 7 days to save space while keeping data available for queries
-- Wrapped in exception blocks for robustness on fresh installs.

DO $$
DECLARE
    tbl TEXT;
BEGIN
    -- Traces/spans compression
    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'traces') THEN tbl := 'traces';
    ELSIF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'spans') THEN tbl := 'spans';
    ELSE tbl := NULL;
    END IF;

    IF tbl IS NOT NULL THEN
        BEGIN
            EXECUTE format('ALTER TABLE %I SET (timescaledb.compress, timescaledb.compress_segmentby = ''org_id, service_name'', timescaledb.compress_orderby = ''started_at DESC'')', tbl);
            EXECUTE format('SELECT add_compression_policy(''%s'', INTERVAL ''7 days'', if_not_exists => true)', tbl);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Traces compression skipped: %', SQLERRM;
        END;
    END IF;

    -- Metrics compression
    BEGIN
        ALTER TABLE metrics SET (
            timescaledb.compress,
            timescaledb.compress_segmentby = 'org_id, target_id, metric_name',
            timescaledb.compress_orderby = 'timestamp DESC'
        );
        PERFORM add_compression_policy('metrics', INTERVAL '7 days', if_not_exists => true);
    EXCEPTION WHEN OTHERS THEN
        RAISE NOTICE 'Metrics compression skipped: %', SQLERRM;
    END;

    -- Logs compression
    BEGIN
        ALTER TABLE logs SET (
            timescaledb.compress,
            timescaledb.compress_segmentby = 'org_id, target_id, source',
            timescaledb.compress_orderby = 'timestamp DESC'
        );
        PERFORM add_compression_policy('logs', INTERVAL '7 days', if_not_exists => true);
    EXCEPTION WHEN OTHERS THEN
        RAISE NOTICE 'Logs compression skipped: %', SQLERRM;
    END;

    -- OTel Profiles compression
    BEGIN
        ALTER TABLE otel_profiles SET (
            timescaledb.compress,
            timescaledb.compress_segmentby = 'org_id, service_name',
            timescaledb.compress_orderby = 'started_at DESC'
        );
        PERFORM add_compression_policy('otel_profiles', INTERVAL '7 days', if_not_exists => true);
    EXCEPTION WHEN OTHERS THEN
        RAISE NOTICE 'Profiles compression skipped: %', SQLERRM;
    END;
END $$;
