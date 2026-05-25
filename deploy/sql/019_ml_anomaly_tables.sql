-- ML Anomaly detection results
CREATE TABLE IF NOT EXISTS anomaly_detections (
    id                  UUID NOT NULL DEFAULT uuid_generate_v4(),
    detected_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    target_id           UUID REFERENCES targets(target_id) ON DELETE CASCADE,
    metric_name         TEXT NOT NULL,
    value               FLOAT8 NOT NULL,
    expected_mean       FLOAT8,
    expected_stddev     FLOAT8,
    deviation_sigma     FLOAT8,
    detector            TEXT NOT NULL,  -- zscore | ewma | seasonal
    severity            TEXT NOT NULL DEFAULT 'info',
    acknowledged        BOOLEAN NOT NULL DEFAULT FALSE,
    false_positive      BOOLEAN NOT NULL DEFAULT FALSE
);

SELECT create_hypertable('anomaly_detections', 'detected_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_anomaly_target   ON anomaly_detections (target_id, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_metric   ON anomaly_detections (metric_name, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_severity ON anomaly_detections (severity, detected_at DESC);

SELECT add_compression_policy('anomaly_detections', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('anomaly_detections', INTERVAL '180 days', if_not_exists => TRUE);

-- Seasonal detector baseline persistence (so warm state survives restart)
CREATE TABLE IF NOT EXISTS anomaly_seasonal_baselines (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    target_id       UUID NOT NULL REFERENCES targets(target_id) ON DELETE CASCADE,
    metric_name     TEXT NOT NULL,
    bucket_key      INT NOT NULL,   -- hour*7 + weekday (0..167)
    mean            FLOAT8 NOT NULL,
    stddev          FLOAT8 NOT NULL,
    sample_count    BIGINT NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (target_id, metric_name, bucket_key)
);

CREATE INDEX IF NOT EXISTS idx_seasonal_target ON anomaly_seasonal_baselines (target_id, metric_name);

-- Anomaly feedback for model improvement
CREATE TABLE IF NOT EXISTS anomaly_feedback (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    anomaly_id      UUID NOT NULL REFERENCES anomaly_detections(id) ON DELETE CASCADE,
    is_real         BOOLEAN NOT NULL,
    comment         TEXT,
    created_by      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
