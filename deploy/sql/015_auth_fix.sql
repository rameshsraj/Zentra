-- Fix auth system for existing users table schema
-- Add missing columns
ALTER TABLE users ADD COLUMN IF NOT EXISTS is_locked boolean DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS locked_until timestamptz;
ALTER TABLE users ADD COLUMN IF NOT EXISTS mfa_enabled boolean DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS mfa_secret varchar(255);
ALTER TABLE users ADD COLUMN IF NOT EXISTS display_name varchar(255);
ALTER TABLE users ADD COLUMN IF NOT EXISTS timezone varchar(50) DEFAULT 'UTC';
ALTER TABLE users ADD COLUMN IF NOT EXISTS theme varchar(50) DEFAULT 'dark';
ALTER TABLE users ADD COLUMN IF NOT EXISTS password_changed_at timestamptz;

-- Create user_sessions table if missing
CREATE TABLE IF NOT EXISTS user_sessions (
    session_id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id uuid NOT NULL,
    token_hash varchar(255) NOT NULL,
    refresh_token_hash varchar(255),
    ip_address inet,
    user_agent text,
    expires_at timestamptz NOT NULL,
    created_at timestamptz DEFAULT now(),
    is_revoked boolean DEFAULT false
);
ALTER TABLE user_sessions ADD COLUMN IF NOT EXISTS refresh_token_hash varchar(255);

-- Update existing admin user password to 'changeme' with valid bcrypt hash
-- bcrypt hash of 'changeme' with cost 12: $2b$12$3koL0Rcqt5g3YdxIsq/g1u3h0gWbUGaj9uh2KFchf1xPkkPGS74gS
UPDATE users SET password_hash = '$2b$12$3koL0Rcqt5g3YdxIsq/g1u3h0gWbUGaj9uh2KFchf1xPkkPGS74gS'
WHERE email = 'ramesh@smarsh.com';

-- Also insert a dedicated admin@Zentra.local user
INSERT INTO users (user_id, org_id, email, username, password_hash, full_name, display_name, role, is_active)
VALUES (
    uuid_generate_v4(),
    (SELECT org_id FROM users LIMIT 1),
    'admin@Zentra.local',
    'admin',
    '$2b$12$3koL0Rcqt5g3YdxIsq/g1u3h0gWbUGaj9uh2KFchf1xPkkPGS74gS',
    'Administrator',
    'Administrator',
    'admin',
    true
) ON CONFLICT (email) DO UPDATE SET password_hash = EXCLUDED.password_hash;
