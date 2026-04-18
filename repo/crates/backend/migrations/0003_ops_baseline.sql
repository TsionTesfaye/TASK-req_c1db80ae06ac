-- 0003_ops_baseline.sql
-- Admin security + ops baseline: endpoint_allowlist, device_certs, mtls_config,
-- retention_policies, api_metrics, client_crash_reports. Matches design
-- §Monitoring/Ops/Security and api-spec §Security & Ops.

CREATE TABLE IF NOT EXISTS endpoint_allowlist (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cidr        CIDR        NOT NULL,
    note        TEXT,
    enabled     BOOLEAN     NOT NULL DEFAULT TRUE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_endpoint_allowlist_enabled ON endpoint_allowlist (enabled);

CREATE TABLE IF NOT EXISTS device_certs (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    label              TEXT        NOT NULL,
    issued_to_user_id  UUID REFERENCES users(id) ON DELETE SET NULL,
    serial             TEXT,
    spki_sha256        BYTEA       NOT NULL,
    pem_path           TEXT,
    notes              TEXT,
    issued_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at         TIMESTAMPTZ,
    created_by         UUID REFERENCES users(id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_device_certs_spki ON device_certs (spki_sha256);
CREATE INDEX IF NOT EXISTS ix_device_certs_user ON device_certs (issued_to_user_id);

CREATE TABLE IF NOT EXISTS mtls_config (
    id          INT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    enforced    BOOLEAN     NOT NULL DEFAULT FALSE,
    updated_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
INSERT INTO mtls_config (id, enforced) VALUES (1, FALSE) ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS retention_policies (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    domain           TEXT        NOT NULL UNIQUE
        CHECK (domain IN ('env_raw','kpi','feedback','audit')),
    ttl_days         INT         NOT NULL CHECK (ttl_days >= 0),
    last_enforced_at TIMESTAMPTZ,
    updated_by       UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- Defaults per design §Authoritative Business Rules #14.
INSERT INTO retention_policies (domain, ttl_days) VALUES
    ('env_raw',  548),   -- ~18 months
    ('kpi',      1825),  -- 5 years
    ('feedback', 730),   -- 24 months
    ('audit',    0)      -- 0 = indefinite
ON CONFLICT (domain) DO NOTHING;

CREATE TABLE IF NOT EXISTS api_metrics (
    id           BIGSERIAL PRIMARY KEY,
    route        TEXT        NOT NULL,
    method       TEXT        NOT NULL,
    status       INT         NOT NULL,
    latency_ms   INT         NOT NULL,
    request_id   TEXT,
    at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_api_metrics_at    ON api_metrics (at DESC);
CREATE INDEX IF NOT EXISTS ix_api_metrics_route ON api_metrics (route);

CREATE TABLE IF NOT EXISTS client_crash_reports (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID REFERENCES users(id) ON DELETE SET NULL,
    page         TEXT,
    agent        TEXT,
    stack        TEXT,
    payload_json JSONB       NOT NULL DEFAULT '{}'::JSONB,
    reported_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_client_crash_at ON client_crash_reports (reported_at DESC);
