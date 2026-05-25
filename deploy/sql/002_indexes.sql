-- Zentra Index Strategy v1.0
-- Optimized for drill-down queries, real-time dashboards, and log search

-- ============================================================================
-- TARGETS indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_targets_org_id ON targets(org_id);
CREATE INDEX IF NOT EXISTS idx_targets_org_type ON targets(org_id, target_type);
CREATE INDEX IF NOT EXISTS idx_targets_org_active ON targets(org_id, is_active);
CREATE INDEX IF NOT EXISTS idx_targets_status ON targets(status);

-- ============================================================================
-- METRICS indexes (on hypertable — TimescaleDB auto-creates time index)
-- ============================================================================
-- Composite indexes for common drill-down queries
CREATE INDEX IF NOT EXISTS idx_metrics_org_name ON metrics(org_id, metric_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_target ON metrics(target_id, metric_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_resource ON metrics(resource_id, metric_name, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_plugin ON metrics(plugin_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_resource_type ON metrics(resource_type, timestamp DESC);

-- ============================================================================
-- LOGS indexes (on hypertable)
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_logs_org_time ON logs(org_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_target ON logs(target_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_severity ON logs(org_id, severity, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_source ON logs(org_id, source, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_logs_component ON logs(component, timestamp DESC);

-- Full-text search index on the generated tsvector column
CREATE INDEX IF NOT EXISTS idx_logs_message_fts ON logs USING GIN(message_tsv);

-- Trigram index for LIKE/ILIKE queries
CREATE INDEX IF NOT EXISTS idx_logs_message_trgm ON logs USING GIN(message gin_trgm_ops);

-- ============================================================================
-- ALERTS indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_alerts_org_status ON alerts(org_id, status);
CREATE INDEX IF NOT EXISTS idx_alerts_org_severity ON alerts(org_id, severity);
CREATE INDEX IF NOT EXISTS idx_alerts_triggered ON alerts(triggered_at DESC);
CREATE INDEX IF NOT EXISTS idx_alerts_target ON alerts(target_id);
CREATE INDEX IF NOT EXISTS idx_alerts_rule ON alerts(rule_id);

-- ============================================================================
-- ALERT_RULES indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_alert_rules_org ON alert_rules(org_id);
CREATE INDEX IF NOT EXISTS idx_alert_rules_metric ON alert_rules(metric_name);
CREATE INDEX IF NOT EXISTS idx_alert_rules_enabled ON alert_rules(org_id, enabled);

-- ============================================================================
-- ALERT_ROUTING indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_alert_routing_rule ON alert_routing(rule_id);
CREATE INDEX IF NOT EXISTS idx_alert_routing_org ON alert_routing(org_id);

-- ============================================================================
-- EVENTS indexes (on hypertable)
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type, timestamp DESC);

-- ============================================================================
-- NETWORK_FLOWS indexes (on hypertable)
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_flows_org_time ON network_flows(org_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_flows_target ON network_flows(target_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_flows_src_ip ON network_flows(src_ip, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_flows_dst_ip ON network_flows(dst_ip, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_flows_protocol ON network_flows(protocol, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_flows_service ON network_flows(service, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_flows_direction ON network_flows(direction, timestamp DESC);

-- ============================================================================
-- DRILLDOWN_HIERARCHY indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_drilldown_org ON drilldown_hierarchy(org_id);
CREATE INDEX IF NOT EXISTS idx_drilldown_parent ON drilldown_hierarchy(org_id, parent_type, parent_id);
CREATE INDEX IF NOT EXISTS idx_drilldown_child ON drilldown_hierarchy(org_id, child_type, child_id);

-- ============================================================================
-- DASHBOARDS indexes
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_dashboards_org ON dashboards(org_id);

-- ============================================================================
-- THREATS indexes (on hypertable)
-- ============================================================================
CREATE INDEX IF NOT EXISTS idx_threats_org_time ON threats(org_id, first_seen DESC);
CREATE INDEX IF NOT EXISTS idx_threats_target ON threats(target_id, first_seen DESC);
CREATE INDEX IF NOT EXISTS idx_threats_category ON threats(category, first_seen DESC);
CREATE INDEX IF NOT EXISTS idx_threats_severity ON threats(severity, first_seen DESC);
CREATE INDEX IF NOT EXISTS idx_threats_status ON threats(org_id, status);
CREATE INDEX IF NOT EXISTS idx_threats_source ON threats(source_ip, first_seen DESC);

-- ============================================================================
-- DATA RETENTION POLICIES
-- ============================================================================
-- Metrics: retain 90 days by default
SELECT add_retention_policy('metrics', INTERVAL '90 days', if_not_exists => TRUE);

-- Logs: retain 30 days by default
SELECT add_retention_policy('logs', INTERVAL '30 days', if_not_exists => TRUE);

-- Network flows: retain 14 days by default
SELECT add_retention_policy('network_flows', INTERVAL '14 days', if_not_exists => TRUE);

-- Events: retain 7 days by default
SELECT add_retention_policy('events', INTERVAL '7 days', if_not_exists => TRUE);

-- Threats: retain 90 days by default
SELECT add_retention_policy('threats', INTERVAL '90 days', if_not_exists => TRUE);

-- ============================================================================
-- CONTINUOUS AGGREGATES for dashboard performance
-- ============================================================================
-- 1-minute metric rollup
CREATE MATERIALIZED VIEW IF NOT EXISTS metrics_1m
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 minute', timestamp) AS bucket,
    org_id,
    target_id,
    metric_name,
    resource_type,
    avg(value) AS avg_value,
    max(value) AS max_value,
    min(value) AS min_value,
    count(*) AS sample_count
FROM metrics
GROUP BY 1, 2, 3, 4, 5
WITH NO DATA;

SELECT add_continuous_aggregate_policy('metrics_1m',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute',
    if_not_exists => TRUE
);

-- 1-hour metric rollup
CREATE MATERIALIZED VIEW IF NOT EXISTS metrics_1h
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    org_id,
    target_id,
    metric_name,
    resource_type,
    avg(value) AS avg_value,
    max(value) AS max_value,
    min(value) AS min_value,
    count(*) AS sample_count
FROM metrics
GROUP BY 1, 2, 3, 4, 5
WITH NO DATA;

SELECT add_continuous_aggregate_policy('metrics_1h',
    start_offset => INTERVAL '2 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 1-day metric rollup
CREATE MATERIALIZED VIEW IF NOT EXISTS metrics_1d
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', timestamp) AS bucket,
    org_id,
    target_id,
    metric_name,
    resource_type,
    avg(value) AS avg_value,
    max(value) AS max_value,
    min(value) AS min_value,
    count(*) AS sample_count
FROM metrics
GROUP BY 1, 2, 3, 4, 5
WITH NO DATA;

SELECT add_continuous_aggregate_policy('metrics_1d',
    start_offset => INTERVAL '30 days',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Log severity histogram (1 hour buckets)
CREATE MATERIALIZED VIEW IF NOT EXISTS log_severity_1h
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) AS bucket,
    org_id,
    severity,
    source,
    count(*) AS log_count
FROM logs
GROUP BY 1, 2, 3, 4
WITH NO DATA;

SELECT add_continuous_aggregate_policy('log_severity_1h',
    start_offset => INTERVAL '2 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);
