-- 0002_rbac.sql
-- Roles, permissions, user_roles, role_permissions, append-only audit_log.
--
-- Business rules:
--   * Five canonical roles are verbatim (design §Actors And Roles).
--   * Authoritative permission codes seeded here (design §Permissions).
--   * Users may hold multiple roles (union of permissions).
--   * audit_log is append-only (trigger blocks UPDATE/DELETE).

CREATE TABLE IF NOT EXISTS roles (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL UNIQUE,
    display     TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS permissions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code        TEXT NOT NULL UNIQUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id       UUID NOT NULL REFERENCES roles(id)       ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE IF NOT EXISTS user_roles (
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id     UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    granted_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    granted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, role_id)
);

-- ---- Seed canonical roles (idempotent) ----
INSERT INTO roles (name, display) VALUES
    ('administrator', 'Administrator'),
    ('data_steward',  'Data Steward'),
    ('analyst',       'Analyst'),
    ('recruiter',     'Recruiter'),
    ('regular_user',  'Regular User')
ON CONFLICT (name) DO NOTHING;

-- ---- Seed authoritative permissions (idempotent) ----
INSERT INTO permissions (code) VALUES
    ('user.manage'), ('role.assign'),
    ('retention.manage'), ('monitoring.read'),
    ('allowlist.manage'), ('mtls.manage'),
    ('product.read'), ('product.write'), ('product.import'), ('product.history.read'),
    ('ref.write'),
    ('metric.read'), ('metric.configure'),
    ('alert.manage'), ('alert.ack'),
    ('report.schedule'), ('report.run'),
    ('kpi.read'),
    ('talent.read'), ('talent.manage'), ('talent.feedback')
ON CONFLICT (code) DO NOTHING;

-- ---- Seed role → permission grants (idempotent) ----
-- Administrator: user.manage, role.assign, retention.manage, monitoring.read,
--                allowlist.manage, mtls.manage, product.read, product.history.read,
--                metric.read, alert.manage, alert.ack, report.schedule, report.run,
--                kpi.read, talent.manage
-- Data Steward:  product.read, product.write, product.import, product.history.read,
--                ref.write, alert.ack, report.schedule, report.run, kpi.read
-- Analyst:       product.read, product.history.read, metric.read, metric.configure,
--                alert.manage, alert.ack, report.schedule, report.run, kpi.read
-- Recruiter:     product.read, alert.ack, report.run, kpi.read, talent.read,
--                talent.manage, talent.feedback
-- Regular User:  product.read, metric.read, alert.ack, report.run, kpi.read
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id FROM roles r CROSS JOIN permissions p
WHERE (r.name, p.code) IN (
    ('administrator', 'user.manage'), ('administrator', 'role.assign'),
    ('administrator', 'retention.manage'), ('administrator', 'monitoring.read'),
    ('administrator', 'allowlist.manage'), ('administrator', 'mtls.manage'),
    ('administrator', 'product.read'), ('administrator', 'product.history.read'),
    ('administrator', 'metric.read'),
    ('administrator', 'alert.manage'), ('administrator', 'alert.ack'),
    ('administrator', 'report.schedule'), ('administrator', 'report.run'),
    ('administrator', 'kpi.read'),
    ('administrator', 'talent.manage'),

    ('data_steward', 'product.read'), ('data_steward', 'product.write'),
    ('data_steward', 'product.import'), ('data_steward', 'product.history.read'),
    ('data_steward', 'ref.write'),
    ('data_steward', 'alert.ack'),
    ('data_steward', 'report.schedule'), ('data_steward', 'report.run'),
    ('data_steward', 'kpi.read'),

    ('analyst', 'product.read'), ('analyst', 'product.history.read'),
    ('analyst', 'metric.read'), ('analyst', 'metric.configure'),
    ('analyst', 'alert.manage'), ('analyst', 'alert.ack'),
    ('analyst', 'report.schedule'), ('analyst', 'report.run'),
    ('analyst', 'kpi.read'),

    ('recruiter', 'product.read'),
    ('recruiter', 'alert.ack'),
    ('recruiter', 'report.run'),
    ('recruiter', 'kpi.read'),
    ('recruiter', 'talent.read'), ('recruiter', 'talent.manage'),
    ('recruiter', 'talent.feedback'),

    ('regular_user', 'product.read'),
    ('regular_user', 'metric.read'),
    ('regular_user', 'alert.ack'),
    ('regular_user', 'report.run'),
    ('regular_user', 'kpi.read')
)
ON CONFLICT DO NOTHING;

-- ---- Audit log (append-only) ----
CREATE TABLE IF NOT EXISTS audit_log (
    id            BIGSERIAL PRIMARY KEY,
    actor_id      UUID REFERENCES users(id) ON DELETE SET NULL,
    action        TEXT        NOT NULL,
    target_type   TEXT,
    target_id     TEXT,
    meta_json     JSONB       NOT NULL DEFAULT '{}'::JSONB,
    at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS ix_audit_log_at     ON audit_log (at DESC);
CREATE INDEX IF NOT EXISTS ix_audit_log_actor  ON audit_log (actor_id);
CREATE INDEX IF NOT EXISTS ix_audit_log_action ON audit_log (action);

CREATE OR REPLACE FUNCTION audit_log_immutable() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'audit_log is append-only (action=%%)', TG_OP;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_audit_log_immutable ON audit_log;
CREATE TRIGGER trg_audit_log_immutable
BEFORE UPDATE OR DELETE ON audit_log
FOR EACH ROW EXECUTE FUNCTION audit_log_immutable();
