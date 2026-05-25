-- ============================================================================
-- Migration 010: RBAC, Multi-Tenancy Enforcement, Enhanced Audit Logging
-- ============================================================================

-- Roles
CREATE TABLE IF NOT EXISTS roles (
    role_id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            TEXT NOT NULL,
    description     TEXT,
    is_system       BOOLEAN DEFAULT false,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, name)
);

-- Permissions
CREATE TABLE IF NOT EXISTS permissions (
    permission_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource        TEXT NOT NULL,  -- e.g. 'dashboard', 'alert', 'target', 'user', 'org'
    action          TEXT NOT NULL,  -- e.g. 'read', 'write', 'delete', 'admin'
    description     TEXT,
    UNIQUE(resource, action)
);

-- Role-Permission mapping
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id         UUID NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    permission_id   UUID NOT NULL REFERENCES permissions(permission_id) ON DELETE CASCADE,
    PRIMARY KEY(role_id, permission_id)
);

-- User-Role mapping
CREATE TABLE IF NOT EXISTS user_roles (
    user_id         UUID NOT NULL,
    role_id         UUID NOT NULL REFERENCES roles(role_id) ON DELETE CASCADE,
    org_id          UUID NOT NULL,
    assigned_by     UUID,
    assigned_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(user_id, role_id, org_id)
);

-- Add FK to users if users table exists (it will be created in migration 012)
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'users') THEN
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.table_constraints
            WHERE constraint_name = 'fk_user_roles_user'
        ) THEN
            ALTER TABLE user_roles ADD CONSTRAINT fk_user_roles_user
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
        END IF;
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_user_roles_user ON user_roles(user_id);
CREATE INDEX IF NOT EXISTS idx_user_roles_org ON user_roles(org_id);

-- API keys for service accounts / agents
CREATE TABLE IF NOT EXISTS api_keys (
    key_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    user_id         UUID,
    name            TEXT NOT NULL,
    key_hash        TEXT NOT NULL,
    key_prefix      TEXT NOT NULL,  -- First 8 chars for display
    role_id         UUID REFERENCES roles(role_id) ON DELETE SET NULL,
    expires_at      TIMESTAMPTZ,
    last_used_at    TIMESTAMPTZ,
    enabled         BOOLEAN DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_api_keys_org ON api_keys(org_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);

-- Enhanced audit log (append-only)
CREATE TABLE IF NOT EXISTS audit_log (
    log_id          UUID DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    user_id         UUID,
    api_key_id      UUID,
    action          TEXT NOT NULL,       -- e.g. 'dashboard.create', 'alert.delete'
    resource_type   TEXT NOT NULL,       -- e.g. 'dashboard', 'alert_rule', 'target'
    resource_id     TEXT,
    details         JSONB DEFAULT '{}',
    ip_address      INET,
    user_agent      TEXT,
    status          TEXT DEFAULT 'success', -- 'success', 'denied', 'error'
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Make audit_log a hypertable for efficient time-based queries
SELECT create_hypertable('audit_log', 'timestamp', if_not_exists => TRUE);

-- Retention: keep audit logs for 1 year
DO $$ BEGIN
    PERFORM add_retention_policy('audit_log', INTERVAL '365 days', if_not_exists => TRUE);
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Retention policy skipped (TimescaleDB Community license required): %', SQLERRM;
END $$;

CREATE INDEX IF NOT EXISTS idx_audit_log_org ON audit_log(org_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_user ON audit_log(user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_resource ON audit_log(resource_type, resource_id);

-- Multi-tenancy: org-level settings
CREATE TABLE IF NOT EXISTS org_settings (
    org_id          UUID PRIMARY KEY,
    max_targets     INT DEFAULT 100,
    max_dashboards  INT DEFAULT 50,
    max_alert_rules INT DEFAULT 200,
    max_users       INT DEFAULT 25,
    retention_days  INT DEFAULT 90,
    features        JSONB DEFAULT '{}',  -- Feature flags per org
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed default permissions
INSERT INTO permissions (permission_id, resource, action, description) VALUES
    (gen_random_uuid(), 'dashboard', 'read', 'View dashboards'),
    (gen_random_uuid(), 'dashboard', 'write', 'Create/edit dashboards'),
    (gen_random_uuid(), 'dashboard', 'delete', 'Delete dashboards'),
    (gen_random_uuid(), 'alert', 'read', 'View alerts and rules'),
    (gen_random_uuid(), 'alert', 'write', 'Create/edit alert rules'),
    (gen_random_uuid(), 'alert', 'delete', 'Delete alert rules'),
    (gen_random_uuid(), 'alert', 'acknowledge', 'Acknowledge alerts'),
    (gen_random_uuid(), 'target', 'read', 'View targets'),
    (gen_random_uuid(), 'target', 'write', 'Create/edit targets'),
    (gen_random_uuid(), 'target', 'delete', 'Delete targets'),
    (gen_random_uuid(), 'user', 'read', 'View users'),
    (gen_random_uuid(), 'user', 'write', 'Create/edit users'),
    (gen_random_uuid(), 'user', 'admin', 'Manage roles and permissions'),
    (gen_random_uuid(), 'org', 'read', 'View organization settings'),
    (gen_random_uuid(), 'org', 'admin', 'Manage organization settings'),
    (gen_random_uuid(), 'notification', 'read', 'View notification policies'),
    (gen_random_uuid(), 'notification', 'write', 'Create/edit notification policies'),
    (gen_random_uuid(), 'metrics', 'read', 'Query metrics'),
    (gen_random_uuid(), 'metrics', 'write', 'Write metrics (agent)'),
    (gen_random_uuid(), 'logs', 'read', 'Query logs'),
    (gen_random_uuid(), 'logs', 'write', 'Write logs (agent)')
ON CONFLICT DO NOTHING;

-- Seed default roles for first org
INSERT INTO roles (role_id, org_id, name, description, is_system) VALUES
    ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000001', 'Admin', 'Full access to all resources', true),
    ('00000000-0000-0000-0000-000000000011', '00000000-0000-0000-0000-000000000001', 'Editor', 'Read/write access to dashboards and alerts', true),
    ('00000000-0000-0000-0000-000000000012', '00000000-0000-0000-0000-000000000001', 'Viewer', 'Read-only access', true),
    ('00000000-0000-0000-0000-000000000013', '00000000-0000-0000-0000-000000000001', 'Agent', 'Metric/log ingestion only', true)
ON CONFLICT DO NOTHING;

-- Grant all permissions to Admin role
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000010', permission_id FROM permissions
ON CONFLICT DO NOTHING;

-- Grant read permissions to Viewer role
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000012', permission_id FROM permissions WHERE action = 'read'
ON CONFLICT DO NOTHING;

-- Grant read+write to Editor role
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000011', permission_id FROM permissions WHERE action IN ('read', 'write', 'acknowledge')
ON CONFLICT DO NOTHING;

-- Grant metrics+logs write to Agent role
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000013', permission_id FROM permissions WHERE resource IN ('metrics', 'logs') AND action = 'write'
ON CONFLICT DO NOTHING;
