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
    session_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
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
UPDATE users SET password_hash = '$2b$12$W7iWSMpQJpRGBUU9PPOjnuyCnhCBBHbjQT/E.0hhNPXncQAg2V3/m'
WHERE email = 'ramesh@smarsh.com';

-- Also insert a dedicated admin@Zentra.local user
INSERT INTO users (id, org_id, email, username, password_hash, display_name, full_name, is_active)
VALUES (
    '00000000-0000-0000-0000-000000000100',
    '00000000-0000-0000-0000-000000000001',
    'admin@Zentra.local',
    'admin',
    '$2b$12$W7iWSMpQJpRGBUU9PPOjnuyCnhCBBHbjQT/E.0hhNPXncQAg2V3/m',
    'Administrator',
    'Administrator',
    true
) ON CONFLICT DO NOTHING;
