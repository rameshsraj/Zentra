-- Clear ALL failed login attempts to reset IP-based rate limiting
DELETE FROM login_attempts WHERE success = false;

-- Show remaining
SELECT COUNT(*) AS remaining_failed FROM login_attempts WHERE success = false;
