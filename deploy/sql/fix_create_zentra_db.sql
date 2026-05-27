-- Create zentra database by cloning postgres (with all data)
-- This must be run while no other sessions are connected to postgres DB

-- Step 1: Terminate other connections to postgres DB
SELECT pg_terminate_backend(pid) FROM pg_stat_activity
WHERE datname = 'postgres' AND pid <> pg_backend_pid();

-- Step 2: Create zentra as a copy of postgres
-- NOTE: This SQL file cannot do CREATE DATABASE inside a transaction.
-- We'll use a shell script instead.
