-- SAAS-03: Self-service signup support columns and signup rate-limit

-- Add email verification columns to organizations
ALTER TABLE organizations
    ADD COLUMN IF NOT EXISTS email_verified BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS email_verify_token TEXT,
    ADD COLUMN IF NOT EXISTS email_verify_expires TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_organizations_email_verify_token
    ON organizations (email_verify_token) WHERE email_verify_token IS NOT NULL;

-- Signup rate-limit tracking (5 per IP per hour)
CREATE TABLE IF NOT EXISTS signup_rate_limit (
    ip_address  INET        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_signup_rate_limit_ip_time
    ON signup_rate_limit (ip_address, created_at);
