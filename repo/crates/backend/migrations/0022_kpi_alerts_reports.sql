-- 0022_kpi_alerts_reports.sql
-- Alert rules + events, report jobs, KPI rollup cache (P-B).
--
-- Business rules:
--   * Alert rules reference a metric_definition; thresholds are checked by
--     the 30-second background evaluator job.
--   * duration_seconds = 0 means fire immediately on first violation.
--   * Alert events track ack (who + when) and resolution (when cleared).
--   * Report jobs have a lifecycle: scheduled → running → done|failed|cancelled.
--   * One transient retry: retry_count may not exceed 1 before terminal fail.
--   * kpi_rollup_daily is the on-demand KPI cache written by the hourly job
--     (or computed inline in K1-K6 handlers if no cached row exists).

CREATE TABLE IF NOT EXISTS alert_rules (
    id                   UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    metric_definition_id UUID        NOT NULL REFERENCES metric_definitions(id) ON DELETE CASCADE,
    threshold            DOUBLE PRECISION NOT NULL,
    operator             TEXT        NOT NULL
        CHECK (operator IN ('>', '<', '>=', '<=', '=')),
    duration_seconds     INT         NOT NULL DEFAULT 0,
    severity             TEXT        NOT NULL DEFAULT 'warning'
        CHECK (severity IN ('info', 'warning', 'critical')),
    enabled              BOOLEAN     NOT NULL DEFAULT TRUE,
    created_by           UUID        REFERENCES users(id) ON DELETE SET NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_alert_rules_metric
    ON alert_rules (metric_definition_id) WHERE enabled = TRUE;

CREATE TABLE IF NOT EXISTS alert_events (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    rule_id     UUID        NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
    fired_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    value       DOUBLE PRECISION NOT NULL,
    acked_at    TIMESTAMPTZ,
    acked_by    UUID        REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS ix_alert_events_rule_fired
    ON alert_events (rule_id, fired_at DESC);
CREATE INDEX IF NOT EXISTS ix_alert_events_unacked
    ON alert_events (acked_at) WHERE acked_at IS NULL;

-- Trigger: keep alert_rules.updated_at fresh.
CREATE OR REPLACE FUNCTION alert_rules_set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_alert_rules_updated_at ON alert_rules;
CREATE TRIGGER trg_alert_rules_updated_at
BEFORE UPDATE ON alert_rules
FOR EACH ROW EXECUTE FUNCTION alert_rules_set_updated_at();

CREATE TABLE IF NOT EXISTS report_jobs (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id            UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind                TEXT        NOT NULL
        CHECK (kind IN ('kpi_summary', 'env_series', 'alert_digest')),
    format              TEXT        NOT NULL
        CHECK (format IN ('pdf', 'csv', 'xlsx')),
    params              JSONB       NOT NULL DEFAULT '{}'::JSONB,
    cron                TEXT,
    status              TEXT        NOT NULL DEFAULT 'scheduled'
        CHECK (status IN ('scheduled', 'running', 'done', 'failed', 'cancelled')),
    last_run_at         TIMESTAMPTZ,
    last_artifact_path  TEXT,
    retry_count         INT         NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS ix_report_jobs_owner
    ON report_jobs (owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS ix_report_jobs_due
    ON report_jobs (status, updated_at) WHERE status IN ('scheduled', 'failed');

-- Trigger: keep report_jobs.updated_at fresh.
CREATE OR REPLACE FUNCTION report_jobs_set_updated_at()
RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_report_jobs_updated_at ON report_jobs;
CREATE TRIGGER trg_report_jobs_updated_at
BEFORE UPDATE ON report_jobs
FOR EACH ROW EXECUTE FUNCTION report_jobs_set_updated_at();

-- KPI rollup cache for dashboard K1–K6 (populated on-demand or by background job).
CREATE TABLE IF NOT EXISTS kpi_rollup_daily (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    day            DATE        NOT NULL,
    site_id        UUID        REFERENCES sites(id) ON DELETE CASCADE,
    department_id  UUID        REFERENCES departments(id) ON DELETE CASCADE,
    metric_kind    TEXT        NOT NULL
        CHECK (metric_kind IN ('cycle_time', 'funnel_conversion', 'anomaly_count', 'efficiency_index')),
    value          DOUBLE PRECISION NOT NULL
);
-- Logical uniqueness across (day, site_id, dept_id, metric_kind) with NULL
-- handled explicitly — UNIQUE constraints can't use function expressions,
-- so we use a unique index with COALESCE placeholders instead.
CREATE UNIQUE INDEX IF NOT EXISTS ux_kpi_rollup_daily_key
    ON kpi_rollup_daily (
        day,
        COALESCE(site_id,       '00000000-0000-0000-0000-000000000000'::uuid),
        COALESCE(department_id, '00000000-0000-0000-0000-000000000000'::uuid),
        metric_kind
    );
CREATE INDEX IF NOT EXISTS ix_kpi_rollup_day
    ON kpi_rollup_daily (day DESC, metric_kind);
