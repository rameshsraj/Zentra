-- 1. Clear ALL failed login attempts (reset rate limiting)
DELETE FROM login_attempts WHERE success = false;

-- 2. Unlock all users
UPDATE users SET is_locked = false, locked_until = NULL
WHERE username IN ('satheesh', 'jimmy', 'anand', 'admin');

-- 3. Fix admin email to zentra.local
UPDATE users SET email = 'admin@zentra.local' WHERE username = 'admin';

-- 4. Show password hashes so we can identify the password
SELECT username, email, password_hash FROM users
WHERE username IN ('admin', 'satheesh', 'jimmy', 'anand');
