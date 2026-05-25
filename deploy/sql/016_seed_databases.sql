-- Seed database targets so the Database Dashboard shows them
-- Oracle instance
INSERT INTO targets (target_id, org_id, name, target_type, target_subtype, host, port, status, is_active, credentials)
VALUES (
    uuid_generate_v4(),
    (SELECT org_id FROM users LIMIT 1),
    'Oracle ORCL (Prod)',
    'database',
    'oracle',
    '10.88.11.162',
    1521,
    'healthy',
    true,
    '{"username": "system", "database_name": "ORCL"}'::jsonb
) ON CONFLICT DO NOTHING;
