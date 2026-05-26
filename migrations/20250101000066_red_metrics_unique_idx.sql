-- Add unique index to span_red_metrics_1m to support REFRESH MATERIALIZED VIEW CONCURRENTLY
CREATE UNIQUE INDEX IF NOT EXISTS idx_span_red_metrics_unique ON span_red_metrics_1m (bucket, org_id, service_name, http_method, http_status_code, db_system);
