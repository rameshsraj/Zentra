-- 1. Unlock accounts
UPDATE users SET is_locked = false, locked_until = NULL
WHERE username IN ('satheesh', 'jimmy', 'anand');

-- 2. Clear failed login attempts from the login_attempts table (IP-based rate limiting)
DELETE FROM login_attempts
WHERE email IN ('satheesh@zentra.local', 'jimmy@zentra.local', 'anand@zentra.local')
  AND success = false;

-- 3. Copy admin's actual password hash to all 3 users so they share the same password
UPDATE users SET password_hash = (SELECT password_hash FROM users WHERE username = 'admin' LIMIT 1)
WHERE username IN ('satheesh', 'jimmy', 'anand');

-- 4. Verify
SELECT username, email, is_active, is_locked, LEFT(password_hash, 30) AS pw_prefix
FROM users WHERE username IN ('satheesh', 'jimmy', 'anand', 'admin');
