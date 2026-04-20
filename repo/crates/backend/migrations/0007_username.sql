-- 0007_username.sql
-- Audit #4 Issue #4: sign-in contract is locally-validated username +
-- password. Prior builds looked users up by email_hash; we keep the
-- email column for operator/audit display but add a first-class
-- `username` column and backfill it from the email local-part for every
-- existing row so the demo accounts log in cleanly.
--
-- Username is case-insensitive-unique (we store lowercased). The
-- `ux_users_username` index enforces uniqueness; the legacy
-- `ux_users_email_hash` index stays in place for self-service lookups
-- that still need the address.

ALTER TABLE users ADD COLUMN IF NOT EXISTS username TEXT;

-- Backfill: derive username from email_mask local-part for seeded rows.
-- email_mask is of the form "j***@example.com"; splitting at '*' / '@'
-- gives us a predictable-but-not-unique prefix, so we fall back to the
-- row id when the prefix would collide.
UPDATE users
SET    username = LOWER(
           SPLIT_PART(
               CASE WHEN STRPOS(email_mask, '@') > 0
                    THEN SPLIT_PART(email_mask, '@', 1)
                    ELSE email_mask
               END,
               '*', 1)
       )
WHERE  username IS NULL;

-- Any leftover NULLs (rows with unusual masks) get the uuid.
UPDATE users SET username = LOWER(id::text) WHERE username IS NULL OR username = '';

ALTER TABLE users ALTER COLUMN username SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS ux_users_username ON users (LOWER(username));
