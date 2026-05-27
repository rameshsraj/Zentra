-- ═══════════════════════════════════════════════════════════════════════════
-- 022: User Module Access Control
-- Per-user module-level RBAC for sidebar visibility and Command Center filtering
-- ═══════════════════════════════════════════════════════════════════════════

-- Module access table: which modules each user can see/access
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
-- Seed users: satheesh, jimmy, anand (password: Admin@12345)
-- bcrypt hash for 'Admin@12345' at cost 12
-- ═══════════════════════════════════════════════════════════════════════════

-- Default org_id
DO $$
DECLARE
    v_org_id UUID := '00000000-0000-0000-0000-000000000001';
    v_admin_id UUID;
    v_satheesh_id UUID;
    v_jimmy_id UUID;
    v_anand_id UUID;
    v_viewer_role_id UUID;
    v_admin_role_id UUID;
    v_pw_hash TEXT;
BEGIN
    -- Get admin user id (users table PK is user_id, auth code aliases it as id)
    SELECT user_id INTO v_admin_id FROM users WHERE email = 'admin@zentra.local' LIMIT 1;
    IF v_admin_id IS NULL THEN
        SELECT user_id INTO v_admin_id FROM users WHERE username = 'admin' LIMIT 1;
    END IF;

    -- Reuse the admin's password hash so new users start with the same password
    SELECT password_hash INTO v_pw_hash FROM users WHERE user_id = v_admin_id;
    IF v_pw_hash IS NULL THEN
        -- fallback: bcrypt hash for 'Admin@12345'
        v_pw_hash := '$2b$12$3koL0Rcqt5g3YdxIsq/g1u3h0gWbUGaj9uh2KFchf1xPkkPGS74gS';
    END IF;

    -- Get role IDs
    SELECT role_id INTO v_viewer_role_id FROM roles WHERE name = 'Viewer' AND org_id = v_org_id LIMIT 1;
    SELECT role_id INTO v_admin_role_id FROM roles WHERE name = 'Admin' AND org_id = v_org_id LIMIT 1;

    -- If no Viewer role, create one
    IF v_viewer_role_id IS NULL THEN
        v_viewer_role_id := gen_random_uuid();
        INSERT INTO roles (role_id, org_id, name, description, is_system)
        VALUES (v_viewer_role_id, v_org_id, 'Viewer', 'Read-only access', true)
        ON CONFLICT DO NOTHING;
    END IF;

    -- ═══════════════════════════════════════════════════════════════════
    -- Create user: satheesh
    -- ═══════════════════════════════════════════════════════════════════
    v_satheesh_id := gen_random_uuid();
    INSERT INTO users (user_id, org_id, email, username, password_hash, display_name, is_active)
    VALUES (v_satheesh_id, v_org_id, 'satheesh@zentra.local', 'satheesh', v_pw_hash, 'Satheesh', true)
    ON CONFLICT (email) DO UPDATE SET display_name = 'Satheesh', is_active = true
    RETURNING user_id INTO v_satheesh_id;

    -- Assign Viewer role
    INSERT INTO user_roles (user_id, role_id, org_id) VALUES (v_satheesh_id, v_viewer_role_id, v_org_id)
    ON CONFLICT DO NOTHING;

    -- Module access: command_center, infrastructure, database, middleware, alerts_events
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_satheesh_id, 'command_center', v_admin_id),
        (v_org_id, v_satheesh_id, 'infrastructure', v_admin_id),
        (v_org_id, v_satheesh_id, 'database', v_admin_id),
        (v_org_id, v_satheesh_id, 'middleware', v_admin_id),
        (v_org_id, v_satheesh_id, 'alerts_events', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- ═══════════════════════════════════════════════════════════════════
    -- Create user: jimmy
    -- ═══════════════════════════════════════════════════════════════════
    v_jimmy_id := gen_random_uuid();
    INSERT INTO users (user_id, org_id, email, username, password_hash, display_name, is_active)
    VALUES (v_jimmy_id, v_org_id, 'jimmy@zentra.local', 'jimmy', v_pw_hash, 'Jimmy', true)
    ON CONFLICT (email) DO UPDATE SET display_name = 'Jimmy', is_active = true
    RETURNING user_id INTO v_jimmy_id;

    INSERT INTO user_roles (user_id, role_id, org_id) VALUES (v_jimmy_id, v_viewer_role_id, v_org_id)
    ON CONFLICT DO NOTHING;

    -- Module access: database, middleware, security, logs, alerts_events
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_jimmy_id, 'database', v_admin_id),
        (v_org_id, v_jimmy_id, 'middleware', v_admin_id),
        (v_org_id, v_jimmy_id, 'security', v_admin_id),
        (v_org_id, v_jimmy_id, 'logs', v_admin_id),
        (v_org_id, v_jimmy_id, 'alerts_events', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- ═══════════════════════════════════════════════════════════════════
    -- Create user: anand
    -- ═══════════════════════════════════════════════════════════════════
    v_anand_id := gen_random_uuid();
    INSERT INTO users (user_id, org_id, email, username, password_hash, display_name, is_active)
    VALUES (v_anand_id, v_org_id, 'anand@zentra.local', 'anand', v_pw_hash, 'Anand', true)
    ON CONFLICT (email) DO UPDATE SET display_name = 'Anand', is_active = true
    RETURNING user_id INTO v_anand_id;

    INSERT INTO user_roles (user_id, role_id, org_id) VALUES (v_anand_id, v_viewer_role_id, v_org_id)
    ON CONFLICT DO NOTHING;

    -- Module access: command_center, infrastructure, logs, alerts_events, ai_analytics, opentelemetry
    INSERT INTO user_module_access (org_id, user_id, module_id, granted_by) VALUES
        (v_org_id, v_anand_id, 'command_center', v_admin_id),
        (v_org_id, v_anand_id, 'infrastructure', v_admin_id),
        (v_org_id, v_anand_id, 'logs', v_admin_id),
        (v_org_id, v_anand_id, 'alerts_events', v_admin_id),
        (v_org_id, v_anand_id, 'ai_analytics', v_admin_id),
        (v_org_id, v_anand_id, 'opentelemetry', v_admin_id)
    ON CONFLICT (org_id, user_id, module_id) DO NOTHING;

    -- ═══════════════════════════════════════════════════════════════════
    -- Admin gets ALL modules
    -- ═══════════════════════════════════════════════════════════════════
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

    RAISE NOTICE 'Module access seeded: admin=%, satheesh=%, jimmy=%, anand=%',
        v_admin_id, v_satheesh_id, v_jimmy_id, v_anand_id;
END $$;
