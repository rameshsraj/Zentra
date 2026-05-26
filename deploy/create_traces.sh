#!/bin/bash
set -e

# Get the traces table DDL from postgres DB and apply it to zentra DB
echo "=== Getting traces table DDL from postgres ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d postgres -t -c "
SELECT 'CREATE TABLE IF NOT EXISTS traces (' || string_agg(
  column_name || ' ' || 
  CASE 
    WHEN data_type = 'USER-DEFINED' THEN udt_name
    WHEN data_type = 'character varying' THEN 'varchar(' || character_maximum_length || ')'
    WHEN data_type = 'ARRAY' THEN udt_name
    ELSE data_type
  END ||
  CASE WHEN is_nullable = 'NO' THEN ' NOT NULL' ELSE '' END,
  ', ' ORDER BY ordinal_position
) || ');'
FROM information_schema.columns
WHERE table_name = 'traces' AND table_schema = 'public';
" 2>&1

echo ""
echo "=== Column details ==="
PGPASSWORD=zantaz psql -h 127.0.0.1 -p 5434 -U postgres -d postgres -c "
SELECT column_name, data_type, udt_name, is_nullable, column_default
FROM information_schema.columns
WHERE table_name = 'traces' AND table_schema = 'public'
ORDER BY ordinal_position;
" 2>&1
