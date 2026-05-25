-- Row-Level Security for multi-tenant isolation (zentra database)
-- This adds defense-in-depth: even if application code forgets WHERE org_id = $1,
-- the database will enforce it.
--
-- NOTE: This migration creates RLS policies but does NOT enforce them on the
-- 'postgres' superuser role (which is what the app currently uses). The policies
-- will only take effect when the app connects as 'zentra_app' role — this is a
-- FUTURE migration to switch connection roles. The app currently connects as
-- superuser, so existing queries continue to work unchanged.

-- Create an application role for Zentra API connections
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'zentra_app') THEN
        CREATE ROLE zentra_app;
    END IF;
END $$;

-- Enable RLS on core tenant-scoped tables
ALTER TABLE targets ENABLE ROW LEVEL SECURITY;
ALTER TABLE metrics ENABLE ROW LEVEL SECURITY;
ALTER TABLE logs ENABLE ROW LEVEL SECURITY;
ALTER TABLE alerts ENABLE ROW LEVEL SECURITY;
ALTER TABLE alert_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE dashboards ENABLE ROW LEVEL SECURITY;

-- Create policies (only if they don't exist)
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_targets') THEN
        CREATE POLICY tenant_isolation_targets ON targets
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_metrics') THEN
        CREATE POLICY tenant_isolation_metrics ON metrics
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_logs') THEN
        CREATE POLICY tenant_isolation_logs ON logs
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_alerts') THEN
        CREATE POLICY tenant_isolation_alerts ON alerts
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_alert_rules') THEN
        CREATE POLICY tenant_isolation_alert_rules ON alert_rules
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_dashboards') THEN
        CREATE POLICY tenant_isolation_dashboards ON dashboards
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
END $$;
