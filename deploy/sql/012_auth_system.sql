-- ============================================================================
-- Migration 012: Authentication System — Users, Sessions, Login, Groups
-- Extends 010_rbac_audit.sql (roles, permissions, role_permissions, user_roles,
-- api_keys, audit_log already exist)
-- ============================================================================

-- Users table (core authentication)
CREATE TABLE IF NOT EXISTS users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    email           VARCHAR(255) NOT NULL,
    username        VARCHAR(100) NOT NULL,
    password_hash   VARCHAR(255) NOT NULL,
    display_name    VARCHAR(255),
    avatar_url      TEXT,
    timezone        VARCHAR(50) DEFAULT 'UTC',
    theme           VARCHAR(20) DEFAULT 'dark',
    is_active       BOOLEAN DEFAULT true,
    is_locked       BOOLEAN DEFAULT false,
    locked_until    TIMESTAMPTZ,
    mfa_enabled     BOOLEAN DEFAULT false,
    mfa_secret      VARCHAR(255),
    last_login      TIMESTAMPTZ,
    password_changed_at TIMESTAMPTZ DEFAULT NOW(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, email),
    UNIQUE(org_id, username)
);

CREATE INDEX IF NOT EXISTS idx_users_org ON users(org_id);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

-- User sessions (JWT tracking + refresh tokens)
CREATE TABLE IF NOT EXISTS user_sessions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash          VARCHAR(255) NOT NULL,
    refresh_token_hash  VARCHAR(255),
    ip_address          INET,
    user_agent          TEXT,
    expires_at          TIMESTAMPTZ NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_sessions_user ON user_sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_user_sessions_token ON user_sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_user_sessions_expires ON user_sessions(expires_at);

-- Login attempts (brute force protection)
CREATE TABLE IF NOT EXISTS login_attempts (
    id              BIGSERIAL PRIMARY KEY,
    email           VARCHAR(255),
    ip_address      INET,
    success         BOOLEAN NOT NULL,
    user_agent      TEXT,
    attempted_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_login_attempts_email ON login_attempts(email, attempted_at DESC);
CREATE INDEX IF NOT EXISTS idx_login_attempts_ip ON login_attempts(ip_address, attempted_at DESC);

-- User groups
CREATE TABLE IF NOT EXISTS user_groups (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    name            VARCHAR(100) NOT NULL,
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, name)
);

CREATE TABLE IF NOT EXISTS user_group_members (
    group_id        UUID NOT NULL REFERENCES user_groups(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    added_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(group_id, user_id)
);

-- Add user_id FK to existing user_roles if not already referencing users
-- (010 had user_id UUID NOT NULL without FK — we add soft reference)
-- No ALTER needed since user_roles.user_id is just a UUID column

-- Add additional permissions for new modules
INSERT INTO permissions (permission_id, resource, action, description) VALUES
    (gen_random_uuid(), 'servers', 'read', 'View servers'),
    (gen_random_uuid(), 'servers', 'write', 'Manage servers'),
    (gen_random_uuid(), 'network', 'read', 'View network'),
    (gen_random_uuid(), 'network', 'write', 'Manage network'),
    (gen_random_uuid(), 'storage', 'read', 'View storage'),
    (gen_random_uuid(), 'storage', 'write', 'Manage storage'),
    (gen_random_uuid(), 'databases', 'read', 'View databases'),
    (gen_random_uuid(), 'databases', 'write', 'Manage databases'),
    (gen_random_uuid(), 'middleware', 'read', 'View middleware'),
    (gen_random_uuid(), 'middleware', 'write', 'Manage middleware'),
    (gen_random_uuid(), 'security', 'read', 'View security'),
    (gen_random_uuid(), 'security', 'write', 'Manage security'),
    (gen_random_uuid(), 'healthchecks', 'read', 'View health checks'),
    (gen_random_uuid(), 'healthchecks', 'write', 'Manage health checks'),
    (gen_random_uuid(), 'deployments', 'read', 'View deployments'),
    (gen_random_uuid(), 'deployments', 'write', 'Manage deployments'),
    (gen_random_uuid(), 'discovery', 'read', 'View discovery'),
    (gen_random_uuid(), 'discovery', 'write', 'Manage discovery'),
    (gen_random_uuid(), 'templates', 'read', 'View templates'),
    (gen_random_uuid(), 'templates', 'write', 'Manage templates'),
    (gen_random_uuid(), 'maintenance', 'read', 'View maintenance'),
    (gen_random_uuid(), 'maintenance', 'write', 'Manage maintenance'),
    (gen_random_uuid(), 'licenses', 'read', 'View licenses'),
    (gen_random_uuid(), 'licenses', 'write', 'Manage licenses'),
    (gen_random_uuid(), 'ai', 'read', 'View AI insights'),
    (gen_random_uuid(), 'ai', 'write', 'Manage AI settings'),
    (gen_random_uuid(), 'oracle', 'read', 'View Oracle monitoring'),
    (gen_random_uuid(), 'oracle', 'write', 'Manage Oracle monitoring'),
    (gen_random_uuid(), 'topology', 'read', 'View topology'),
    (gen_random_uuid(), 'topology', 'write', 'Manage topology'),
    (gen_random_uuid(), 'user', 'delete', 'Delete users'),
    (gen_random_uuid(), 'admin', 'read', 'View admin panel'),
    (gen_random_uuid(), 'admin', 'write', 'Manage admin settings'),
    (gen_random_uuid(), 'admin', 'admin', 'Full admin access')
ON CONFLICT DO NOTHING;

-- Add Super Admin role (distinct from existing Admin which is org-scoped)
INSERT INTO roles (role_id, org_id, name, description, is_system) VALUES
    ('00000000-0000-0000-0000-000000000014', '00000000-0000-0000-0000-000000000001',
     'Super Admin', 'Full access including user management and system configuration', true),
    ('00000000-0000-0000-0000-000000000015', '00000000-0000-0000-0000-000000000001',
     'Operator', 'All monitoring read/write, acknowledge alerts, create targets', true)
ON CONFLICT DO NOTHING;

-- Grant all permissions to Super Admin
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000014', permission_id FROM permissions
ON CONFLICT DO NOTHING;

-- Grant all permissions to existing Admin too
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000010', permission_id FROM permissions
ON CONFLICT DO NOTHING;

-- Grant read+write to Operator (no admin, no user mgmt)
INSERT INTO role_permissions (role_id, permission_id)
SELECT '00000000-0000-0000-0000-000000000015', permission_id FROM permissions
WHERE action IN ('read', 'write', 'acknowledge')
  AND resource NOT IN ('user', 'admin')
ON CONFLICT DO NOTHING;

-- Seed default admin user
-- Password: changeme (bcrypt hash with cost 12)
INSERT INTO users (id, org_id, email, username, password_hash, display_name)
VALUES (
    '00000000-0000-0000-0000-000000000100',
    '00000000-0000-0000-0000-000000000001',
    'admin@Zentra.local',
    'admin',
    '$2b$12$LK5HF.0Z3Y5Y5Y5Y5Y5Y5OaCvmP2QR5R5R5R5R5R5R5R5R5R5R5',
    'Administrator'
) ON CONFLICT DO NOTHING;

-- Assign Super Admin role to default admin user
INSERT INTO user_roles (user_id, role_id, org_id)
VALUES (
    '00000000-0000-0000-0000-000000000100',
    '00000000-0000-0000-0000-000000000014',
    '00000000-0000-0000-0000-000000000001'
) ON CONFLICT DO NOTHING;

-- System settings table
CREATE TABLE IF NOT EXISTS system_settings (
    key             VARCHAR(100) PRIMARY KEY,
    value           JSONB NOT NULL,
    description     TEXT,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by      UUID REFERENCES users(id)
);

-- Seed default settings
INSERT INTO system_settings (key, value, description) VALUES
    ('site_name', '"Zentra"', 'Site display name'),
    ('default_timezone', '"UTC"', 'Default timezone for new users'),
    ('session_timeout_minutes', '60', 'Session timeout in minutes'),
    ('max_login_attempts', '5', 'Max failed login attempts before lockout'),
    ('lockout_duration_minutes', '30', 'Account lockout duration after max failed attempts'),
    ('password_min_length', '8', 'Minimum password length'),
    ('mfa_required', 'false', 'Require MFA for all users'),
    ('smtp_host', '""', 'SMTP server host'),
    ('smtp_port', '587', 'SMTP server port'),
    ('smtp_username', '""', 'SMTP username'),
    ('smtp_from', '"noreply@Zentra.local"', 'SMTP from address'),
    ('smtp_tls', 'true', 'Use TLS for SMTP'),
    ('retention_metrics_days', '90', 'Metrics data retention in days'),
    ('retention_logs_days', '30', 'Log data retention in days'),
    ('retention_audit_days', '365', 'Audit log retention in days'),
    ('retention_events_days', '90', 'Event data retention in days')
ON CONFLICT (key) DO NOTHING;
