-- 0004_notifications.sql
-- Notification center + retry queue + mailbox exports (design §Notifications).

CREATE TABLE IF NOT EXISTS notification_subscriptions (
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    topic       TEXT        NOT NULL,
    enabled     BOOLEAN     NOT NULL DEFAULT TRUE,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, topic)
);

CREATE TABLE IF NOT EXISTS notifications (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    topic        TEXT        NOT NULL,
    title        TEXT        NOT NULL,
    body         TEXT        NOT NULL,
    payload_json JSONB       NOT NULL DEFAULT '{}'::JSONB,
    read_at      TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_notifications_user_created
    ON notifications (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS ix_notifications_user_unread
    ON notifications (user_id) WHERE read_at IS NULL;

CREATE TABLE IF NOT EXISTS notification_delivery_attempts (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id  UUID        NOT NULL REFERENCES notifications(id) ON DELETE CASCADE,
    attempt_no       INT         NOT NULL,
    state            TEXT        NOT NULL
        CHECK (state IN ('pending','success','failed')),
    error            TEXT,
    attempted_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    next_retry_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_notif_delivery_pending
    ON notification_delivery_attempts (state, next_retry_at) WHERE state = 'pending';

CREATE TABLE IF NOT EXISTS mailbox_exports (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    path         TEXT        NOT NULL,
    size_bytes   BIGINT      NOT NULL DEFAULT 0,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_mailbox_exports_user_at
    ON mailbox_exports (user_id, generated_at DESC);
