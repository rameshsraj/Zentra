-- OTel P10: OTel-Native Alerting from Signals
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS signal_source TEXT;
ALTER TABLE alert_rules ADD COLUMN IF NOT EXISTS otel_query JSONB;
