-- 0001_identity.sql
-- Baseline identity schema: users + sessions. RBAC, auth helpers, and feature
-- tables land in later migrations (0002+).
--
-- Email storage: plaintext email is never persisted. We keep:
--   * email_ciphertext : AES-256-GCM ciphertext (key lives on runtime volume)
--   * email_hash       : HMAC-SHA256 over normalized email (for uniqueness + lookup)
--   * email_mask       : precomputed display mask (e.g. "j***@example.com")
-- See design.md §Data-at-Rest for full details.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS users (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name        TEXT        NOT NULL,
    email_ciphertext    BYTEA       NOT NULL,
    email_hash          BYTEA       NOT NULL,
    email_mask          TEXT        NOT NULL,
    password_hash       TEXT        NOT NULL,
    password_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active           BOOLEAN     NOT NULL DEFAULT TRUE,
    failed_login_count  INT         NOT NULL DEFAULT 0,
    locked_until        TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_users_email_hash ON users (email_hash);
CREATE INDEX IF NOT EXISTS ix_users_is_active ON users (is_active);

CREATE TABLE IF NOT EXISTS sessions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    refresh_token_hash  BYTEA       NOT NULL,
    issued_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at          TIMESTAMPTZ NOT NULL,
    revoked_at          TIMESTAMPTZ,
    user_agent          TEXT,
    ip_addr             INET
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_sessions_refresh_hash ON sessions (refresh_token_hash);
CREATE INDEX IF NOT EXISTS ix_sessions_user ON sessions (user_id);
CREATE INDEX IF NOT EXISTS ix_sessions_active ON sessions (user_id) WHERE revoked_at IS NULL;
