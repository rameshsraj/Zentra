-- OTel P09: Span-to-Metrics Connector — derived RED metrics from traces
CREATE TABLE IF NOT EXISTS otel_derived_metrics (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    service_name    TEXT NOT NULL,
    operation_name  TEXT,
    metric_name     TEXT NOT NULL,
    value           DOUBLE PRECISION NOT NULL,
    dimensions      JSONB NOT NULL DEFAULT '{}',
    bucket_start    TIMESTAMPTZ NOT NULL,
    bucket_duration INTERVAL NOT NULL DEFAULT '1 minute'
);
SELECT create_hypertable('otel_derived_metrics', 'bucket_start', migrate_data => true, if_not_exists => true);
CREATE INDEX IF NOT EXISTS idx_derived_metrics_service ON otel_derived_metrics(service_name, metric_name, bucket_start DESC);
