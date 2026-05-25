-- ============================================================================
-- 014_oracle_monitoring.sql — Oracle native connection pool + metric framework
-- V5 Prompt 14: Oracle SQL Schema + Native Connection Pool
-- ============================================================================

-- Oracle Connections (native pool, replaces SSH+sqlplus)
CREATE TABLE IF NOT EXISTS oracle_connections (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(org_id),
    name            TEXT NOT NULL,
    host            TEXT NOT NULL,
    port            INT NOT NULL DEFAULT 1521,
    service_name    TEXT NOT NULL DEFAULT 'ORCL',
    username        TEXT NOT NULL DEFAULT 'system',
    auth_mode       TEXT NOT NULL DEFAULT 'password'
                    CHECK (auth_mode IN ('password','wallet','external','vault')),
    password_enc    TEXT,
    vault_provider  TEXT,
    vault_config    JSONB DEFAULT '{}',
    role            TEXT DEFAULT 'DEFAULT'
                    CHECK (role IN ('DEFAULT','SYSDBA','SYSOPER','SYSASM','SYSDG','SYSKM')),
    tns_admin_path  TEXT,
    pool_min        INT NOT NULL DEFAULT 2,
    pool_max        INT NOT NULL DEFAULT 10,
    pool_increment  INT NOT NULL DEFAULT 1,
    conn_max_lifetime_secs INT NOT NULL DEFAULT 3600,
    query_timeout_secs     INT NOT NULL DEFAULT 30,
    labels          JSONB DEFAULT '{}',
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending','connected','error','disabled')),
    last_ping       TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oracle_conn_org ON oracle_connections(org_id);
CREATE INDEX IF NOT EXISTS idx_oracle_conn_status ON oracle_connections(status);

-- Oracle Metric Definitions (scrape config, inspired by oracledb_exporter)
CREATE TABLE IF NOT EXISTS oracle_metric_definitions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id            UUID NOT NULL REFERENCES organizations(org_id),
    context           TEXT NOT NULL,
    labels            TEXT[] DEFAULT '{}',
    metrics_desc      JSONB NOT NULL DEFAULT '{}',
    metrics_type      JSONB NOT NULL DEFAULT '{}',
    metrics_buckets   JSONB DEFAULT '{}',
    field_to_append   TEXT,
    request           TEXT NOT NULL,
    ignore_zero_result BOOLEAN NOT NULL DEFAULT FALSE,
    query_timeout     INT DEFAULT 30,
    scrape_interval   INT DEFAULT 60,
    databases         TEXT[] DEFAULT '{}',
    is_default        BOOLEAN NOT NULL DEFAULT FALSE,
    is_active         BOOLEAN NOT NULL DEFAULT TRUE,
    source_file       TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oracle_metdef_org ON oracle_metric_definitions(org_id);
CREATE INDEX IF NOT EXISTS idx_oracle_metdef_ctx ON oracle_metric_definitions(context);

-- Oracle Metric Cache (latest scraped values per connection+metric)
CREATE TABLE IF NOT EXISTS oracle_metric_cache (
    connection_id   UUID NOT NULL REFERENCES oracle_connections(id) ON DELETE CASCADE,
    metric_id       UUID NOT NULL REFERENCES oracle_metric_definitions(id) ON DELETE CASCADE,
    cached_value    JSONB NOT NULL DEFAULT '{}',
    scraped_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    scrape_duration_ms INT,
    error           TEXT,
    PRIMARY KEY (connection_id, metric_id)
);

CREATE INDEX IF NOT EXISTS idx_oracle_cache_scraped ON oracle_metric_cache(scraped_at);

-- Seed default metric definitions (core Oracle metrics)
INSERT INTO oracle_metric_definitions (id, org_id, context, labels, metrics_desc, metrics_type, request, is_default, source_file)
VALUES
    (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 'sessions',
     ARRAY['status','type'], '{"value":"Total sessions"}'::jsonb, '{"value":"gauge"}'::jsonb,
     'SELECT status, type, COUNT(*) AS value FROM v$session GROUP BY status, type',
     TRUE, 'default-metrics.toml'),

    (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 'resource',
     ARRAY['resource_name'], '{"current_utilization":"Current utilization","max_utilization":"Max utilization","initial_allocation":"Initial allocation","limit_value":"Limit"}'::jsonb,
     '{"current_utilization":"gauge","max_utilization":"gauge","initial_allocation":"gauge","limit_value":"gauge"}'::jsonb,
     'SELECT resource_name, current_utilization, max_utilization, initial_allocation, limit_value FROM v$resource_limit',
     TRUE, 'default-metrics.toml'),

    (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 'wait_class',
     ARRAY['wait_class'], '{"total_waits":"Total waits","time_waited":"Time waited (hundredths of a second)"}'::jsonb,
     '{"total_waits":"counter","time_waited":"counter"}'::jsonb,
     'SELECT wait_class, SUM(total_waits) AS total_waits, SUM(time_waited) AS time_waited FROM v$system_wait_class WHERE wait_class != ''Idle'' GROUP BY wait_class',
     TRUE, 'default-metrics.toml'),

    (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 'tablespace',
     ARRAY['tablespace_name'], '{"bytes":"Size in bytes","max_bytes":"Max size in bytes","free_bytes":"Free bytes","used_percent":"Pct used"}'::jsonb,
     '{"bytes":"gauge","max_bytes":"gauge","free_bytes":"gauge","used_percent":"gauge"}'::jsonb,
     'SELECT dt.tablespace_name, dt.bytes, dt.max_bytes, df.free_bytes, ROUND((1-df.free_bytes/NULLIF(dt.bytes,0))*100,2) AS used_percent FROM (SELECT tablespace_name, SUM(bytes) bytes, SUM(GREATEST(bytes,maxbytes)) max_bytes FROM dba_data_files GROUP BY tablespace_name) dt LEFT JOIN (SELECT tablespace_name, SUM(bytes) free_bytes FROM dba_free_space GROUP BY tablespace_name) df ON dt.tablespace_name=df.tablespace_name',
     TRUE, 'default-metrics.toml'),

    (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 'asm_diskgroup',
     ARRAY['name'], '{"total_mb":"Total MB","usable_mb":"Usable MB","offline_disks":"Offline disks"}'::jsonb,
     '{"total_mb":"gauge","usable_mb":"gauge","offline_disks":"gauge"}'::jsonb,
     'SELECT name, total_mb, usable_file_mb AS usable_mb, offline_disks FROM v$asm_diskgroup',
     TRUE, 'default-metrics.toml'),

    (gen_random_uuid(), '00000000-0000-0000-0000-000000000001', 'activity',
     ARRAY['name'], '{"value":"Activity metric value"}'::jsonb, '{"value":"counter"}'::jsonb,
     'SELECT name, value FROM v$sysstat WHERE name IN (''db block gets from cache'',''consistent gets from cache'',''physical reads'',''physical writes'',''redo writes'',''parse count (total)'',''parse count (hard)'',''user commits'',''user rollbacks'')',
     TRUE, 'default-metrics.toml')
ON CONFLICT DO NOTHING;
