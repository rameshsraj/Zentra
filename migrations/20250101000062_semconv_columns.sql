-- OTel P05: Semantic Convention Enrichment — indexed columns on spans/traces
-- Wrapped in DO block: the base table may be called 'traces' (fresh install)
-- or 'spans' (upgrade path). Skip entirely if neither exists yet.
DO $$
DECLARE
    tbl TEXT;
BEGIN
    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'traces' AND schemaname = 'public') THEN
        tbl := 'traces';
    ELSIF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'spans' AND schemaname = 'public') THEN
        tbl := 'spans';
    ELSE
        RAISE NOTICE 'semconv_columns: neither traces nor spans table exists — skipping';
        RETURN;
    END IF;

    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS http_method      TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS http_status_code INTEGER', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS http_route       TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS db_system        TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS db_name          TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS db_operation     TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS rpc_system       TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS rpc_service      TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS rpc_method       TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS messaging_system TEXT', tbl);
    EXECUTE format('ALTER TABLE %I ADD COLUMN IF NOT EXISTS peer_service     TEXT', tbl);

    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_spans_http_method ON %I(http_method) WHERE http_method IS NOT NULL', tbl);
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_spans_db_system ON %I(db_system) WHERE db_system IS NOT NULL', tbl);
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_spans_http_status ON %I(http_status_code) WHERE http_status_code IS NOT NULL', tbl);
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_spans_rpc_system ON %I(rpc_system) WHERE rpc_system IS NOT NULL', tbl);
    EXECUTE format('CREATE INDEX IF NOT EXISTS idx_spans_tags_gin ON %I USING GIN(tags jsonb_path_ops)', tbl);
END $$;
