-- Zentra Predictive Alerting Engine
-- Migration 004: Metric baselines, predictions, anomaly events

-- ============================================================================
-- Metric Baselines (rolling statistics per metric stream)
-- ============================================================================
CREATE TABLE IF NOT EXISTS metric_baselines (
    baseline_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL REFERENCES targets(target_id) ON DELETE CASCADE,
    metric_name     TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    mean            DOUBLE PRECISION NOT NULL,
    stddev          DOUBLE PRECISION NOT NULL,
    min_val         DOUBLE PRECISION NOT NULL,
    max_val         DOUBLE PRECISION NOT NULL,
    sample_count    BIGINT NOT NULL DEFAULT 0,
    window_start    TIMESTAMPTZ NOT NULL,
    window_end      TIMESTAMPTZ NOT NULL,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(org_id, target_id, metric_name, resource_id)
);

CREATE INDEX IF NOT EXISTS idx_baselines_target ON metric_baselines(target_id);
CREATE INDEX IF NOT EXISTS idx_baselines_metric ON metric_baselines(metric_name);
CREATE INDEX IF NOT EXISTS idx_baselines_computed ON metric_baselines(computed_at);

-- ============================================================================
-- Predictions / Forecasts
-- ============================================================================
CREATE TABLE IF NOT EXISTS predictions (
    forecast_id     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL REFERENCES targets(target_id) ON DELETE CASCADE,
    metric_name     TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    current_value   DOUBLE PRECISION NOT NULL,
    predicted_value DOUBLE PRECISION NOT NULL,
    rate_per_hour   DOUBLE PRECISION NOT NULL,
    hours_to_threshold DOUBLE PRECISION,
    threshold       DOUBLE PRECISION NOT NULL,
    confidence      DOUBLE PRECISION NOT NULL DEFAULT 0,
    sample_count    BIGINT NOT NULL DEFAULT 0,
    description     TEXT,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_predictions_target ON predictions(target_id);
CREATE INDEX IF NOT EXISTS idx_predictions_metric ON predictions(metric_name);
CREATE INDEX IF NOT EXISTS idx_predictions_breach ON predictions(hours_to_threshold)
    WHERE hours_to_threshold IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_predictions_computed ON predictions(computed_at);

-- ============================================================================
-- Anomaly Events
-- ============================================================================
CREATE TABLE IF NOT EXISTS anomaly_events (
    anomaly_id      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL REFERENCES targets(target_id) ON DELETE CASCADE,
    metric_name     TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    observed_value  DOUBLE PRECISION NOT NULL,
    expected_mean   DOUBLE PRECISION NOT NULL,
    expected_stddev DOUBLE PRECISION NOT NULL,
    z_score         DOUBLE PRECISION NOT NULL,
    severity        TEXT NOT NULL DEFAULT 'low'
                    CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    description     TEXT,
    detected_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    baseline_window_start TIMESTAMPTZ,
    baseline_window_end   TIMESTAMPTZ,
    acknowledged    BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_by TEXT,
    acknowledged_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_anomalies_target ON anomaly_events(target_id);
CREATE INDEX IF NOT EXISTS idx_anomalies_metric ON anomaly_events(metric_name);
CREATE INDEX IF NOT EXISTS idx_anomalies_severity ON anomaly_events(severity);
CREATE INDEX IF NOT EXISTS idx_anomalies_detected ON anomaly_events(detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_anomalies_unacked ON anomaly_events(acknowledged)
    WHERE acknowledged = FALSE;

-- ============================================================================
-- Trend Alerts
-- ============================================================================
CREATE TABLE IF NOT EXISTS trend_alerts (
    alert_id        UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    target_id       UUID NOT NULL REFERENCES targets(target_id) ON DELETE CASCADE,
    metric_name     TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    alert_type      TEXT NOT NULL DEFAULT 'capacity_exhaustion'
                    CHECK (alert_type IN ('capacity_exhaustion', 'anomaly_spike',
                                          'degradation_trend', 'resource_contention')),
    severity        TEXT NOT NULL DEFAULT 'info'
                    CHECK (severity IN ('info', 'warning', 'critical')),
    title           TEXT NOT NULL,
    description     TEXT,
    current_value   DOUBLE PRECISION,
    predicted_value DOUBLE PRECISION,
    threshold       DOUBLE PRECISION,
    hours_to_breach DOUBLE PRECISION,
    confidence      DOUBLE PRECISION NOT NULL DEFAULT 0,
    recommendations JSONB NOT NULL DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at     TIMESTAMPTZ,
    resolved        BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX IF NOT EXISTS idx_trend_alerts_target ON trend_alerts(target_id);
CREATE INDEX IF NOT EXISTS idx_trend_alerts_severity ON trend_alerts(severity);
CREATE INDEX IF NOT EXISTS idx_trend_alerts_type ON trend_alerts(alert_type);
CREATE INDEX IF NOT EXISTS idx_trend_alerts_active ON trend_alerts(resolved)
    WHERE resolved = FALSE;
CREATE INDEX IF NOT EXISTS idx_trend_alerts_created ON trend_alerts(created_at DESC);

-- ============================================================================
-- Recording Rules Configuration
-- ============================================================================
CREATE TABLE IF NOT EXISTS recording_rules (
    rule_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            TEXT NOT NULL,
    source_metric   TEXT NOT NULL,
    output_metric   TEXT NOT NULL,
    aggregation     TEXT NOT NULL DEFAULT 'avg'
                    CHECK (aggregation IN ('avg', 'sum', 'min', 'max', 'count',
                                           'p50', 'p95', 'p99', 'rate', 'stddev')),
    window_seconds  INT NOT NULL DEFAULT 300,
    group_by        TEXT[] NOT NULL DEFAULT '{}',
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    evaluation_interval_seconds INT NOT NULL DEFAULT 60,
    labels          JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_recording_rules_enabled ON recording_rules(enabled)
    WHERE enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_recording_rules_source ON recording_rules(source_metric);

-- ============================================================================
-- Views
-- ============================================================================

-- Active anomalies summary
CREATE OR REPLACE VIEW active_anomalies AS
SELECT
    ae.org_id,
    ae.target_id,
    t.name AS target_name,
    ae.metric_name,
    ae.resource_id,
    ae.severity,
    ae.z_score,
    ae.observed_value,
    ae.expected_mean,
    ae.description,
    ae.detected_at
FROM anomaly_events ae
JOIN targets t ON ae.target_id = t.target_id
WHERE ae.acknowledged = FALSE
  AND ae.detected_at > now() - INTERVAL '24 hours'
ORDER BY ae.detected_at DESC;

-- Active capacity forecasts that predict a breach
CREATE OR REPLACE VIEW active_forecasts AS
SELECT
    p.org_id,
    p.target_id,
    t.name AS target_name,
    p.metric_name,
    p.resource_id,
    p.current_value,
    p.predicted_value,
    p.rate_per_hour,
    p.hours_to_threshold,
    p.threshold,
    p.confidence,
    p.description,
    p.computed_at
FROM predictions p
JOIN targets t ON p.target_id = t.target_id
WHERE p.hours_to_threshold IS NOT NULL
  AND p.confidence > 0.5
  AND p.computed_at > now() - INTERVAL '6 hours'
ORDER BY p.hours_to_threshold ASC;

-- Prediction summary per org
CREATE OR REPLACE VIEW prediction_summary AS
SELECT
    org_id,
    COUNT(*) AS total_forecasts,
    COUNT(*) FILTER (WHERE hours_to_threshold IS NOT NULL AND hours_to_threshold <= 24) AS breaching_24h,
    COUNT(*) FILTER (WHERE hours_to_threshold IS NOT NULL AND hours_to_threshold <= 168) AS breaching_7d,
    COUNT(DISTINCT target_id) AS affected_targets,
    AVG(confidence) AS avg_confidence
FROM predictions
WHERE computed_at > now() - INTERVAL '6 hours'
GROUP BY org_id;
