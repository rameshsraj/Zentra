-- Add unique index to span_red_metrics_1m to support REFRESH MATERIALIZED VIEW CONCURRENTLY
-- Wrapped in DO block because the materialized view may not exist yet (created in 000070)
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'span_red_metrics_1m') THEN
        EXECUTE 'CREATE UNIQUE INDEX IF NOT EXISTS idx_span_red_metrics_unique ON span_red_metrics_1m (bucket, org_id, service_name, http_method, http_status_code, db_system)';
    END IF;
END $$;
