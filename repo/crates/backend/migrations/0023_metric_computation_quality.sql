-- 0023_metric_computation_quality.sql
-- Audit fix: make alignment + confidence first-class columns on
-- `metric_computations` so the comfort-index pipeline (and any future
-- multi-source derived metric) can surface:
--   * alignment  — how well the source timestamps line up with the
--                  computation `at` within the window. 1.0 = perfect,
--                  0.0 = stale / off-window.
--   * confidence — how trustworthy the computation is given the count of
--                  contributing sources and sample density. 1.0 = full
--                  three-source comfort (temp + humidity + air_speed)
--                  with dense samples; lower = partial or sparse.
--
-- Both columns are NULLable so pre-existing rows stay valid, and so
-- single-source formulas (moving_average, rate_of_change) that do not
-- compute these dimensions simply leave them NULL.

ALTER TABLE metric_computations
    ADD COLUMN IF NOT EXISTS alignment  DOUBLE PRECISION
        CHECK (alignment IS NULL OR (alignment >= 0.0 AND alignment <= 1.0)),
    ADD COLUMN IF NOT EXISTS confidence DOUBLE PRECISION
        CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0));
