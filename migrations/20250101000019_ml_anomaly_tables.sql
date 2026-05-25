-- ML Anomaly detection results
CREATE TABLE IF NOT EXISTS anomaly_detections (
    id                  UUID NOT NULL DEFAULT gen_random_uuid(),
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
DO $$ BEGIN
    ALTER TABLE anomaly_detections ADD PRIMARY KEY (id, detected_at);
EXCEPTION WHEN OTHERS THEN NULL;
END $$;

CREATE INDEX IF NOT EXISTS idx_anomaly_target   ON anomaly_detections (target_id, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_metric   ON anomaly_detections (metric_name, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomaly_severity ON anomaly_detections (severity, detected_at DESC);

DO $$ BEGIN
    PERFORM add_compression_policy('anomaly_detections', INTERVAL '7 days', if_not_exists => TRUE);
    PERFORM add_retention_policy('anomaly_detections', INTERVAL '180 days', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Compression/retention policy skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

-- Seasonal detector baseline persistence (so warm state survives restart)
CREATE TABLE IF NOT EXISTS anomaly_seasonal_baselines (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    anomaly_id      UUID NOT NULL,
    is_real         BOOLEAN NOT NULL,
    comment         TEXT,
    created_by      UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
