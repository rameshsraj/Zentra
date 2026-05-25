-- SkyWalking P06: Hubble gRPC Flow Collector
CREATE INDEX IF NOT EXISTS idx_netflow_hubble ON netflow_records(flow_version, received_at DESC) WHERE flow_version = 100;

CREATE OR REPLACE VIEW hubble_l7_flows AS
SELECT received_at, src_ip, src_port, dst_ip, dst_port, protocol_name,
    extra->>'verdict'           AS verdict,
    extra->>'drop_reason'       AS drop_reason,
    extra->'l7'->>'type'        AS l7_type,
    extra->'l7'->>'method'      AS http_method,
    extra->'l7'->>'url'         AS http_url,
    extra->'l7'->>'status_code' AS http_status,
    extra->'l7'->>'query'       AS dns_query,
    extra->'l7'->>'topic'       AS kafka_topic
FROM netflow_records WHERE flow_version = 100;
