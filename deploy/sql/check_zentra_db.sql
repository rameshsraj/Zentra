SELECT count(*) AS table_count FROM information_schema.tables WHERE table_schema = 'public';
SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename LIMIT 10;
