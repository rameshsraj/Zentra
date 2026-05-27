-- Update emails to @zentra.local for consistency
UPDATE users SET email = 'satheesh@zentra.local' WHERE username = 'satheesh';
UPDATE users SET email = 'jimmy@zentra.local' WHERE username = 'jimmy';
UPDATE users SET email = 'anand@zentra.local' WHERE username = 'anand';

-- Verify
SELECT username, email, is_active, is_locked FROM users
WHERE username IN ('satheesh', 'jimmy', 'anand', 'admin');
