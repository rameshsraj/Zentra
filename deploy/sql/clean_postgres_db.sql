-- ═══════════════════════════════════════════════════════════════════════════
-- Clean up 'postgres' database: drop all application tables to ensure isolation
-- ═══════════════════════════════════════════════════════════════════════════

DO $$
DECLARE
    r RECORD;
BEGIN
    FOR r IN (
        SELECT tablename 
        FROM pg_tables 
        WHERE schemaname = 'public' 
          AND tablename NOT IN ('spatial_ref_sys', 'geography_columns', 'geometry_columns') -- Keep PostGIS system tables if any
    ) LOOP
        EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(r.tablename) || ' CASCADE';
    END LOOP;
END $$;
