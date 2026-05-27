-- Check users table constraints
SELECT conname, contype, pg_get_constraintdef(oid)
FROM pg_constraint WHERE conrelid = 'users'::regclass;

-- Check if satheesh/jimmy/anand already exist
SELECT id, username, email, is_active, is_locked FROM users
WHERE username IN ('satheesh', 'jimmy', 'anand', 'admin');
