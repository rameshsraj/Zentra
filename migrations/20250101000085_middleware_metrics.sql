-- middleware_metrics: time-series metrics collected from middleware pollers
CREATE TABLE IF NOT EXISTS middleware_metrics (
    id              BIGSERIAL,
    instance_id     TEXT NOT NULL,
    instance_type   TEXT NOT NULL,
    metric_name     TEXT NOT NULL,
    value           DOUBLE PRECISION NOT NULL,
    collected_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

SELECT create_hypertable('middleware_metrics', 'collected_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_mw_metrics_instance ON middleware_metrics(instance_id, metric_name, collected_at DESC);
