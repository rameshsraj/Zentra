-- Zentra Prometheus-compatible Scrape Engine
-- Migration 005: Scrape target configuration and results

-- ============================================================================
-- Scrape Target Configuration
-- ============================================================================
CREATE TABLE IF NOT EXISTS scrape_targets (
    target_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    host            TEXT NOT NULL,
    port            INT NOT NULL DEFAULT 9090,
    scheme          TEXT NOT NULL DEFAULT 'http',
    metrics_path    TEXT NOT NULL DEFAULT '/metrics',
    job_name        TEXT,
    scrape_interval_seconds INT NOT NULL DEFAULT 15,
    scrape_timeout_seconds  INT NOT NULL DEFAULT 10,
    labels          JSONB NOT NULL DEFAULT '{}',
    honor_labels    BOOLEAN NOT NULL DEFAULT FALSE,
    honor_timestamps BOOLEAN NOT NULL DEFAULT TRUE,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    discovered_by   TEXT NOT NULL DEFAULT 'static',
    basic_auth_user TEXT,
    basic_auth_pass TEXT,
    bearer_token    TEXT,
    tls_insecure    BOOLEAN NOT NULL DEFAULT FALSE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_scrape_targets_org ON scrape_targets(org_id);
CREATE INDEX IF NOT EXISTS idx_scrape_targets_enabled ON scrape_targets(enabled)
    WHERE enabled = TRUE;
CREATE INDEX IF NOT EXISTS idx_scrape_targets_job ON scrape_targets(job_name);

-- ============================================================================
-- Scrape Results / Health
-- ============================================================================
CREATE TABLE IF NOT EXISTS scrape_results (
    result_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    target_id       UUID NOT NULL REFERENCES scrape_targets(target_id) ON DELETE CASCADE,
    health          TEXT NOT NULL DEFAULT 'unknown'
                    CHECK (health IN ('up', 'down', 'unknown')),
    sample_count    INT NOT NULL DEFAULT 0,
    scrape_duration_ms DOUBLE PRECISION,
    error_message   TEXT,
    scraped_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_scrape_results_target ON scrape_results(target_id);
CREATE INDEX IF NOT EXISTS idx_scrape_results_scraped ON scrape_results(scraped_at DESC);

-- Index for cleanup queries (plain index — partial index with now() is not allowed)
CREATE INDEX IF NOT EXISTS idx_scrape_results_cleanup ON scrape_results(scraped_at);

-- ============================================================================
-- Scrape Recording Rules
-- ============================================================================
CREATE TABLE IF NOT EXISTS scrape_recording_rules (
    rule_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            TEXT NOT NULL,
    source_metric   TEXT NOT NULL,
    output_metric   TEXT NOT NULL,
    aggregation     TEXT NOT NULL DEFAULT 'sum'
                    CHECK (aggregation IN ('sum', 'avg', 'min', 'max', 'count', 'rate')),
    group_by        TEXT[] NOT NULL DEFAULT '{}',
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_scrape_rules_enabled ON scrape_recording_rules(enabled)
    WHERE enabled = TRUE;

-- ============================================================================
-- Views
-- ============================================================================

-- Scrape targets with latest health
CREATE OR REPLACE VIEW scrape_target_status AS
SELECT
    st.target_id,
    st.org_id,
    st.host,
    st.port,
    st.job_name,
    st.scheme || '://' || st.host || ':' || st.port || st.metrics_path AS url,
    st.enabled,
    st.discovered_by,
    sr.health,
    sr.sample_count,
    sr.scrape_duration_ms,
    sr.error_message,
    sr.scraped_at AS last_scrape
FROM scrape_targets st
LEFT JOIN LATERAL (
    SELECT health, sample_count, scrape_duration_ms, error_message, scraped_at
    FROM scrape_results
    WHERE target_id = st.target_id
    ORDER BY scraped_at DESC
    LIMIT 1
) sr ON true
ORDER BY st.job_name, st.host;

-- Scrape health summary per org
CREATE OR REPLACE VIEW scrape_health_summary AS
SELECT
    st.org_id,
    COUNT(*) AS total_targets,
    COUNT(*) FILTER (WHERE sr.health = 'up') AS targets_up,
    COUNT(*) FILTER (WHERE sr.health = 'down') AS targets_down,
    COUNT(*) FILTER (WHERE sr.health = 'unknown' OR sr.health IS NULL) AS targets_unknown,
    AVG(sr.scrape_duration_ms) AS avg_scrape_duration_ms,
    SUM(sr.sample_count) AS total_samples
FROM scrape_targets st
LEFT JOIN LATERAL (
    SELECT health, scrape_duration_ms, sample_count
    FROM scrape_results
    WHERE target_id = st.target_id
    ORDER BY scraped_at DESC
    LIMIT 1
) sr ON true
WHERE st.enabled = TRUE
GROUP BY st.org_id;
