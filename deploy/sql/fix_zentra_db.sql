-- ═══════════════════════════════════════════════════════════════════════════
-- Fix zentra database: users, passwords, module access, rate limiting
-- ═══════════════════════════════════════════════════════════════════════════

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ═══════════════════════════════════════════════════════════════════════════
-- 1. Create user_module_access table if not exists
-- ═══════════════════════════════════════════════════════════════════════════
CREATE TABLE IF NOT EXISTS user_module_access (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000001',
    user_id     UUID NOT NULL,
    module_id   TEXT NOT NULL,
    granted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    granted_by  UUID,
    UNIQUE(org_id, user_id, module_id)
);
CREATE INDEX IF NOT EXISTS idx_user_module_access_user ON user_module_access(user_id);
CREATE INDEX IF NOT EXISTS idx_user_module_access_module ON user_module_access(module_id);
CREATE INDEX IF NOT EXISTS idx_user_module_access_org ON user_module_access(org_id);

-- ═══════════════════════════════════════════════════════════════════════════
-- 2. Fix admin email + create users + set passwords
-- ═══════════════════════════════════════════════════════════════════════════
DO $$
DECLARE
    v_org_id UUID := '00000000-0000-0000-0000-000000000001';
    v_admin_id UUID;
    v_satheesh_id UUID;
    v_jimmy_id UUID;
    v_anand_id UUID;
    v_viewer_role_id UUID;
    v_pw_hash TEXT;
BEGIN
    -- Generate bcrypt hash for Admin@12345
    v_pw_hash := crypt('Admin@12345', gen_salt('bf', 12));

    -- Fix admin email to @zentra.local
    UPDATE users SET email = 'admin@zentra.local' WHERE username = 'admin' AND org_id = v_org_id;

    -- Get admin user id (zentra DB uses 'id' as PK)
    SELECT id INTO v_admin_id FROM users WHERE username = 'admin' AND org_id = v_org_id LIMIT 1;

    -- Set admin password + unlock
    UPDATE users SET password_hash = v_pw_hash, is_locked = false, locked_until = NULL
    WHERE id = v_admin_id;

    -- Get Viewer role
    SELECT role_id INTO v_viewer_role_id FROM roles WHERE name = 'Viewer' AND org_id = v_org_id LIMIT 1;
    IF v_viewer_role_id IS NULL THEN
        v_viewer_role_id := gen_random_uuid();
        INSERT INTO roles (role_id, org_id, name, description, is_system)
        VALUES (v_viewer_role_id, v_org_id, 'Viewer', 'Read-only access', true)
        ON CONFLICT DO NOTHING;
    END IF;

    -- ═══════════════════════════════════════════════════════════════════
    -- Create/update user: satheesh
    -- UNIQUE constraint is (org_id, email) in zentra DB
    -- ═══════════════════════════════════════════════════════════════════
    v_satheesh_id := gen_random_uuid();
    INSERT INTO users (id, org_id, email, username, password_hash, display_name, is_active)
    VALUES (v_satheesh_id, v_org_id, 'satheesh@zentra.local', 'satheesh', v_pw_hash, 'Satheesh', true)
    ON CONFLICT (org_id, email) DO UPDATE SET password_hash = v_pw_hash, display_name = 'Satheesh', is_active = true, is_locked = false, locked_until = NULL
    RETURNING id INTO v_satheesh_id;

    INSERT INTO user_roles (user_id, role_id, org_id) VALUES (v_satheesh_id, v_viewer_role_id, v_org_id)
    ON CONFLICT DO NOTHING;

    -- ═══════════════════════════════════════════════════════════════════
    -- Create/update user: jimmy
    -- ═══════════════════════════════════════════════════════════════════
    v_jimmy_id := gen_random_uuid();
    INSERT INTO users (id, org_id, email, username, password_hash, display_name, is_active)
    VALUES (v_jimmy_id, v_org_id, 'jimmy@zentra.local', 'jimmy', v_pw_hash, 'Jimmy', true)
    ON CONFLICT (org_id, email) DO UPDATE SET password_hash = v_pw_hash, display_name = 'Jimmy', is_active = true, is_locked = false, locked_until = NULL
    RETURNING id INTO v_jimmy_id;

    INSERT INTO user_roles (user_id, role_id, org_id) VALUES (v_jimmy_id, v_viewer_role_id, v_org_id)
    ON CONFLICT DO NOTHING;

    -- ═══════════════════════════════════════════════════════════════════
    -- Create/update user: anand
    -- ═══════════════════════════════════════════════════════════════════
    v_anand_id := gen_random_uuid();
    INSERT INTO users (id, org_id, email, username, password_hash, display_name, is_active)
    VALUES (v_anand_id, v_org_id, 'anand@zentra.local', 'anand', v_pw_hash, 'Anand', true)
    ON CONFLICT (org_id, email) DO UPDATE SET password_hash = v_pw_hash, display_name = 'Anand', is_active = true, is_locked = false, locked_until = NULL
    RETURNING id INTO v_anand_id;

    INSERT INTO user_roles (user_id, role_id, org_id) VALUES (v_anand_id, v_viewer_role_id, v_org_id)
    ON CONFLICT DO NOTHING;

    -- ═══════════════════════════════════════════════════════════════════
    -- Module access: top-level modules
    -- ═══════════════════════════════════════════════════════════════════

    -- satheesh: command_center, infrastructure, database, middleware, alerts_events
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_satheesh_id, 'command_center', v_admin_id),
        (v_org_id, v_satheesh_id, 'infrastructure', v_admin_id),
        (v_org_id, v_satheesh_id, 'database', v_admin_id),
        (v_org_id, v_satheesh_id, 'middleware', v_admin_id),
        (v_org_id, v_satheesh_id, 'alerts_events', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- jimmy: database, middleware, security, logs, alerts_events
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_jimmy_id, 'database', v_admin_id),
        (v_org_id, v_jimmy_id, 'middleware', v_admin_id),
        (v_org_id, v_jimmy_id, 'security', v_admin_id),
        (v_org_id, v_jimmy_id, 'logs', v_admin_id),
        (v_org_id, v_jimmy_id, 'alerts_events', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- anand: command_center, infrastructure, logs, alerts_events, ai_analytics, opentelemetry
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_anand_id, 'command_center', v_admin_id),
        (v_org_id, v_anand_id, 'infrastructure', v_admin_id),
        (v_org_id, v_anand_id, 'logs', v_admin_id),
        (v_org_id, v_anand_id, 'alerts_events', v_admin_id),
        (v_org_id, v_anand_id, 'ai_analytics', v_admin_id),
        (v_org_id, v_anand_id, 'opentelemetry', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- admin: all modules
    IF v_admin_id IS NOT NULL THEN
        INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
            (v_org_id, v_admin_id, 'command_center', v_admin_id),
            (v_org_id, v_admin_id, 'infrastructure', v_admin_id),
            (v_org_id, v_admin_id, 'database', v_admin_id),
            (v_org_id, v_admin_id, 'middleware', v_admin_id),
            (v_org_id, v_admin_id, 'security', v_admin_id),
            (v_org_id, v_admin_id, 'logs', v_admin_id),
            (v_org_id, v_admin_id, 'alerts_events', v_admin_id),
            (v_org_id, v_admin_id, 'ai_analytics', v_admin_id),
            (v_org_id, v_admin_id, 'opentelemetry', v_admin_id),
            (v_org_id, v_admin_id, 'admin', v_admin_id)
        ON CONFLICT (org_id, user_id, module_id) DO NOTHING;
    END IF;

    -- ═══════════════════════════════════════════════════════════════════
    -- Sub-module menu items
    -- ═══════════════════════════════════════════════════════════════════

    -- satheesh: infra(servers+network), db(all), alerts(active+channels)
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_satheesh_id, 'infrastructure.dashboard', v_admin_id),
        (v_org_id, v_satheesh_id, 'infrastructure.servers', v_admin_id),
        (v_org_id, v_satheesh_id, 'infrastructure.network', v_admin_id),
        (v_org_id, v_satheesh_id, 'database.dashboard', v_admin_id),
        (v_org_id, v_satheesh_id, 'database.oracle', v_admin_id),
        (v_org_id, v_satheesh_id, 'alerts_events.active', v_admin_id),
        (v_org_id, v_satheesh_id, 'alerts_events.channels', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- jimmy: db(all), security(dashboard+firewall), logs(explorer+alerts)
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_jimmy_id, 'database.dashboard', v_admin_id),
        (v_org_id, v_jimmy_id, 'database.oracle', v_admin_id),
        (v_org_id, v_jimmy_id, 'security.dashboard', v_admin_id),
        (v_org_id, v_jimmy_id, 'security.firewall', v_admin_id),
        (v_org_id, v_jimmy_id, 'logs.explorer', v_admin_id),
        (v_org_id, v_jimmy_id, 'logs.alerts', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- anand: infra(storage), ai(insights+predictive+rca), otel(health+traces+span_metrics)
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

    RAISE NOTICE 'ALL DONE: admin=%, satheesh=%, jimmy=%, anand=%', v_admin_id, v_satheesh_id, v_jimmy_id, v_anand_id;
END $$;

-- ═══════════════════════════════════════════════════════════════════════════
-- 3. Clear rate limiting
-- ═══════════════════════════════════════════════════════════════════════════
DELETE FROM login_attempts WHERE success = false;

-- ═══════════════════════════════════════════════════════════════════════════
-- 4. Verify
-- ═══════════════════════════════════════════════════════════════════════════
SELECT username, email, is_active, is_locked FROM users
WHERE username IN ('admin', 'satheesh', 'jimmy', 'anand')
ORDER BY username;

SELECT u.username, array_agg(uma.module_id ORDER BY uma.module_id) AS access
FROM user_module_access uma JOIN users u ON u.id = uma.user_id
GROUP BY u.username ORDER BY u.username;
