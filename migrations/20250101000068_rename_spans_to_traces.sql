-- Rename 'spans' table to 'traces' for consistency with OTel nomenclature.
-- Fully idempotent:
--   • Fresh install: 'traces' already exists as base table, 'spans' is a VIEW → nothing to do
--   • Upgrade path:  'spans' is a base table, 'traces' doesn't exist → rename

DO $$ BEGIN
    -- Only rename if 'spans' is a real BASE TABLE (not a view) and 'traces' doesn't exist
    IF EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'spans' AND schemaname = 'public')
       AND NOT EXISTS (SELECT 1 FROM pg_tables WHERE tablename = 'traces' AND schemaname = 'public') THEN
        ALTER TABLE spans RENAME TO traces;
    ELSE
        RAISE NOTICE 'rename_spans_to_traces: nothing to do (traces already exists or spans is a view)';
    END IF;
END $$;

-- Update associated indexes (skip if already renamed or not present)
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'spans_pkey') THEN ALTER INDEX spans_pkey RENAME TO traces_pkey; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_trace_id') THEN ALTER INDEX idx_spans_trace_id RENAME TO idx_traces_trace_id; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_service_name') THEN ALTER INDEX idx_spans_service_name RENAME TO idx_traces_service_name; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_http_method') THEN ALTER INDEX idx_spans_http_method RENAME TO idx_traces_http_method; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_db_system') THEN ALTER INDEX idx_spans_db_system RENAME TO idx_traces_db_system; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_http_status') THEN ALTER INDEX idx_spans_http_status RENAME TO idx_traces_http_status; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_rpc_system') THEN ALTER INDEX idx_spans_rpc_system RENAME TO idx_traces_rpc_system; END IF;
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_spans_tags_gin') THEN ALTER INDEX idx_spans_tags_gin RENAME TO idx_traces_tags_gin; END IF;
END $$;
