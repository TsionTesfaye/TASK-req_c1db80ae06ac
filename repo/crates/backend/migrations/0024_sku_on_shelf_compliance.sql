-- 0024_sku_on_shelf_compliance.sql
-- Audit #13 Issue #2: add first-class SKU on-shelf compliance support
-- to the metric/KPI/alert model.
--
-- What this migration adds:
--   * `sku_on_shelf_compliance` as a valid `metric_definitions.formula_kind`
--     — it computes the % of tracked SKUs observed on-shelf in the window.
--   * `sku_on_shelf_compliance` as a valid `kpi_rollup_daily.metric_kind`
--     — so the dashboard/drill surfaces can carry the new kind.
--   * A seeded default metric definition + default alert rule so the
--     behavior is real out of the box (not "schema-only"). The alert
--     fires when compliance drops below 95 % (classic retail shelf-set
--     threshold), severity=warning, eval-every cycle of the existing
--     30-second alert evaluator.
--
-- The two CHECK constraints were previously inline (no explicit name),
-- so the generated Postgres name is `<table>_<column>_check`.
-- We drop and re-add to widen the allowed set in-place.

-- ---- metric_definitions.formula_kind ---------------------------------------
ALTER TABLE metric_definitions
    DROP CONSTRAINT IF EXISTS metric_definitions_formula_kind_check;
ALTER TABLE metric_definitions
    ADD CONSTRAINT metric_definitions_formula_kind_check
    CHECK (formula_kind IN (
        'moving_average',
        'rate_of_change',
        'comfort_index',
        'sku_on_shelf_compliance'
    ));

-- ---- kpi_rollup_daily.metric_kind ------------------------------------------
ALTER TABLE kpi_rollup_daily
    DROP CONSTRAINT IF EXISTS kpi_rollup_daily_metric_kind_check;
ALTER TABLE kpi_rollup_daily
    ADD CONSTRAINT kpi_rollup_daily_metric_kind_check
    CHECK (metric_kind IN (
        'cycle_time',
        'funnel_conversion',
        'anomaly_count',
        'efficiency_index',
        'sku_on_shelf_compliance'
    ));

-- ---- Seed default metric definition + default alert rule ------------------
-- Idempotent: only insert if the definition name is not already present.
-- Defaults:
--   * formula_kind       = 'sku_on_shelf_compliance'
--   * window_seconds     = 86400 (one day)
--   * params.threshold   = 95.0 (percent)
--   * alert: compliance < 95 (warning), duration_seconds = 0
INSERT INTO metric_definitions (name, formula_kind, params, source_ids, window_seconds, enabled)
SELECT 'sku_on_shelf_compliance_default',
       'sku_on_shelf_compliance',
       '{"threshold_pct": 95.0}'::jsonb,
       ARRAY[]::UUID[],
       86400,
       TRUE
WHERE NOT EXISTS (
    SELECT 1 FROM metric_definitions
    WHERE name = 'sku_on_shelf_compliance_default'
);

INSERT INTO alert_rules (metric_definition_id, threshold, operator, duration_seconds, severity, enabled)
SELECT md.id, 95.0, '<', 0, 'warning', TRUE
FROM metric_definitions md
WHERE md.name = 'sku_on_shelf_compliance_default'
  AND NOT EXISTS (
      SELECT 1 FROM alert_rules ar
      WHERE ar.metric_definition_id = md.id
  );
