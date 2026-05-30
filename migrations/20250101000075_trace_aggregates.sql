-- Item 16: Trace percentile aggregates for p50/p95/p99 surfacing
-- Creates a continuous aggregate on traces for fast percentile queries.

DO $$
BEGIN
  -- Only create if traces table exists and we have TimescaleDB
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'traces') THEN
    -- Create the materialized view for span RED metrics with percentiles
    IF NOT EXISTS (SELECT 1 FROM pg_matviews WHERE matviewname = 'trace_aggregates_5m') THEN
      CREATE MATERIALIZED VIEW trace_aggregates_5m AS
      SELECT
        time_bucket('5 minutes', started_at) AS bucket,
        org_id,
        service_name,
        operation_name,
        COUNT(*) AS request_count,
        COUNT(*) FILTER (WHERE status = 'error') AS error_count,
        ROUND(COUNT(*) FILTER (WHERE status = 'error')::numeric / NULLIF(COUNT(*), 0) * 100, 2) AS error_rate,
        percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_us) AS p50_us,
        percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_us) AS p95_us,
        percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_us) AS p99_us,
        AVG(duration_us) AS avg_us,
        MAX(duration_us) AS max_us,
        MIN(duration_us) AS min_us
      FROM traces
      WHERE started_at > NOW() - INTERVAL '7 days'
      GROUP BY bucket, org_id, service_name, operation_name;

      CREATE INDEX IF NOT EXISTS idx_trace_agg_5m_lookup
        ON trace_aggregates_5m (org_id, service_name, bucket DESC);
    END IF;
  END IF;
END $$;
