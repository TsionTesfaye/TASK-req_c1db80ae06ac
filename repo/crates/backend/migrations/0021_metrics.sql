-- 0021_metrics.sql
-- Metric definitions, formula computations, and lineage (P-B).
--
-- Business rules:
--   * A metric definition names a formula_kind + set of source_ids + window.
--   * Each computation stores the input observation ids + values so lineage
--     can be reconstructed without re-running the formula.
--   * Soft-deleting a definition (enabled=false) preserves history.
--   * Formula kinds are restricted to the three supported algorithms.

CREATE TABLE IF NOT EXISTS metric_definitions (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT        NOT NULL UNIQUE,
    formula_kind TEXT        NOT NULL
        CHECK (formula_kind IN ('moving_average', 'rate_of_change', 'comfort_index')),
    params       JSONB       NOT NULL DEFAULT '{}'::JSONB,
    source_ids   UUID[]      NOT NULL DEFAULT '{}',
    window_seconds INT       NOT NULL DEFAULT 3600,
    enabled      BOOLEAN     NOT NULL DEFAULT TRUE,
    created_by   UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_metric_def_enabled
    ON metric_definitions (enabled) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS metric_computations (
    id             UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    definition_id  UUID             NOT NULL REFERENCES metric_definitions(id) ON DELETE CASCADE,
    computed_at    TIMESTAMPTZ      NOT NULL DEFAULT NOW(),
    result         DOUBLE PRECISION NOT NULL,
    inputs         JSONB            NOT NULL DEFAULT '[]'::JSONB,
    -- inputs is an array of {observation_id, observed_at, value} objects
    window_start   TIMESTAMPTZ      NOT NULL,
    window_end     TIMESTAMPTZ      NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_metric_comp_def_at
    ON metric_computations (definition_id, computed_at DESC);

-- Trigger: keep metric_definitions.updated_at fresh.
CREATE OR REPLACE FUNCTION metric_definitions_set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_metric_def_updated_at ON metric_definitions;
CREATE TRIGGER trg_metric_def_updated_at
BEFORE UPDATE ON metric_definitions
FOR EACH ROW EXECUTE FUNCTION metric_definitions_set_updated_at();
