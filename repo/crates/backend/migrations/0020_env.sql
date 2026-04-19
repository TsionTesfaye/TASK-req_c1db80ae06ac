-- 0020_env.sql
-- Environmental data sources + partitioned observations table (P-B).
--
-- Business rules:
--   * Each env_source is scoped to a site + department + unit.
--   * Observations are immutable once inserted (no UPDATE path).
--   * Partitioned by observed_at range for future pruning by retention enforcer.
--   * Default partition covers 2024-01-01 → 2099-01-01.

CREATE TABLE IF NOT EXISTS env_sources (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT        NOT NULL,
    kind          TEXT        NOT NULL,       -- e.g. 'temperature', 'humidity', 'pressure'
    site_id       UUID        REFERENCES sites(id)       ON DELETE SET NULL,
    department_id UUID        REFERENCES departments(id) ON DELETE SET NULL,
    unit_id       UUID        REFERENCES units(id)       ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_env_sources_site
    ON env_sources (site_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_env_sources_dept
    ON env_sources (department_id) WHERE deleted_at IS NULL;

-- Partitioned observations table. The partition key is observed_at.
-- We create one default range partition covering the expected operational window.
-- Future partitions can be added (e.g. per-year) as volume grows.
CREATE TABLE IF NOT EXISTS env_observations (
    id          UUID             NOT NULL DEFAULT gen_random_uuid(),
    source_id   UUID             NOT NULL REFERENCES env_sources(id) ON DELETE CASCADE,
    observed_at TIMESTAMPTZ      NOT NULL,
    value       DOUBLE PRECISION NOT NULL,
    unit        TEXT             NOT NULL,
    raw         JSONB
) PARTITION BY RANGE (observed_at);

CREATE TABLE IF NOT EXISTS env_observations_default
    PARTITION OF env_observations
    FOR VALUES FROM ('2024-01-01') TO ('2099-01-01');

-- Primary key must be created on the parent and will propagate to partitions.
-- In Postgres 11+ we can create unique constraints on partitioned tables
-- only if the partition key is part of the constraint. We use a plain index
-- for the per-source timeline lookup instead.
CREATE INDEX IF NOT EXISTS ix_env_obs_source_at
    ON env_observations (source_id, observed_at DESC);

-- Trigger: keep env_sources.updated_at fresh.
CREATE OR REPLACE FUNCTION env_sources_set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_env_sources_updated_at ON env_sources;
CREATE TRIGGER trg_env_sources_updated_at
BEFORE UPDATE ON env_sources
FOR EACH ROW EXECUTE FUNCTION env_sources_set_updated_at();
