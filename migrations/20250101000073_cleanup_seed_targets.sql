-- M09: Remove hardcoded Oracle target seed row from migration 000016.
-- On a fresh non-Docker deploy, no fabricated target rows should exist.
-- Bootstrap auth data (users/roles/permissions from migrations 012, 015) remains intact.
-- This migration is idempotent: if the row doesn't exist, nothing happens.

DELETE FROM targets
WHERE target_id = '00000000-0000-0000-0000-000000000202'
  AND host = '10.88.11.162'
  AND name = 'Oracle ORCL (Prod)';
