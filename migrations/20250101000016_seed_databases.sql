-- Seed database targets so the Database Dashboard shows them
INSERT INTO targets (target_id, org_id, name, target_type, target_subtype, host, port, status, is_active, credentials)
VALUES (
    '00000000-0000-0000-0000-000000000201',
    '00000000-0000-0000-0000-000000000001',
    'PostgreSQL Primary',
    'database',
    'postgresql',
    '10.88.11.162',
    5434,
    'healthy',
    true,
    '{"username": "postgres", "database_name": "postgres"}'::jsonb
) ON CONFLICT DO NOTHING;

-- Also add the Oracle instance if not present
INSERT INTO targets (target_id, org_id, name, target_type, target_subtype, host, port, status, is_active, credentials)
VALUES (
    '00000000-0000-0000-0000-000000000202',
    '00000000-0000-0000-0000-000000000001',
    'Oracle ORCL (Prod)',
    'database',
    'oracle',
    '10.88.11.162',
    1521,
    'healthy',
    true,
    '{"username": "system", "database_name": "ORCL"}'::jsonb
) ON CONFLICT DO NOTHING;
