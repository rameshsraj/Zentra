-- OTel P05: Semantic Convention Enrichment — indexed columns on spans
ALTER TABLE spans ADD COLUMN IF NOT EXISTS http_method      TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS http_status_code INTEGER;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS http_route       TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS db_system        TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS db_name          TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS db_operation     TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS rpc_system       TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS rpc_service      TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS rpc_method       TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS messaging_system TEXT;
ALTER TABLE spans ADD COLUMN IF NOT EXISTS peer_service     TEXT;

CREATE INDEX IF NOT EXISTS idx_spans_http_method ON spans(http_method) WHERE http_method IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_db_system ON spans(db_system) WHERE db_system IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_http_status ON spans(http_status_code) WHERE http_status_code IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_rpc_system ON spans(rpc_system) WHERE rpc_system IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_spans_tags_gin ON spans USING GIN(tags jsonb_path_ops);
