-- OT10: Fix TimescaleDB compression policies to use 'traces' (not 'spans').
-- Also adds retention policy to auto-drop old data.
-- Idempotent — safe to re-run.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
        -- Traces compression (7-day policy)
        BEGIN
            ALTER TABLE traces SET (
                timescaledb.compress,
                timescaledb.compress_segmentby = 'org_id, service_name',
                timescaledb.compress_orderby = 'started_at DESC'
            );
            PERFORM add_compression_policy('traces', INTERVAL '7 days', if_not_exists => true);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Traces compression setup skipped: %', SQLERRM;
        END;

        -- Retention policy: drop chunks older than 90 days
        BEGIN
            PERFORM add_retention_policy('traces', INTERVAL '90 days', if_not_exists => true);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Traces retention policy skipped: %', SQLERRM;
        END;

        -- Metrics retention
        BEGIN
            PERFORM add_retention_policy('metrics', INTERVAL '90 days', if_not_exists => true);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Metrics retention policy skipped: %', SQLERRM;
        END;

        -- Logs retention
        BEGIN
            PERFORM add_retention_policy('logs', INTERVAL '30 days', if_not_exists => true);
        EXCEPTION WHEN OTHERS THEN
            RAISE NOTICE 'Logs retention policy skipped: %', SQLERRM;
        END;
    END IF;
END $$;
