-- NetFlow / IPFIX / sFlow records table
CREATE TABLE IF NOT EXISTS netflow_records (
    id              UUID NOT NULL DEFAULT uuid_generate_v4(),
    received_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    exporter_ip     INET NOT NULL,
    src_ip          INET NOT NULL,
    dst_ip          INET NOT NULL,
    src_port        INT,
    dst_port        INT,
    protocol        SMALLINT,
    protocol_name   TEXT,
    packets         BIGINT,
    bytes           BIGINT,
    duration_ms     BIGINT,
    tcp_flags       INT,
    tos             INT,
    src_as          BIGINT,
    dst_as          BIGINT,
    next_hop        INET,
    flow_version    SMALLINT,  -- 5, 9, 10=IPFIX, 50=sFlow
    extra           JSONB
);

SELECT create_hypertable('netflow_records', 'received_at', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_netflow_exporter   ON netflow_records (exporter_ip, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_netflow_src        ON netflow_records (src_ip, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_netflow_dst        ON netflow_records (dst_ip, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_netflow_proto      ON netflow_records (protocol, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_netflow_bytes      ON netflow_records (bytes DESC, received_at DESC);

-- Compression: compress after 1 day, retain 90 days
SELECT add_compression_policy('netflow_records', INTERVAL '1 day', if_not_exists => TRUE);
SELECT add_retention_policy('netflow_records', INTERVAL '90 days', if_not_exists => TRUE);

-- Pre-aggregated TopN views for fast dashboard queries
CREATE MATERIALIZED VIEW IF NOT EXISTS netflow_topn_talkers_hourly AS
SELECT
    time_bucket('1 hour', received_at) AS bucket,
    src_ip,
    dst_ip,
    protocol_name,
    SUM(bytes) AS total_bytes,
    SUM(packets) AS total_packets,
    COUNT(*) AS flow_count
FROM netflow_records
WHERE received_at >= NOW() - INTERVAL '24 hours'
GROUP BY bucket, src_ip, dst_ip, protocol_name
ORDER BY total_bytes DESC
WITH NO DATA;

CREATE UNIQUE INDEX IF NOT EXISTS idx_netflow_topn_talkers
    ON netflow_topn_talkers_hourly (bucket, src_ip, dst_ip, protocol_name);

-- sFlow exporters registry (auto-created when sFlow first received)
CREATE TABLE IF NOT EXISTS flow_exporters (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    ip              INET UNIQUE NOT NULL,
    hostname        TEXT,
    flow_type       TEXT NOT NULL DEFAULT 'netflow',  -- netflow | sflow | ipfix
    last_seen       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    packets_rx      BIGINT NOT NULL DEFAULT 0,
    flows_rx        BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_flow_exporters_ip ON flow_exporters (ip);
