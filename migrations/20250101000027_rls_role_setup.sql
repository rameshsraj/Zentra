-- Migration 027: RLS Role Setup
-- Creates the zentra_app role and grants permissions for Row-Level Security.
-- The API server must SET LOCAL app.current_org_id on each connection for RLS policies to work.

-- 1. Create the application role if it doesn't exist
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'zentra_app') THEN
        CREATE ROLE zentra_app LOGIN PASSWORD 'zentra_app_change_me';
    END IF;
END $$;

-- 2. Grant permissions on existing objects
GRANT USAGE ON SCHEMA public TO zentra_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO zentra_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO zentra_app;

-- 3. Default privileges for future objects
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO zentra_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public
    GRANT USAGE, SELECT ON SEQUENCES TO zentra_app;

-- 4. Allow zentra_app to set session variables (needed for RLS)
GRANT SET ON PARAMETER app.current_org_id TO zentra_app;
