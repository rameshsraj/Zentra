-- ============================================================================
-- Migration 076: Organizations — Parent entity for multi-tenancy
--
-- Creates the `organizations` table as the authoritative source for org_id.
-- Seeds the default org (00000000-...-000000000001) that all existing data
-- already references.  Adds FK constraints from users, roles, user_roles,
-- api_keys, user_groups, and org_settings so every org_id is now validated.
-- ============================================================================

-- 1. Organizations table
CREATE TABLE IF NOT EXISTS organizations (
    org_id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            VARCHAR(255) NOT NULL,
    slug            VARCHAR(100) NOT NULL UNIQUE,       -- URL-safe identifier
    plan            VARCHAR(50)  NOT NULL DEFAULT 'starter',  -- free, starter, professional, enterprise
    status          VARCHAR(50)  NOT NULL DEFAULT 'active',   -- active, suspended, trial, cancelled
    owner_email     VARCHAR(255) NOT NULL,
    trial_ends_at   TIMESTAMPTZ,
    max_targets     INT NOT NULL DEFAULT 100,
    max_users       INT NOT NULL DEFAULT 25,
    max_dashboards  INT NOT NULL DEFAULT 50,
    max_alert_rules INT NOT NULL DEFAULT 200,
    retention_days  INT NOT NULL DEFAULT 90,
    features        JSONB NOT NULL DEFAULT '{}',         -- feature flags per org
    custom_branding BOOLEAN NOT NULL DEFAULT false,
    sso_enabled     BOOLEAN NOT NULL DEFAULT false,
    api_rate_limit  INT NOT NULL DEFAULT 2000,           -- requests per minute
    region          VARCHAR(50) NOT NULL DEFAULT 'default',
    notes           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_organizations_slug   ON organizations(slug);
CREATE INDEX IF NOT EXISTS idx_organizations_status ON organizations(status);

-- 2. Seed the default organization that all existing data references
INSERT INTO organizations (org_id, name, slug, plan, status, owner_email, max_targets, max_users, max_dashboards, max_alert_rules, retention_days)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'Default Organization',
    'default',
    'enterprise',
    'active',
    'admin@zentra.local',
    500, 50, 200, 1000, 365
) ON CONFLICT (org_id) DO NOTHING;

-- 3. Seed org_settings row if missing (matches the organizations row)
INSERT INTO org_settings (org_id, max_targets, max_dashboards, max_alert_rules, max_users, retention_days)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    500, 200, 1000, 50, 365
) ON CONFLICT (org_id) DO NOTHING;

-- 4. Add FK constraints (idempotent — skip if already present)

-- users.org_id → organizations.org_id
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_users_org' AND table_name = 'users'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT fk_users_org
            FOREIGN KEY (org_id) REFERENCES organizations(org_id);
    END IF;
END $$;

-- roles.org_id → organizations.org_id
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_roles_org' AND table_name = 'roles'
    ) THEN
        ALTER TABLE roles ADD CONSTRAINT fk_roles_org
            FOREIGN KEY (org_id) REFERENCES organizations(org_id);
    END IF;
END $$;

-- user_roles.org_id → organizations.org_id
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_user_roles_org' AND table_name = 'user_roles'
    ) THEN
        ALTER TABLE user_roles ADD CONSTRAINT fk_user_roles_org
            FOREIGN KEY (org_id) REFERENCES organizations(org_id);
    END IF;
END $$;

-- api_keys.org_id → organizations.org_id
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_api_keys_org' AND table_name = 'api_keys'
    ) THEN
        ALTER TABLE api_keys ADD CONSTRAINT fk_api_keys_org
            FOREIGN KEY (org_id) REFERENCES organizations(org_id);
    END IF;
END $$;

-- user_groups.org_id → organizations.org_id
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_user_groups_org' AND table_name = 'user_groups'
    ) THEN
        ALTER TABLE user_groups ADD CONSTRAINT fk_user_groups_org
            FOREIGN KEY (org_id) REFERENCES organizations(org_id);
    END IF;
END $$;

-- org_settings.org_id → organizations.org_id
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_org_settings_org' AND table_name = 'org_settings'
    ) THEN
        ALTER TABLE org_settings ADD CONSTRAINT fk_org_settings_org
            FOREIGN KEY (org_id) REFERENCES organizations(org_id);
    END IF;
END $$;
