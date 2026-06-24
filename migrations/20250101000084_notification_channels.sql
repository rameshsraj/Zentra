-- notification_channels: persistent storage for alert notification channels
CREATE TABLE IF NOT EXISTS notification_channels (
    channel_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id       UUID NOT NULL REFERENCES organizations(org_id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    channel_type TEXT NOT NULL,  -- 'slack' | 'email' | 'pagerduty' | 'webhook' | 'telegram' | 'opsgenie'
    config       JSONB NOT NULL DEFAULT '{}',
    enabled      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notification_channels_org ON notification_channels(org_id);
