-- Add must_change_password flag to users table (zentra database only)
ALTER TABLE users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT false;
-- Set flag for default admin accounts
UPDATE users SET must_change_password = true WHERE email IN ('admin@Zentra.local', 'admin@smarsh-nexus.com');
