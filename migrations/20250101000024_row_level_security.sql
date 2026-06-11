-- Row-Level Security for multi-tenant isolation (zentra database)
-- NOTE: RLS on TimescaleDB hypertables with columnstore (metrics, logs) may fail
-- on newer TimescaleDB versions — wrapped in exception blocks for safety.

-- Create an application role for Zentra API connections
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'zentra_app') THEN
        CREATE ROLE zentra_app;
    END IF;
END $$;

-- Enable RLS on core tenant-scoped tables
ALTER TABLE targets ENABLE ROW LEVEL SECURITY;
ALTER TABLE alerts ENABLE ROW LEVEL SECURITY;
ALTER TABLE alert_rules ENABLE ROW LEVEL SECURITY;
ALTER TABLE dashboards ENABLE ROW LEVEL SECURITY;

-- Hypertables: RLS may fail with columnstore — skip gracefully
DO $$ BEGIN
    ALTER TABLE metrics ENABLE ROW LEVEL SECURITY;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'metrics RLS skipped: %', SQLERRM;
END $$;

DO $$ BEGIN
    ALTER TABLE logs ENABLE ROW LEVEL SECURITY;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'logs RLS skipped: %', SQLERRM;
END $$;

-- Create policies (only if they don't exist)
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_targets') THEN
        CREATE POLICY tenant_isolation_targets ON targets
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

-- Hypertable policies — skip gracefully if RLS wasn't enabled above
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_metrics') THEN
        CREATE POLICY tenant_isolation_metrics ON metrics
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'metrics policy skipped: %', SQLERRM;
END $$;

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE policyname = 'tenant_isolation_logs') THEN
        CREATE POLICY tenant_isolation_logs ON logs
            USING (org_id = current_setting('app.current_org_id')::uuid);
    END IF;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'logs policy skipped: %', SQLERRM;
END $$;
