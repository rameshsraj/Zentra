-- Rename 'spans' table to 'traces' for consistency with OTel nomenclature
-- and to match the 'trace_id' primary identifier.

ALTER TABLE spans RENAME TO traces;

-- Update associated indexes
ALTER INDEX IF EXISTS spans_pkey RENAME TO traces_pkey;
ALTER INDEX IF EXISTS idx_spans_trace_id RENAME TO idx_traces_trace_id;
ALTER INDEX IF EXISTS idx_spans_service_name RENAME TO idx_traces_service_name;
ALTER INDEX IF EXISTS idx_spans_http_method RENAME TO idx_traces_http_method;
ALTER INDEX IF EXISTS idx_spans_db_system RENAME TO idx_traces_db_system;
ALTER INDEX IF EXISTS idx_spans_http_status RENAME TO idx_traces_http_status;
ALTER INDEX IF EXISTS idx_spans_rpc_system RENAME TO idx_traces_rpc_system;
ALTER INDEX IF EXISTS idx_spans_tags_gin RENAME TO idx_traces_tags_gin;

-- Update the materialized view definition if necessary (PostgreSQL usually handles this automatically
-- but if we recreate it later we should use the new name).
