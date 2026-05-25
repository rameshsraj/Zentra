-- ═══════════════════════════════════════════════════════════════════════════
-- Zentra Network Security Module — E2 Foundation
-- ═══════════════════════════════════════════════════════════════════════════

-- IDS Rules
CREATE TABLE IF NOT EXISTS ids_rules (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    sid         INTEGER UNIQUE NOT NULL,
    category    TEXT NOT NULL DEFAULT 'uncategorized',
    severity    TEXT NOT NULL DEFAULT 'medium' CHECK (severity IN ('low','medium','high','critical')),
    message     TEXT NOT NULL,
    rule_text   TEXT NOT NULL,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    source      TEXT NOT NULL DEFAULT 'custom',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_ids_rules_sid ON ids_rules(sid);
CREATE INDEX IF NOT EXISTS idx_ids_rules_category ON ids_rules(category);
CREATE INDEX IF NOT EXISTS idx_ids_rules_severity ON ids_rules(severity);

-- IDS Alerts
CREATE TABLE IF NOT EXISTS ids_alerts (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id       UUID NOT NULL,
    rule_id      UUID REFERENCES ids_rules(id) ON DELETE SET NULL,
    timestamp    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    src_ip       INET,
    src_port     INTEGER,
    dest_ip      INET,
    dest_port    INTEGER,
    protocol     TEXT,
    payload      TEXT,
    severity     TEXT NOT NULL DEFAULT 'medium',
    status       TEXT NOT NULL DEFAULT 'new' CHECK (status IN ('new','acknowledged','closed','false_positive')),
    assigned_to  UUID,
    community_id TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_ids_alerts_timestamp ON ids_alerts(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_ids_alerts_src_ip ON ids_alerts(src_ip);
CREATE INDEX IF NOT EXISTS idx_ids_alerts_dest_ip ON ids_alerts(dest_ip);
CREATE INDEX IF NOT EXISTS idx_ids_alerts_severity ON ids_alerts(severity);
CREATE INDEX IF NOT EXISTS idx_ids_alerts_status ON ids_alerts(status);
CREATE INDEX IF NOT EXISTS idx_ids_alerts_community_id ON ids_alerts(community_id);

-- Connection Logs
CREATE TABLE IF NOT EXISTS connection_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uid         TEXT,
    src_ip      INET,
    src_port    INTEGER,
    dest_ip     INET,
    dest_port   INTEGER,
    proto       TEXT,
    service     TEXT,
    duration    DOUBLE PRECISION,
    bytes_orig  BIGINT DEFAULT 0,
    bytes_resp  BIGINT DEFAULT 0,
    pkts_orig   BIGINT DEFAULT 0,
    pkts_resp   BIGINT DEFAULT 0,
    conn_state  TEXT,
    community_id TEXT,
    geo_src     TEXT,
    geo_dest    TEXT
);
CREATE INDEX IF NOT EXISTS idx_conn_logs_timestamp ON connection_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_conn_logs_src_ip ON connection_logs(src_ip);
CREATE INDEX IF NOT EXISTS idx_conn_logs_dest_ip ON connection_logs(dest_ip);

-- DNS Logs
CREATE TABLE IF NOT EXISTS dns_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uid         TEXT,
    src_ip      INET,
    query       TEXT,
    qtype       TEXT,
    rcode       TEXT,
    answers     JSONB DEFAULT '[]',
    ttl         INTEGER,
    rejected    BOOLEAN DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_dns_logs_timestamp ON dns_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_dns_logs_query ON dns_logs(query);
CREATE INDEX IF NOT EXISTS idx_dns_logs_src_ip ON dns_logs(src_ip);

-- HTTP Logs
CREATE TABLE IF NOT EXISTS http_logs (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id           UUID NOT NULL,
    timestamp        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uid              TEXT,
    src_ip           INET,
    dest_ip          INET,
    method           TEXT,
    host             TEXT,
    uri              TEXT,
    status_code      INTEGER,
    user_agent       TEXT,
    referrer         TEXT,
    mime_type        TEXT,
    request_body_len  BIGINT DEFAULT 0,
    response_body_len BIGINT DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_http_logs_timestamp ON http_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_http_logs_host ON http_logs(host);
CREATE INDEX IF NOT EXISTS idx_http_logs_status ON http_logs(status_code);

-- SSL/TLS Logs
CREATE TABLE IF NOT EXISTS ssl_logs (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id           UUID NOT NULL,
    timestamp        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uid              TEXT,
    src_ip           INET,
    dest_ip          INET,
    server_name      TEXT,
    version          TEXT,
    cipher           TEXT,
    ja3              TEXT,
    ja3s             TEXT,
    cert_chain       JSONB DEFAULT '[]',
    validation_status TEXT,
    not_valid_before TIMESTAMPTZ,
    not_valid_after  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_ssl_logs_timestamp ON ssl_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_ssl_logs_ja3 ON ssl_logs(ja3);
CREATE INDEX IF NOT EXISTS idx_ssl_logs_server_name ON ssl_logs(server_name);

-- SSH Logs
CREATE TABLE IF NOT EXISTS ssh_logs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id         UUID NOT NULL,
    timestamp      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    uid            TEXT,
    src_ip         INET,
    dest_ip        INET,
    auth_attempts  INTEGER DEFAULT 0,
    client         TEXT,
    server         TEXT,
    hassh          TEXT,
    success        BOOLEAN DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_ssh_logs_timestamp ON ssh_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_ssh_logs_hassh ON ssh_logs(hassh);

-- File Analysis
CREATE TABLE IF NOT EXISTS file_analysis (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        UUID NOT NULL,
    timestamp     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    filename      TEXT,
    size          BIGINT DEFAULT 0,
    md5           TEXT,
    sha1          TEXT,
    sha256        TEXT,
    mime_type     TEXT,
    yara_matches  JSONB DEFAULT '[]',
    verdict       TEXT DEFAULT 'unknown' CHECK (verdict IN ('clean','suspicious','malicious','unknown')),
    source        TEXT
);
CREATE INDEX IF NOT EXISTS idx_file_analysis_sha256 ON file_analysis(sha256);
CREATE INDEX IF NOT EXISTS idx_file_analysis_verdict ON file_analysis(verdict);

-- SOC Cases
CREATE TABLE IF NOT EXISTS soc_cases (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','investigating','resolved','closed')),
    priority    TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('low','medium','high','critical')),
    assignee    UUID,
    alert_count INTEGER DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_soc_cases_status ON soc_cases(status);

-- Case Events
CREATE TABLE IF NOT EXISTS case_events (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id     UUID NOT NULL REFERENCES soc_cases(id) ON DELETE CASCADE,
    event_type  TEXT NOT NULL,
    content     JSONB DEFAULT '{}',
    user_id     UUID,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_case_events_case ON case_events(case_id);

-- IOC Lists
CREATE TABLE IF NOT EXISTS ioc_lists (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    name        TEXT NOT NULL,
    type        TEXT NOT NULL CHECK (type IN ('ip','domain','hash','url')),
    description TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- IOC Entries
CREATE TABLE IF NOT EXISTS ioc_entries (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id     UUID NOT NULL REFERENCES ioc_lists(id) ON DELETE CASCADE,
    value       TEXT NOT NULL,
    added_by    UUID,
    added_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_ioc_entries_value ON ioc_entries(value);
CREATE INDEX IF NOT EXISTS idx_ioc_entries_list ON ioc_entries(list_id);

-- Firewall Logs
CREATE TABLE IF NOT EXISTS firewall_logs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    src_ip      INET,
    src_port    INTEGER,
    dest_ip     INET,
    dest_port   INTEGER,
    protocol    TEXT,
    action      TEXT CHECK (action IN ('allow','deny','drop','reject')),
    rule        TEXT,
    interface   TEXT,
    direction   TEXT CHECK (direction IN ('inbound','outbound'))
);
CREATE INDEX IF NOT EXISTS idx_fw_logs_timestamp ON firewall_logs(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_fw_logs_action ON firewall_logs(action);
CREATE INDEX IF NOT EXISTS idx_fw_logs_src_ip ON firewall_logs(src_ip);

-- Threat Intel Cache
CREATE TABLE IF NOT EXISTS threat_intel_cache (
    indicator   TEXT NOT NULL,
    type        TEXT NOT NULL,
    source      TEXT NOT NULL,
    result      JSONB DEFAULT '{}',
    queried_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at  TIMESTAMPTZ,
    PRIMARY KEY (indicator, type, source)
);
CREATE INDEX IF NOT EXISTS idx_threat_intel_expires ON threat_intel_cache(expires_at);
