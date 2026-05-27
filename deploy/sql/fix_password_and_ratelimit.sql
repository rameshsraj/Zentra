-- Generate bcrypt hash for 'Admin@12345' using pgcrypto
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Set password to 'Admin@12345' for all 4 users
UPDATE users SET password_hash = crypt('Admin@12345', gen_salt('bf', 12))
WHERE username IN ('admin', 'satheesh', 'jimmy', 'anand');

-- Clear ALL login attempts to reset rate limiting
DELETE FROM login_attempts;

-- Unlock all users
UPDATE users SET is_locked = false, locked_until = NULL
WHERE username IN ('admin', 'satheesh', 'jimmy', 'anand');

-- Verify
SELECT username, email, is_active, is_locked FROM users
WHERE username IN ('admin', 'satheesh', 'jimmy', 'anand');
