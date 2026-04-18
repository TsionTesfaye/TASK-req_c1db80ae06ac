-- 0005_user_ext.sql
-- Adds `timezone` to users (needed by AuthUserDto + user profile surfaces).
-- Nullable: falls back to AppState.default_timezone when unset.

ALTER TABLE users ADD COLUMN IF NOT EXISTS timezone TEXT;
