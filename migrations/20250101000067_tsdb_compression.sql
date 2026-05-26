-- OT-PERF-01: Enable TimescaleDB compression for historical telemetry
-- Compressed after 7 days to save space while keeping data available for queries

-- Traces
ALTER TABLE spans SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'org_id, service_name',
  timescaledb.compress_orderby = 'started_at DESC'
);
SELECT add_compression_policy('spans', INTERVAL '7 days', if_not_exists => true);

-- Metrics
ALTER TABLE metrics SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'org_id, target_id, metric_name',
  timescaledb.compress_orderby = 'timestamp DESC'
);
SELECT add_compression_policy('metrics', INTERVAL '7 days', if_not_exists => true);

-- Logs
ALTER TABLE logs SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'org_id, target_id, source',
  timescaledb.compress_orderby = 'timestamp DESC'
);
SELECT add_compression_policy('logs', INTERVAL '7 days', if_not_exists => true);

-- OTel Profiles
ALTER TABLE otel_profiles SET (
  timescaledb.compress,
  timescaledb.compress_segmentby = 'org_id, service_name',
  timescaledb.compress_orderby = 'started_at DESC'
);
SELECT add_compression_policy('otel_profiles', INTERVAL '7 days', if_not_exists => true);
