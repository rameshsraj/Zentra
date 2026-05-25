-- Migration 025: Dashboard Feature Tables
-- Adds: dashboard_snapshots, dashboard_playlists, dashboard_bookmarks, query_history
-- These tables support dashboard snapshots for sharing, playlist auto-rotation,
-- user bookmarks, and query history with starring/commenting.

-- 1. Dashboard Snapshots
CREATE TABLE IF NOT EXISTS dashboard_snapshots (
    snapshot_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        UUID NOT NULL,
    dashboard_id  UUID NOT NULL REFERENCES dashboards(dashboard_id) ON DELETE CASCADE,
    name          VARCHAR(255) NOT NULL,
    key           TEXT NOT NULL UNIQUE,
    delete_key    TEXT NOT NULL UNIQUE,
    snapshot_data JSONB NOT NULL,
    external      BOOLEAN NOT NULL DEFAULT FALSE,
    expires_at    TIMESTAMPTZ,
    created_by    UUID,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_snapshots_org_dashboard
    ON dashboard_snapshots (org_id, dashboard_id);
CREATE INDEX IF NOT EXISTS idx_snapshots_key
    ON dashboard_snapshots (key);
CREATE INDEX IF NOT EXISTS idx_snapshots_expires
    ON dashboard_snapshots (expires_at)
    WHERE expires_at IS NOT NULL;

-- 2. Dashboard Playlists
CREATE TABLE IF NOT EXISTS dashboard_playlists (
    playlist_id       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id            UUID NOT NULL,
    name              VARCHAR(255) NOT NULL,
    description       TEXT,
    dashboard_ids     UUID[] NOT NULL DEFAULT '{}',
    interval_seconds  INT NOT NULL DEFAULT 30,
    enabled           BOOLEAN NOT NULL DEFAULT TRUE,
    created_by        UUID,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'dashboard_playlists_org_name_key'
    ) THEN
        ALTER TABLE dashboard_playlists ADD CONSTRAINT dashboard_playlists_org_name_key UNIQUE (org_id, name);
    END IF;
END $$;

-- 3. Dashboard Bookmarks
CREATE TABLE IF NOT EXISTS dashboard_bookmarks (
    bookmark_id   UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        UUID NOT NULL,
    user_id       UUID NOT NULL,
    dashboard_id  UUID NOT NULL REFERENCES dashboards(dashboard_id) ON DELETE CASCADE,
    sort_order    INT NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE constraint_name = 'dashboard_bookmarks_user_dashboard_key'
    ) THEN
        ALTER TABLE dashboard_bookmarks ADD CONSTRAINT dashboard_bookmarks_user_dashboard_key UNIQUE (user_id, dashboard_id);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_bookmarks_user_order
    ON dashboard_bookmarks (user_id, sort_order);

-- 4. Query History
CREATE TABLE IF NOT EXISTS query_history (
    history_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        UUID NOT NULL,
    user_id       UUID NOT NULL,
    query_text    TEXT NOT NULL,
    datasource    VARCHAR(100) NOT NULL DEFAULT 'prometheus',
    duration_ms   INT,
    starred       BOOLEAN NOT NULL DEFAULT FALSE,
    comment       TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_query_history_user_time
    ON query_history (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_query_history_starred
    ON query_history (user_id, starred)
    WHERE starred = TRUE;
