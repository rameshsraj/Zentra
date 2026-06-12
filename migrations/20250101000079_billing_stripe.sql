-- SAAS-05: Billing and Stripe subscription support

-- Add subscription columns to organizations
ALTER TABLE organizations
    ADD COLUMN IF NOT EXISTS stripe_customer_id TEXT,
    ADD COLUMN IF NOT EXISTS stripe_subscription_id TEXT,
    ADD COLUMN IF NOT EXISTS subscription_status TEXT NOT NULL DEFAULT 'trialling',
    ADD COLUMN IF NOT EXISTS subscription_period_end TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS subscription_past_due_since TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_organizations_stripe_customer
    ON organizations (stripe_customer_id) WHERE stripe_customer_id IS NOT NULL;

-- Webhook event log for idempotency
CREATE TABLE IF NOT EXISTS stripe_webhook_events (
    event_id   TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    processed  BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
