-- ═══════════════════════════════════════════════════════════════════════════
-- 023: Add org_id to user_module_access + Sub-Module Menu Item Access
-- ═══════════════════════════════════════════════════════════════════════════
-- 1. Add org_id column to user_module_access for tenant isolation
-- 2. Backfill existing rows with org_id from the users table
-- 3. Update unique constraint to include org_id
-- 4. Seed sub-module menu items per user
--
-- Convention:
--   module_id without dot = top-level module  (e.g. 'infrastructure')
--   module_id with dot    = sub-menu item     (e.g. 'infrastructure.servers')
-- Rule:
--   If a user has the module but NO sub-items → they see ALL children
--   If sub-items exist for that module → only those children are shown
-- ═══════════════════════════════════════════════════════════════════════════

-- Step 1: Add org_id column
ALTER TABLE user_module_access ADD COLUMN IF NOT EXISTS org_id UUID;

-- Step 2: Backfill org_id from users table
UPDATE user_module_access uma
SET org_id = u.org_id
FROM users u
WHERE uma.user_id = u.user_id AND uma.org_id IS NULL;

-- Step 3: Default org_id for any remaining NULLs
UPDATE user_module_access
SET org_id = '00000000-0000-0000-0000-000000000001'
WHERE org_id IS NULL;

-- Step 4: Make org_id NOT NULL going forward
ALTER TABLE user_module_access ALTER COLUMN org_id SET DEFAULT '00000000-0000-0000-0000-000000000001';

-- Step 5: Drop old unique and create new one scoped to org_id
ALTER TABLE user_module_access DROP CONSTRAINT IF EXISTS user_module_access_user_id_module_id_key;
ALTER TABLE user_module_access ADD CONSTRAINT user_module_access_org_user_module_key UNIQUE (org_id, user_id, module_id);

-- Step 6: Add org_id index
CREATE INDEX IF NOT EXISTS idx_user_module_access_org ON user_module_access(org_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- Seed sub-module menu items
-- ═══════════════════════════════════════════════════════════════════════════

DO $$
DECLARE
    v_org_id UUID := '00000000-0000-0000-0000-000000000001';
    v_satheesh_id UUID;
    v_jimmy_id UUID;
    v_anand_id UUID;
    v_admin_id UUID;
BEGIN
    -- Resolve user IDs
    SELECT id INTO v_admin_id FROM users WHERE email = 'admin@zentra.local' AND org_id = v_org_id LIMIT 1;
    IF v_admin_id IS NULL THEN
        SELECT id INTO v_admin_id FROM users WHERE username = 'admin' AND org_id = v_org_id LIMIT 1;
    END IF;

    SELECT id INTO v_satheesh_id FROM users WHERE email = 'satheesh@zentra.local' AND org_id = v_org_id LIMIT 1;
    SELECT id INTO v_jimmy_id FROM users WHERE email = 'jimmy@zentra.local' AND org_id = v_org_id LIMIT 1;
    SELECT id INTO v_anand_id FROM users WHERE email = 'anand@zentra.local' AND org_id = v_org_id LIMIT 1;

    -- ═══════════════════════════════════════════════════════════════════
    -- satheesh: command_center, infrastructure, database, middleware, alerts_events
    --   Infrastructure: Dashboard + Servers + Network (server/network team)
    --   Database: Dashboard + Oracle (all DB items)
    --   Middleware: no sub-items → sees all
    --   Alerts: Active Alerts + Channels (no Rules editing)
    -- ═══════════════════════════════════════════════════════════════════
    IF v_satheesh_id IS NOT NULL THEN
        INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
            (v_org_id, v_satheesh_id, 'infrastructure.dashboard', v_admin_id),
            (v_org_id, v_satheesh_id, 'infrastructure.servers', v_admin_id),
            (v_org_id, v_satheesh_id, 'infrastructure.network', v_admin_id),
            (v_org_id, v_satheesh_id, 'database.dashboard', v_admin_id),
            (v_org_id, v_satheesh_id, 'database.oracle', v_admin_id),
            (v_org_id, v_satheesh_id, 'alerts_events.active', v_admin_id),
            (v_org_id, v_satheesh_id, 'alerts_events.channels', v_admin_id)
        ON CONFLICT (org_id, user_id, module_id) DO NOTHING;
    END IF;

    -- ═══════════════════════════════════════════════════════════════════
    -- jimmy: database, middleware, security, logs, alerts_events
    --   Database: Dashboard + Oracle
    --   Security: Dashboard + Firewall (no Threat Hunting)
    --   Logs: Explorer + Alerts (no Sources, no Analytics)
    --   Middleware: no sub-items → sees all
    --   Alerts: no sub-items → sees all
    -- ═══════════════════════════════════════════════════════════════════
    IF v_jimmy_id IS NOT NULL THEN
        INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
            (v_org_id, v_jimmy_id, 'database.dashboard', v_admin_id),
            (v_org_id, v_jimmy_id, 'database.oracle', v_admin_id),
            (v_org_id, v_jimmy_id, 'security.dashboard', v_admin_id),
            (v_org_id, v_jimmy_id, 'security.firewall', v_admin_id),
            (v_org_id, v_jimmy_id, 'logs.explorer', v_admin_id),
            (v_org_id, v_jimmy_id, 'logs.alerts', v_admin_id)
        ON CONFLICT (org_id, user_id, module_id) DO NOTHING;
    END IF;

    -- ═══════════════════════════════════════════════════════════════════
    -- anand: command_center, infrastructure, logs, alerts_events, ai_analytics, opentelemetry
    --   Infrastructure: Dashboard + Storage (storage team)
    --   Logs: no sub-items → sees all
    --   AI & Analytics: Insights + Predictive + RCA (no RunBooks/LLM/Profiling)
    --   OpenTelemetry: Health + Traces + Span Metrics (focused)
    --   Alerts: no sub-items → sees all
    -- ═══════════════════════════════════════════════════════════════════
    IF v_anand_id IS NOT NULL THEN
        INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
            (v_org_id, v_anand_id, 'infrastructure.dashboard', v_admin_id),
            (v_org_id, v_anand_id, 'infrastructure.storage', v_admin_id),
            (v_org_id, v_anand_id, 'ai_analytics.insights', v_admin_id),
            (v_org_id, v_anand_id, 'ai_analytics.predictive', v_admin_id),
            (v_org_id, v_anand_id, 'ai_analytics.rca', v_admin_id),
            (v_org_id, v_anand_id, 'opentelemetry.health', v_admin_id),
            (v_org_id, v_anand_id, 'opentelemetry.traces', v_admin_id),
            (v_org_id, v_anand_id, 'opentelemetry.span_metrics', v_admin_id)
        ON CONFLICT (org_id, user_id, module_id) DO NOTHING;
    END IF;

    -- Admin: no sub-items → isAdmin flag grants everything

    RAISE NOTICE 'Sub-module menu items seeded (org=%): satheesh=%, jimmy=%, anand=%',
        v_org_id, v_satheesh_id, v_jimmy_id, v_anand_id;
END $$;
