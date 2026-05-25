-- SkyWalking P09: Continuous eBPF Profiling (profiles table — separate from OTel otel_profiles)
CREATE TABLE IF NOT EXISTS profiling_jobs (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id        UUID NOT NULL,
    target_id     UUID NOT NULL REFERENCES targets(target_id),
    profile_type  TEXT NOT NULL DEFAULT 'cpu',
    status        TEXT NOT NULL DEFAULT 'pending',
    duration_s    INTEGER NOT NULL DEFAULT 30,
    pid_filter    TEXT,
    triggered_by  UUID,
    triggered_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at  TIMESTAMPTZ,
    error_message TEXT
);

CREATE TABLE IF NOT EXISTS profiles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    job_id          UUID NOT NULL REFERENCES profiling_jobs(id),
    target_id       UUID NOT NULL REFERENCES targets(target_id),
    profile_type    TEXT NOT NULL,
    collected_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_s      INTEGER NOT NULL,
    sample_count    INTEGER,
    flamegraph_data JSONB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_profiles_target ON profiles(target_id, collected_at DESC);
