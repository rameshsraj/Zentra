-- middleware_instances: persisted registry for all middleware instances
-- Replaces the in-memory Arc<RwLock<Vec<Instance>>> stores in each handler
CREATE TABLE IF NOT EXISTS middleware_instances (
    id              TEXT PRIMARY KEY,
    instance_type   TEXT NOT NULL,        -- docker, kafka, elasticsearch, haproxy, tomcat, phpfpm, rabbitmq, redis, memcached, kubernetes, nginx, apache
    name            TEXT NOT NULL,
    host            TEXT NOT NULL,
    port            INT  NOT NULL,
    status          TEXT NOT NULL DEFAULT 'unknown',
    extra           JSONB NOT NULL DEFAULT '{}',  -- type-specific fields (e.g. stub_status_path, username)
    org_id          UUID REFERENCES organizations(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_mw_instances_type ON middleware_instances(instance_type);
CREATE INDEX IF NOT EXISTS idx_mw_instances_org  ON middleware_instances(org_id);
